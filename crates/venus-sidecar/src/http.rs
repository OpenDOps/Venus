//! Health + Flush + git log HTTP. Pin still starts at claim.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use axum::extract::{Query, State};
use axum::http::{header, HeaderValue, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use sqlx::PgPool;
use tower_http::cors::{AllowOrigin, CorsLayer};

use crate::config::DEFAULT_CORS_ORIGINS;
use crate::cut::GIT_PATH;
use crate::git::{self, WikiConfig, DEFAULT_WIKI_DIR};
use crate::queue;
use crate::DEFAULT_WORKSPACE_ID;

#[derive(Clone)]
pub struct HttpState {
    pub pool: Option<PgPool>,
    pub wiki: WikiConfig,
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
        .route("/git/log", get(git_log))
        .with_state(HttpState { pool, wiki });
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

pub fn flush_workspace(query: Option<String>) -> String {
    query
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DEFAULT_WORKSPACE_ID.to_string())
}

async fn root() -> impl IntoResponse {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        "venus-sidecar\n",
    )
}

async fn flush(State(state): State<HttpState>, Query(q): Query<FlushQuery>) -> Response {
    let Some(pool) = state.pool.as_ref() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            "no database\n",
        )
            .into_response();
    };
    let workspace = flush_workspace(q.workspace);
    match queue::flush_now(pool, &workspace).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => {
            tracing::warn!(workspace_id = %workspace, error = %e, "flush upsert failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                "flush failed\n",
            )
                .into_response()
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
    match git::log_path(&state.wiki, path) {
        Ok(entries) => Json(entries).into_response(),
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
