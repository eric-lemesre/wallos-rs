//! Tests d'intégration des jetons d'appareil (REQ-AUT-005).
//!
//! Un appairage (`POST /device-sessions`) émet un jeton d'appareil opaque dans le **corps** (et non
//! un cookie) ; ce jeton, présenté en `Authorization: Bearer`, authentifie les requêtes suivantes.

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use serde_json::json;
use sqlx::PgPool;
use tower::ServiceExt;
use wallos_req_macros::verifies;
use wallos_server::app_with_db;
use wallos_storage::Db;

const PASSWORD: &str = "correct horse battery staple";

fn app(pool: PgPool) -> Router {
    app_with_db(Db::from_pool(pool))
}

async fn post(pool: &PgPool, uri: &str, body: serde_json::Value) -> axum::http::Response<Body> {
    app(pool.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn signup(pool: &PgPool, email: &str) {
    let response = post(
        pool,
        "/api/v1/accounts",
        json!({ "email": email, "password": PASSWORD }),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
}

/// Appaire un appareil et renvoie la réponse brute de `POST /device-sessions`.
async fn pair_device(
    pool: &PgPool,
    email: &str,
    password: &str,
    label: &str,
    platform: &str,
) -> axum::http::Response<Body> {
    post(
        pool,
        "/api/v1/device-sessions",
        json!({ "email": email, "password": password, "label": label, "platform": platform }),
    )
    .await
}

/// Extrait le jeton d'appareil du corps JSON d'une réponse d'appairage réussie.
async fn device_token(response: axum::http::Response<Body>) -> String {
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    body["token"].as_str().unwrap().to_string()
}

/// `GET /me` en présentant un jeton d'appareil via `Authorization: Bearer`.
async fn get_me_bearer(pool: &PgPool, token: &str) -> axum::http::Response<Body> {
    app(pool.clone())
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/me")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
}

// --- Parcours fonctionnels ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-AUT-005)]
async fn pairing_emits_device_token_usable_as_bearer(pool: PgPool) {
    signup(&pool, "lea@example.com").await;

    let paired = pair_device(
        &pool,
        "lea@example.com",
        PASSWORD,
        "MacBook de Léa",
        "desktop",
    )
    .await;
    assert_eq!(paired.status(), StatusCode::OK);
    let token = device_token(paired).await;
    assert!(!token.is_empty());

    // Le jeton d'appareil authentifie `/me` — sans aucun cookie.
    let me = get_me_bearer(&pool, &token).await;
    assert_eq!(me.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(me.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["email"], "lea@example.com");
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-AUT-005)]
async fn pairing_with_wrong_password_is_rejected(pool: PgPool) {
    signup(&pool, "nate@example.com").await;
    let rejected = pair_device(
        &pool,
        "nate@example.com",
        "not the password",
        "Phone",
        "mobile",
    )
    .await;
    assert_eq!(rejected.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-AUT-005)]
async fn invalid_bearer_token_is_unauthorized(pool: PgPool) {
    // Un jeton inconnu ne donne aucun accès.
    assert_eq!(
        get_me_bearer(&pool, "definitely-not-a-real-token")
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
}

// --- Autorisation §9 : createDeviceSession (public, comme createSession) ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-AUT-005)]
async fn authz_owner_create_device_session(pool: PgPool) {
    signup(&pool, "owner-dev@example.com").await;
    assert_eq!(
        pair_device(
            &pool,
            "owner-dev@example.com",
            PASSWORD,
            "Laptop",
            "desktop"
        )
        .await
        .status(),
        StatusCode::OK
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-AUT-005)]
async fn authz_other_create_device_session(pool: PgPool) {
    // Endpoint public : un autre compte peut aussi appairer son propre appareil.
    signup(&pool, "other-dev@example.com").await;
    assert_eq!(
        pair_device(
            &pool,
            "other-dev@example.com",
            PASSWORD,
            "Laptop",
            "desktop"
        )
        .await
        .status(),
        StatusCode::OK
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-AUT-005)]
async fn authz_anon_create_device_session(pool: PgPool) {
    // Public : appelable sans session préalable (c'est l'authentification elle-même).
    signup(&pool, "anon-dev@example.com").await;
    assert_eq!(
        pair_device(&pool, "anon-dev@example.com", PASSWORD, "Laptop", "desktop")
            .await
            .status(),
        StatusCode::OK
    );
}
