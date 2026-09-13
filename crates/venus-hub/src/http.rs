//! HTTP + AFFiNE WebSocket. Same paths as M1 keck (export/blob aliases).
//! SPDX-License-Identifier: MIT OR Apache-2.0

use std::future::pending;
use std::sync::Arc;
use std::time::Duration;

use axum::body::Bytes;
use axum::extract::ws::rejection::WebSocketUpgradeRejection;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{DefaultBodyLimit, FromRequestParts, Path, State};
use axum::http::request::Parts;
use axum::http::{header, HeaderMap, HeaderValue, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Serialize;
use tokio::time::{interval_at, Instant, MissedTickBehavior};
use tower_http::cors::{AllowOrigin, CorsLayer};

use crate::blobs::{blob_hash, sniff_content_type};
use crate::config::default_cors_origins;
use crate::db;
use crate::room::{GetRoomError, Hub, Room, OUTBOUND_BYTES, PERSIST_BYTES};
use crate::SUBPROTOCOL;

/// One inbound Yjs/CRDT WS frame. `DefaultBodyLimit` does not apply to WS.
/// A paste larger than this closes the socket. Outbound budget is two of
/// these so a max-size frame plus follow-on typing does not detach a slightly
/// lagging peer.
pub const WS_MAX_MESSAGE: usize = 512 * 1024;

const _: () = assert!(
    OUTBOUND_BYTES >= WS_MAX_MESSAGE,
    "a max-size frame must fit in one client's outbound budget"
);
const _: () = assert!(
    PERSIST_BYTES >= WS_MAX_MESSAGE,
    "a max-size frame must fit in the persist buffer"
);
/// Server ping so half-open sockets are reaped (S3).
pub const WS_PING_EVERY: Duration = Duration::from_secs(30);
/// Detach if this many seconds pass with no Pong after a ping.
pub const WS_PONG_WAIT: Duration = Duration::from_secs(10);

/// Hyphenated UUID (`8-4-4-4-12`). Reject before lease or SQL so a loop of
/// random ids cannot grow rooms (S2).
pub const WORKSPACE_ID_LEN: usize = 36;
/// Same as [`WORKSPACE_ID_LEN`]. Kept so S2 tests that used the old slug cap
/// still name the constant.
pub const WORKSPACE_ID_MAX: usize = WORKSPACE_ID_LEN;

/// Canonical UUID text: 36 chars, hyphens at 8/13/18/23, hex elsewhere.
/// Accepts mixed case; callers lowercase before SQL.
pub fn workspace_id_ok(id: &str) -> bool {
    if id.len() != WORKSPACE_ID_LEN {
        return false;
    }
    for (i, c) in id.bytes().enumerate() {
        match i {
            8 | 13 | 18 | 23 => {
                if c != b'-' {
                    return false;
                }
            }
            _ => {
                if !c.is_ascii_hexdigit() {
                    return false;
                }
            }
        }
    }
    true
}

fn take_workspace_id(id: String) -> Result<String, Response> {
    if !workspace_id_ok(&id) {
        tracing::warn!(bytes = id.len(), "invalid workspace_id");
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "invalid workspace_id" })),
        )
            .into_response());
    }
    Ok(id.to_ascii_lowercase())
}

const BLOB_CACHE_CONTROL: &str = "public, max-age=31536000, immutable";

fn blob_etag(hash: &str) -> String {
    let mut s = String::with_capacity(hash.len() + 2);
    s.push('"');
    s.push_str(hash);
    s.push('"');
    s
}

fn blob_cache_headers(hash: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(BLOB_CACHE_CONTROL),
    );
    if let Ok(v) = HeaderValue::from_str(&blob_etag(hash)) {
        headers.insert(header::ETAG, v);
    }
    headers
}

fn blob_not_modified(hash: &str) -> Response {
    (StatusCode::NOT_MODIFIED, blob_cache_headers(hash)).into_response()
}

