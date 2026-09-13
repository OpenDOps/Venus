//! M3 step-worker: process health is GET / → venus-sidecar.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;
use venus_sidecar::http::router;

#[tokio::test]
async fn root_is_venus_sidecar() {
    let app = router();
    let res = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(std::str::from_utf8(&bytes).unwrap().trim(), "venus-sidecar");
}
