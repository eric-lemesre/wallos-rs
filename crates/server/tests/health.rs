//! Tests d'intégration de l'endpoint de santé versionné.

use wallos_req_macros::verifies;
use wallos_server::app;

#[tokio::test]
#[verifies(REQ-OPS-001, case = "health v1 retourne 200")]
#[allow(non_snake_case)]
async fn getApiV1Health_returns_ok() {
    let app = app();
    let response = tower::ServiceExt::oneshot(
        app,
        axum::http::Request::builder()
            .uri("/api/v1/health")
            .body(axum::body::Body::empty())
            .unwrap(),
    )
    .await
    .unwrap();

    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "ok");
}

#[tokio::test]
#[verifies(REQ-OPS-001, case = "endpoint public accessible anonymement")]
#[allow(non_snake_case)]
async fn authz_owner_getApiV1Health() {
    let app = app();
    let response = tower::ServiceExt::oneshot(
        app,
        axum::http::Request::builder()
            .uri("/api/v1/health")
            .body(axum::body::Body::empty())
            .unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);
}

#[tokio::test]
#[verifies(REQ-OPS-001, case = "endpoint public accessible anonymement")]
#[allow(non_snake_case)]
async fn authz_other_getApiV1Health() {
    let app = app();
    let response = tower::ServiceExt::oneshot(
        app,
        axum::http::Request::builder()
            .uri("/api/v1/health")
            .body(axum::body::Body::empty())
            .unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);
}

#[tokio::test]
#[verifies(REQ-OPS-001, case = "endpoint public accessible anonymement")]
#[allow(non_snake_case)]
async fn authz_anon_getApiV1Health() {
    let app = app();
    let response = tower::ServiceExt::oneshot(
        app,
        axum::http::Request::builder()
            .uri("/api/v1/health")
            .body(axum::body::Body::empty())
            .unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);
}