/// `If-None-Match` vs the content-addressed blob hash (S7). `*` and weak tags match.
fn etag_matches(headers: &HeaderMap, hash: &str) -> bool {
    let Some(raw) = headers
        .get(header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
    else {
        return false;
    };
    for part in raw.split(',') {
        let t = part.trim();
        if t == "*" {
            return true;
        }
        let t = match t.strip_prefix("W/") {
            Some(rest) => rest.trim(),
            None => t,
        };
        let t = match t.strip_prefix('"') {
            Some(inner) => match inner.strip_suffix('"') {
                Some(hash_only) => hash_only,
                None => t,
            },
            None => t,
        };
        if t == hash {
            return true;
        }
    }
    false
}

#[derive(Clone)]
pub struct AppState {
    pub hub: Arc<Hub>,
    pub ws_max_message: usize,
    pub ws_ping: Duration,
    pub ws_pong: Duration,
    pub cors_origins: Vec<String>,
}

impl AppState {
    pub fn new(hub: Arc<Hub>) -> Self {
        Self::with_cors(hub, default_cors_origins())
    }

    pub fn with_cors(hub: Arc<Hub>, cors_origins: Vec<String>) -> Self {
        Self {
            hub,
            ws_max_message: WS_MAX_MESSAGE,
            ws_ping: WS_PING_EVERY,
            ws_pong: WS_PONG_WAIT,
            cors_origins,
        }
    }
}

/// Methods and headers the API actually uses. Empty origin list → no CORS layer
/// (same-origin nginx). Never mirrors the request `Origin`.
fn cors_layer(origins: &[String]) -> Option<CorsLayer> {
    if origins.is_empty() {
        return None;
    }
    let mut values = Vec::with_capacity(origins.len());
    for origin in origins {
        match HeaderValue::from_str(origin) {
            Ok(v) => values.push(v),
            Err(_) => {}
        }
    }
    if values.is_empty() {
        return None;
    }
    Some(
        CorsLayer::new()
            .allow_origin(AllowOrigin::list(values))
            .allow_methods([Method::GET, Method::HEAD, Method::POST, Method::DELETE])
            .allow_headers([header::CONTENT_TYPE, header::IF_NONE_MATCH]),
    )
}

pub fn router(state: AppState) -> Router {
    let cors = cors_layer(&state.cors_origins);
    let mut app = Router::new()
        .route("/", get(root))
        .route(
            "/collaboration/{workspace_id}",
            get(collaboration_get).post(collaboration_post),
        )
        .route("/api/block/{workspace_id}/export", get(export_doc))
        .route("/api/blobs/{workspace_id}", post(blob_post))
        .route(
            "/api/blobs/{workspace_id}/{*hash}",
            get(blob_get).head(blob_head).delete(blob_delete),
        )
        .layer(DefaultBodyLimit::max(32 * 1024 * 1024));
    if let Some(layer) = cors {
        app = app.layer(layer);
    }
    app.with_state(state)
}

async fn root() -> impl IntoResponse {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        "venus-hub\n",
    )
}

#[derive(Serialize)]
struct ProtocolBody {
    protocol: &'static str,
}

async fn collaboration_post(Path(workspace_id): Path<String>) -> Response {
    if let Err(r) = take_workspace_id(workspace_id) {
        return r;
    }
    Json(ProtocolBody {
        protocol: SUBPROTOCOL,
    })
    .into_response()
}

/// GET without `Upgrade: websocket` is health JSON (L9). Axum’s `WebSocketUpgrade`
/// is not `OptionalFromRequestParts`; a required extractor 400s before the handler.
struct OptionalWs(Option<WebSocketUpgrade>);

impl<S> FromRequestParts<S> for OptionalWs
where
    S: Send + Sync,
{
    type Rejection = WebSocketUpgradeRejection;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        if !wants_websocket(&parts.headers) {
            return Ok(Self(None));
        }
        WebSocketUpgrade::from_request_parts(parts, state)
            .await
            .map(|ws| Self(Some(ws)))
    }
}

async fn collaboration_get(
    Path(workspace_id): Path<String>,
    State(st): State<AppState>,
    OptionalWs(ws): OptionalWs,
) -> Response {
    let workspace_id = match take_workspace_id(workspace_id) {
        Ok(id) => id,
        Err(r) => return r,
    };
    let Some(ws) = ws else {
        return Json(ProtocolBody {
            protocol: SUBPROTOCOL,
        })
        .into_response();
    };

    match st.hub.get_room(&workspace_id).await {
        Ok(room) => {
            let max = st.ws_max_message.max(1);
            let ping = st.ws_ping;
            let pong = st.ws_pong;
            ws.max_message_size(max)
                .max_frame_size(max)
                .protocols([SUBPROTOCOL])
                .on_upgrade(move |socket| handle_socket(socket, room, ping, pong))
        }
        Err(GetRoomError::Held {
            workspace_id: id,
            owner,
        }) => {
            tracing::info!(
                workspace_id = %id,
                owner = ?owner,
                "refuse second owner"
            );
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({
                    "error": "workspace owned by another hub",
                    "workspace_id": workspace_id,
                })),
            )
                .into_response()
        }
        Err(GetRoomError::Store(e)) => {
            tracing::error!(workspace_id = %workspace_id, error = %e, "get_room store");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "hub store failed",
                    "workspace_id": workspace_id,
                })),
            )
                .into_response()
        }
    }
}

