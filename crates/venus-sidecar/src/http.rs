//! Health + Flush + git log HTTP. Pin still starts at claim.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use std::sync::{Arc, Mutex};

use axum::extract::{Query, State};
use axum::http::{header, HeaderValue, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tower_http::cors::{AllowOrigin, CorsLayer};

use crate::config::DEFAULT_CORS_ORIGINS;
use crate::cut::GIT_PATH;
use crate::git::{self, WikiConfig, DEFAULT_WIKI_DIR};
use crate::queue;
use crate::{workspace_id_ok, DEFAULT_WORKSPACE_ID};

#[derive(Clone, Default)]
struct GitLogCache {
    inner: Arc<Mutex<Option<GitLogCacheEntry>>>,
}

struct GitLogCacheEntry {
    head: String,
    limit: usize,
    entries: Vec<git::GitLogEntry>,
}

impl GitLogCache {
    fn get(&self, head: &str, limit: usize) -> Option<Vec<git::GitLogEntry>> {
        let guard = self.inner.lock().unwrap_or_else(|err| err.into_inner());
        let hit = guard.as_ref()?;
        if hit.head == head && hit.limit == limit {
            Some(hit.entries.clone())
        } else {
            None
        }
    }

    fn put(&self, head: String, limit: usize, entries: Vec<git::GitLogEntry>) {
        let mut guard = self.inner.lock().unwrap_or_else(|err| err.into_inner());
        *guard = Some(GitLogCacheEntry {
            head,
            limit,
            entries,
        });
    }
}

#[derive(Clone)]
pub struct HttpState {
    pub pool: Option<PgPool>,
    pub wiki: WikiConfig,
    log_cache: GitLogCache,
}

#[derive(Debug, Deserialize, Default)]
pub struct FlushQuery {
    /// Omit for the M0 wiki. Tests / later wikis may pass a UUID.
    pub workspace: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct GitLogQuery {
    /// Catalog v0 path. Omit for `spec/home.md`.
    pub path: Option<String>,
    /// Stop once this many commits touch the path. Omit for 50.
    pub limit: Option<usize>,
}

/// `None` pool: health + git log still serve; `POST /flush` is 503.
pub fn router(pool: Option<PgPool>) -> Router {
    router_with_wiki_and_cors(
        pool,
        WikiConfig::new(DEFAULT_WIKI_DIR),
        DEFAULT_CORS_ORIGINS
            .iter()
            .map(|s| (*s).to_string())
            .collect(),
    )
}

pub fn router_with_cors(pool: Option<PgPool>, cors_origins: Vec<String>) -> Router {
    router_with_wiki_and_cors(pool, WikiConfig::new(DEFAULT_WIKI_DIR), cors_origins)
}

pub fn router_with_wiki(pool: Option<PgPool>, wiki: WikiConfig) -> Router {
    router_with_wiki_and_cors(
        pool,
        wiki,
        DEFAULT_CORS_ORIGINS
            .iter()
            .map(|s| (*s).to_string())
            .collect(),
    )
}

pub fn router_with_wiki_and_cors(
    pool: Option<PgPool>,
    wiki: WikiConfig,
    cors_origins: Vec<String>,
) -> Router {
    let mut app = Router::new()
        .route("/", get(root))
        .route("/flush", post(flush))
        .route("/flush/status", get(flush_status))
        .route("/git/log", get(git_log))
        .with_state(HttpState {
            pool,
            wiki,
            log_cache: GitLogCache::default(),
        });
    if let Some(layer) = cors_layer(&cors_origins) {
        app = app.layer(layer);
    }
    app
}

fn cors_layer(origins: &[String]) -> Option<CorsLayer> {
    if origins.is_empty() {
        return None;
    }
    let mut values = Vec::with_capacity(origins.len());
    for origin in origins {
        if let Ok(v) = HeaderValue::from_str(origin) {
            values.push(v);
        }
    }
    if values.is_empty() {
        return None;
    }
    Some(
        CorsLayer::new()
            .allow_origin(AllowOrigin::list(values))
            .allow_methods([Method::GET, Method::HEAD, Method::POST])
            .allow_headers([header::CONTENT_TYPE]),
    )
}

/// Explicit `?workspace=`, or `bound` when the query is omitted / blank.
pub fn flush_workspace(query: Option<String>, bound: &str) -> String {
    query
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| bound.to_string())
}

async fn root() -> impl IntoResponse {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        "venus-sidecar\n",
    )
}

