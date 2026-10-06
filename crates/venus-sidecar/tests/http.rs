//! M3 step-worker / step-dirty-idle: GET / health; Flush without a DSN is 503.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;
use venus_sidecar::git::WikiConfig;
use venus_sidecar::http::{flush_workspace, router, router_with_wiki};
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
fn flush_empty_body_is_bound_wiki() {
    assert_eq!(
        flush_workspace(None, DEFAULT_WORKSPACE_ID),
        DEFAULT_WORKSPACE_ID
    );
    assert_eq!(
        flush_workspace(Some(String::new()), DEFAULT_WORKSPACE_ID),
        DEFAULT_WORKSPACE_ID
    );
    assert_eq!(
        flush_workspace(Some("  ".into()), DEFAULT_WORKSPACE_ID),
        DEFAULT_WORKSPACE_ID
    );
    let other = "11111111-1111-4111-a111-111111111111";
    assert_eq!(
        flush_workspace(Some(other.into()), DEFAULT_WORKSPACE_ID),
        other
    );
}

#[tokio::test]
async fn flush_status_without_pool_is_503() {
    let app = router(None);
    let res = app
        .oneshot(
            Request::builder()
                .uri("/flush/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
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

#[tokio::test]
async fn flush_other_workspace_is_404() {
    let app = router_with_wiki(None, WikiConfig::bound("wiki", DEFAULT_WORKSPACE_ID));
    let other = "11111111-1111-4111-a111-111111111111";
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/flush?workspace={other}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn flush_bad_workspace_is_400() {
    let app = router_with_wiki(None, WikiConfig::bound("wiki", DEFAULT_WORKSPACE_ID));
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/flush?workspace=not-a-uuid")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}