fn wants_websocket(headers: &HeaderMap) -> bool {
    headers
        .get(header::UPGRADE)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.eq_ignore_ascii_case("websocket"))
        .unwrap_or(false)
}

async fn handle_socket(mut socket: WebSocket, room: Arc<Room>, ping: Duration, pong: Duration) {
    let (id, tx, mut rx) = room.connect_client();
    let hello = match room.attach(id, tx).await {
        Ok(frames) => frames,
        Err(e) => {
            tracing::error!(error = %e, "ws attach");
            room.detach(id).await;
            let _ = socket.send(Message::Close(None)).await;
            return;
        }
    };
    for frame in hello {
        if socket.send(Message::Binary(frame.into())).await.is_err() {
            room.detach(id).await;
            return;
        }
    }

    let mut ping_tick = ws_ping_interval(ping);
    let mut awaiting_pong = false;
    let mut pong_by = Instant::now();

    loop {
        let wait_pong = async {
            if awaiting_pong {
                tokio::time::sleep_until(pong_by).await;
            } else {
                pending::<()>().await;
            }
        };
        let ping_enabled = ping_tick.is_some();

        tokio::select! {
            incoming = socket.recv() => {
                match incoming {
                    None => break,
                    Some(Ok(Message::Binary(bin))) => {
                        room.handle_binary(id, &bin).await;
                    }
                    Some(Ok(Message::Ping(p))) => {
                        if socket.send(Message::Pong(p)).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(Message::Pong(_))) => {
                        awaiting_pong = false;
                    }
                    Some(Ok(Message::Close(_)) | Err(_)) => break,
                    Some(Ok(Message::Text(_))) => {}
                }
            }
            out = rx.recv() => {
                match out {
                    Some(frame) => {
                        if socket.send(Message::Binary(frame)).await.is_err() {
                            break;
                        }
                    }
                    None => break,
                }
            }
            _ = async {
                match ping_tick.as_mut() {
                    Some(tick) => tick.tick().await,
                    None => pending().await,
                }
            }, if ping_enabled => {
                if awaiting_pong {
                    tracing::warn!(
                        workspace = %room.workspace_id,
                        "ws pong missed; detach"
                    );
                    break;
                }
                if socket.send(Message::Ping(Bytes::new())).await.is_err() {
                    break;
                }
                awaiting_pong = true;
                pong_by = Instant::now() + pong;
            }
            _ = wait_pong => {
                tracing::warn!(
                    workspace = %room.workspace_id,
                    "ws pong timeout; detach"
                );
                break;
            }
        }
    }
    room.detach(id).await;
}

/// `interval_at` panics on a zero period (L18). `None` is the disable path the
/// `ping_enabled` guard used to imply.
fn ws_ping_interval(ping: Duration) -> Option<tokio::time::Interval> {
    if ping.is_zero() {
        return None;
    }
    let mut tick = interval_at(Instant::now() + ping, ping);
    tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
    Some(tick)
}

