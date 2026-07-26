//! Tests d'intégration du changement de mot de passe (REQ-AUT-007).
//!
//! Un changement réussi remplace le hash ET **coupe tous les autres accès** (sessions + jetons
//! d'appareil) sauf la crédential courante. Un mot de passe actuel incorrect renvoie `403` sans
//! modifier quoi que ce soit.

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
const NEW_PASSWORD: &str = "totally fresh secret passphrase";

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
    assert_eq!(
        post(
            pool,
            "/api/v1/accounts",
            json!({ "email": email, "password": PASSWORD })
        )
        .await
        .status(),
        StatusCode::CREATED
    );
}

/// Ouvre une session et renvoie la réponse (pour en extraire le cookie).
async fn login(pool: &PgPool, email: &str, password: &str) -> axum::http::Response<Body> {
    post(
        pool,
        "/api/v1/sessions",
        json!({ "email": email, "password": password }),
    )
    .await
}

fn session_cookie(response: &axum::http::Response<Body>) -> String {
    response
        .headers()
        .get(header::SET_COOKIE)
        .and_then(|v| v.to_str().ok())
        .expect("login sets a session cookie")
        .split(';')
        .next()
        .unwrap()
        .to_string()
}

/// Appaire un appareil et renvoie son jeton.
async fn pair_device_token(pool: &PgPool, email: &str) -> String {
    let response = post(
        pool,
        "/api/v1/device-sessions",
        json!({ "email": email, "password": PASSWORD, "label": "Laptop", "platform": "desktop" }),
    )
    .await;
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    body["token"].as_str().unwrap().to_string()
}

async fn get_me(pool: &PgPool, auth: (axum::http::HeaderName, String)) -> StatusCode {
    app(pool.clone())
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/me")
                .header(auth.0, auth.1)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
        .status()
}

fn with_cookie(c: &str) -> (axum::http::HeaderName, String) {
    (header::COOKIE, c.to_string())
}

fn with_bearer(t: &str) -> (axum::http::HeaderName, String) {
    (header::AUTHORIZATION, format!("Bearer {t}"))
}

/// `PUT /password` avec un cookie de session.
async fn change_password(
    pool: &PgPool,
    cookie: Option<&str>,
    current: &str,
    new: &str,
) -> axum::http::Response<Body> {
    let mut builder = Request::builder()
        .method("PUT")
        .uri("/api/v1/password")
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(cookie) = cookie {
        builder = builder.header(header::COOKIE, cookie);
    }
    app(pool.clone())
        .oneshot(
            builder
                .body(Body::from(
                    json!({ "current_password": current, "new_password": new }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap()
}

// --- Parcours fonctionnels ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-AUT-007)]
async fn wrong_current_password_is_forbidden_and_changes_nothing(pool: PgPool) {
    signup(&pool, "keep@example.com").await;
    let cookie = session_cookie(&login(&pool, "keep@example.com", PASSWORD).await);

    let rejected = change_password(&pool, Some(&cookie), "not my password", NEW_PASSWORD).await;
    assert_eq!(rejected.status(), StatusCode::FORBIDDEN);

    // Aucun changement : l'ancien mot de passe fonctionne toujours, le « nouveau » non.
    assert_eq!(
        login(&pool, "keep@example.com", PASSWORD).await.status(),
        StatusCode::OK
    );
    assert_eq!(
        login(&pool, "keep@example.com", NEW_PASSWORD)
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-AUT-007)]
async fn new_password_must_conform_to_policy(pool: PgPool) {
    signup(&pool, "weak@example.com").await;
    let cookie = session_cookie(&login(&pool, "weak@example.com", PASSWORD).await);
    // Trop court (< 12) : rejeté côté serveur, aucun changement.
    let rejected = change_password(&pool, Some(&cookie), PASSWORD, "short").await;
    assert_eq!(rejected.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-AUT-007)]
async fn changing_password_cuts_other_sessions_and_devices_but_keeps_current(pool: PgPool) {
    signup(&pool, "rotate@example.com").await;
    let cookie_a = session_cookie(&login(&pool, "rotate@example.com", PASSWORD).await);
    let cookie_b = session_cookie(&login(&pool, "rotate@example.com", PASSWORD).await);
    let device = pair_device_token(&pool, "rotate@example.com").await;

    // Toutes les crédentiales valent avant le changement.
    assert_eq!(get_me(&pool, with_cookie(&cookie_a)).await, StatusCode::OK);
    assert_eq!(get_me(&pool, with_cookie(&cookie_b)).await, StatusCode::OK);
    assert_eq!(get_me(&pool, with_bearer(&device)).await, StatusCode::OK);

    // Changement depuis la session A.
    assert_eq!(
        change_password(&pool, Some(&cookie_a), PASSWORD, NEW_PASSWORD)
            .await
            .status(),
        StatusCode::NO_CONTENT
    );

    // La session courante (A) survit ; l'autre session (B) et le jeton d'appareil sont coupés.
    assert_eq!(get_me(&pool, with_cookie(&cookie_a)).await, StatusCode::OK);
    assert_eq!(
        get_me(&pool, with_cookie(&cookie_b)).await,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        get_me(&pool, with_bearer(&device)).await,
        StatusCode::UNAUTHORIZED
    );

    // Le nouveau mot de passe authentifie ; l'ancien non.
    assert_eq!(
        login(&pool, "rotate@example.com", NEW_PASSWORD)
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        login(&pool, "rotate@example.com", PASSWORD).await.status(),
        StatusCode::UNAUTHORIZED
    );
}

// --- Autorisation §9 : changePassword (protégé) ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-AUT-007)]
async fn authz_owner_change_password(pool: PgPool) {
    signup(&pool, "owner-pw@example.com").await;
    let cookie = session_cookie(&login(&pool, "owner-pw@example.com", PASSWORD).await);
    assert_eq!(
        change_password(&pool, Some(&cookie), PASSWORD, NEW_PASSWORD)
            .await
            .status(),
        StatusCode::NO_CONTENT
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-AUT-007)]
async fn authz_other_change_password(pool: PgPool) {
    // Un autre compte change SON propre mot de passe : autorisé (endpoint self-scoped).
    signup(&pool, "first-pw@example.com").await;
    signup(&pool, "second-pw@example.com").await;
    let cookie = session_cookie(&login(&pool, "second-pw@example.com", PASSWORD).await);
    assert_eq!(
        change_password(&pool, Some(&cookie), PASSWORD, NEW_PASSWORD)
            .await
            .status(),
        StatusCode::NO_CONTENT
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-AUT-007)]
async fn authz_anon_change_password(pool: PgPool) {
    // Sans authentification : 401 (jamais 403 : on ne révèle rien).
    assert_eq!(
        change_password(&pool, None, PASSWORD, NEW_PASSWORD)
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
}
