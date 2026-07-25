//! Tests d'intégration de l'endpoint de santé versionné.

use wallos_req_macros::verifies;
use wallos_server::app;

/// Émet une requête GET sur `uri` contre l'application et renvoie la réponse.
async fn get(uri: &str) -> axum::http::Response<axum::body::Body> {
    tower::ServiceExt::oneshot(
        app(),
        axum::http::Request::builder()
            .uri(uri)
            .body(axum::body::Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

#[tokio::test]
#[verifies(REQ-OPS-001)]
async fn get_health_returns_ok() {
    let response = get("/api/v1/health").await;

    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "ok");
    assert_eq!(json["service"], "wallos-rs");
}

#[tokio::test]
#[verifies(REQ-OPS-001)]
async fn authz_owner_get_health() {
    // L'endpoint de santé est public : accessible sans contexte d'appelant.
    assert_eq!(
        get("/api/v1/health").await.status(),
        axum::http::StatusCode::OK
    );
}

#[tokio::test]
#[verifies(REQ-OPS-001)]
async fn authz_other_get_health() {
    assert_eq!(
        get("/api/v1/health").await.status(),
        axum::http::StatusCode::OK
    );
}

#[tokio::test]
#[verifies(REQ-OPS-001)]
async fn authz_anon_get_health() {
    assert_eq!(
        get("/api/v1/health").await.status(),
        axum::http::StatusCode::OK
    );
}