async fn export_doc(Path(workspace_id): Path<String>, State(st): State<AppState>) -> Response {
    let workspace_id = match take_workspace_id(workspace_id) {
        Ok(id) => id,
        Err(r) => return r,
    };
    match st.hub.live_export(&workspace_id).await {
        Ok(bytes) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/octet-stream")],
            bytes,
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "export");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

#[derive(Serialize)]
struct BlobPosted {
    id: String,
    exists: bool,
}

async fn blob_post(
    Path(workspace_id): Path<String>,
    State(st): State<AppState>,
    body: Bytes,
) -> Response {
    let workspace_id = match take_workspace_id(workspace_id) {
        Ok(id) => id,
        Err(r) => return r,
    };
    let id = blob_hash(&body);
    match db::put_blob(&st.hub.pool, &workspace_id, &id, &body).await {
        Ok(exists) => Json(BlobPosted { id, exists }).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "blob post");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn blob_get(
    Path((workspace_id, hash)): Path<(String, String)>,
    State(st): State<AppState>,
    headers: HeaderMap,
) -> Response {
    let workspace_id = match take_workspace_id(workspace_id) {
        Ok(id) => id,
        Err(r) => return r,
    };
    if etag_matches(&headers, &hash) {
        return match db::blob_len(&st.hub.pool, &workspace_id, &hash).await {
            Ok(Some(_)) => blob_not_modified(&hash),
            Ok(None) => StatusCode::NOT_FOUND.into_response(),
            Err(e) => {
                tracing::error!(error = %e, "blob get");
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        };
    }
    match db::get_blob(&st.hub.pool, &workspace_id, &hash).await {
        Ok(Some(bytes)) => {
            let ct = sniff_content_type(&bytes);
            let mut headers = blob_cache_headers(&hash);
            headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(ct));
            (StatusCode::OK, headers, bytes).into_response()
        }
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => {
            tracing::error!(error = %e, "blob get");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn blob_head(
    Path((workspace_id, hash)): Path<(String, String)>,
    State(st): State<AppState>,
    headers: HeaderMap,
) -> Response {
    let workspace_id = match take_workspace_id(workspace_id) {
        Ok(id) => id,
        Err(r) => return r,
    };
    if etag_matches(&headers, &hash) {
        return match db::blob_len(&st.hub.pool, &workspace_id, &hash).await {
            Ok(Some(_)) => blob_not_modified(&hash),
            Ok(None) => StatusCode::NOT_FOUND.into_response(),
            Err(e) => {
                tracing::error!(error = %e, "blob head");
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        };
    }
    match db::blob_len(&st.hub.pool, &workspace_id, &hash).await {
        Ok(Some(len)) => {
            let mut headers = blob_cache_headers(&hash);
            headers.insert(
                header::CONTENT_LENGTH,
                HeaderValue::from_str(&len.to_string()).unwrap_or(HeaderValue::from_static("0")),
            );
            (StatusCode::OK, headers).into_response()
        }
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => {
            tracing::error!(error = %e, "blob head");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn blob_delete(
    Path((workspace_id, hash)): Path<(String, String)>,
    State(st): State<AppState>,
) -> Response {
    let workspace_id = match take_workspace_id(workspace_id) {
        Ok(id) => id,
        Err(r) => return r,
    };
    match db::delete_blob(&st.hub.pool, &workspace_id, &hash).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[cfg(test)]
mod l18 {
    use super::*;

    #[tokio::test]
    async fn ws_ping_interval_is_none_when_zero() {
        assert!(ws_ping_interval(Duration::ZERO).is_none());
        assert!(ws_ping_interval(Duration::from_millis(1)).is_some());
    }
}

#[cfg(test)]
mod s2 {
    use super::*;

    #[test]
    fn workspace_id_shape() {
        assert!(workspace_id_ok(crate::DEFAULT_WORKSPACE_ID));
        assert!(workspace_id_ok("77E4A2B1-8B40-5979-A73C-FD4477216D00"));
        assert_eq!(
            take_workspace_id("77E4A2B1-8B40-5979-A73C-FD4477216D00".into()).unwrap(),
            crate::DEFAULT_WORKSPACE_ID
        );
        assert!(!workspace_id_ok("venus-m0"));
        assert!(!workspace_id_ok(""));
        assert!(!workspace_id_ok(&"a".repeat(WORKSPACE_ID_MAX)));
        assert!(!workspace_id_ok(&"a".repeat(WORKSPACE_ID_MAX + 1)));
        assert!(!workspace_id_ok("77e4a2b18b405979a73cfd4477216d00"));
        assert!(!workspace_id_ok("has space"));
        assert!(!workspace_id_ok("a/b"));
        assert!(!workspace_id_ok("Å"));
    }
}

#[cfg(test)]
mod s7 {
    use super::*;

    fn headers_if_none_match(v: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert(
            header::IF_NONE_MATCH,
            HeaderValue::from_str(v).expect("etag"),
        );
        h
    }

    #[test]
    fn etag_matches_quoted_hash_and_star() {
        let hash = "abc=";
        assert!(!etag_matches(&HeaderMap::new(), hash));
        assert!(etag_matches(&headers_if_none_match("\"abc=\""), hash));
        assert!(etag_matches(&headers_if_none_match("W/\"abc=\""), hash));
        assert!(etag_matches(&headers_if_none_match("*"), hash));
        assert!(etag_matches(
            &headers_if_none_match("\"other\", \"abc=\""),
            hash
        ));
        assert!(!etag_matches(&headers_if_none_match("\"nope\""), hash));
    }
}
