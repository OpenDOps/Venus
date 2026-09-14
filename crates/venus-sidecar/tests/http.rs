//! M3 step-worker / step-dirty-idle: GET / health; Flush without a DSN is 503.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;
use venus_sidecar::http::{flush_workspace, router};
use venus_sidecar::DEFAULT_WORKSPACE_ID;

#[tokio::test]
async fn root_is_venus_sidecar() {
    let app = router(None);
    let res = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(std::str::from_utf8(&bytes).unwrap().trim(), "venus-sidecar");
}

#[test]
fn flush_empty_body_is_m0_wiki() {
    assert_eq!(flush_workspace(None), DEFAULT_WORKSPACE_ID);
    assert_eq!(flush_workspace(Some(String::new())), DEFAULT_WORKSPACE_ID);
    assert_eq!(flush_workspace(Some("  ".into())), DEFAULT_WORKSPACE_ID);
    let other = "11111111-1111-4111-a111-111111111111";
    assert_eq!(flush_workspace(Some(other.into())), other);
}

#[tokio::test]
async fn flush_without_pool_is_503() {
    let app = router(None);
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/flush")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
}