fn text_response(status: StatusCode, body: &'static str) -> Response {
    (
        status,
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        body,
    )
        .into_response()
}

/// Shared `?workspace=` checks for Flush and its status. `Err` is the HTTP response.
fn workspace_for_flush(state: &HttpState, query: Option<String>) -> Result<String, Response> {
    let bound = state
        .wiki
        .workspace_id
        .clone()
        .unwrap_or_else(|| DEFAULT_WORKSPACE_ID.to_string());
    let workspace = flush_workspace(query, &bound);
    if !workspace_id_ok(&workspace) {
        return Err(text_response(
            StatusCode::BAD_REQUEST,
            "invalid workspace\n",
        ));
    }
    let workspace = workspace.to_ascii_lowercase();
    if state.wiki.workspace_id.is_some() && workspace != bound.to_ascii_lowercase() {
        tracing::warn!(workspace_id = %workspace, "flush workspace is not bound to this wiki");
        return Err(text_response(StatusCode::NOT_FOUND, "unknown workspace\n"));
    }
    if state.pool.is_none() {
        return Err(text_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "no database\n",
        ));
    }
    Ok(workspace)
}

async fn flush(State(state): State<HttpState>, Query(q): Query<FlushQuery>) -> Response {
    let workspace = match workspace_for_flush(&state, q.workspace) {
        Ok(workspace) => workspace,
        Err(response) => return response,
    };
    let pool = state.pool.as_ref().expect("pool checked");
    match queue::flush_now(pool, &workspace).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => {
            if let Some(blocked) = e.downcast_ref::<queue::FlushBlocked>() {
                let body = format!("{blocked}\n");
                return (
                    StatusCode::CONFLICT,
                    [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                    body,
                )
                    .into_response();
            }
            tracing::warn!(workspace_id = %workspace, error = %e, "flush upsert failed");
            text_response(StatusCode::INTERNAL_SERVER_ERROR, "flush failed\n")
        }
    }
}

#[derive(Serialize)]
struct FlushStatusBody {
    sha: Option<String>,
    last_error: Option<String>,
    attempts: i32,
    failed: bool,
}

async fn flush_status(State(state): State<HttpState>, Query(q): Query<FlushQuery>) -> Response {
    let workspace = match workspace_for_flush(&state, q.workspace) {
        Ok(workspace) => workspace,
        Err(response) => return response,
    };
    let pool = state.pool.as_ref().expect("pool checked");
    match queue::flush_status(pool, &workspace).await {
        Ok(status) => Json(FlushStatusBody {
            sha: status.sha,
            last_error: status.last_error,
            attempts: status.attempts,
            failed: status.failed,
        })
        .into_response(),
        Err(e) => {
            tracing::warn!(workspace_id = %workspace, error = %e, "flush status failed");
            text_response(StatusCode::INTERNAL_SERVER_ERROR, "flush status failed\n")
        }
    }
}

async fn git_log(State(state): State<HttpState>, Query(q): Query<GitLogQuery>) -> Response {
    let path = q
        .path
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(GIT_PATH);
    if !git::is_catalog_log_path(path) {
        return (
            StatusCode::BAD_REQUEST,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            "path must be spec/home.md\n",
        )
            .into_response();
    }
    let limit = q.limit.unwrap_or(git::GIT_LOG_DEFAULT_LIMIT);
    let head = match git::head_oid(&state.wiki) {
        Ok(head) => head,
        Err(e) => {
            tracing::warn!(error = %e, "git log failed");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                "git log failed\n",
            )
                .into_response();
        }
    };
    if let Some(head) = head.as_deref() {
        if let Some(entries) = state.log_cache.get(head, limit) {
            return Json(entries).into_response();
        }
    }
    match git::log_path(&state.wiki, path, limit) {
        Ok(entries) => {
            if let Some(head) = head {
                state.log_cache.put(head, limit, entries.clone());
            }
            Json(entries).into_response()
        }
        Err(e) => {
            tracing::warn!(error = %e, "git log failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                "git log failed\n",
            )
                .into_response()
        }
    }
}
