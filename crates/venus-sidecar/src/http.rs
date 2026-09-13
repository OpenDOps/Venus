//! Health HTTP. Flush / git log are later M3 steps.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use axum::http::{header, StatusCode};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;

pub fn router() -> Router {
    Router::new().route("/", get(root))
}

async fn root() -> impl IntoResponse {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        "venus-sidecar\n",
    )
}
