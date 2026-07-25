//! Tests d'intégration du format d'erreur RFC 9457 (`application/problem+json`).

use wallos_req_macros::verifies;
use wallos_server::app;

#[tokio::test]
#[verifies(REQ-SEC-002)]
async fn unknown_route_returns_problem_json_404() {
    let response = tower::ServiceExt::oneshot(
        app(),
        axum::http::Request::builder()
            .uri("/api/v1/does-not-exist")
            .body(axum::body::Body::empty())
            .unwrap(),
    )
    .await
    .unwrap();

    assert_eq!(response.status(), axum::http::StatusCode::NOT_FOUND);

    let content_type = response
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    assert_eq!(content_type, "application/problem+json");

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let problem: serde_json::Value = serde_json::from_slice(&body).unwrap();

    // Champs RFC 9457.
    assert_eq!(problem["status"], 404);
    assert!(problem["type"].is_string());
    assert!(problem["title"].is_string());
    assert!(problem["instance"].is_string());
}
