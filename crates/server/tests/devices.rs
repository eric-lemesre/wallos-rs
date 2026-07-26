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

// ============================================================================
// REQ-AUT-006 — liste et révocation des appareils
// ============================================================================

/// Ouvre une session web et renvoie le cookie `session=...`.
async fn login_cookie(pool: &PgPool, email: &str) -> String {
    let response = post(
        pool,
        "/api/v1/sessions",
        json!({ "email": email, "password": PASSWORD }),
    )
    .await;
    let set_cookie = response
        .headers()
        .get(header::SET_COOKIE)
        .and_then(|v| v.to_str().ok())
        .expect("login sets a session cookie");
    set_cookie.split(';').next().unwrap().to_string()
}

/// Émet une requête `GET`/`DELETE` sur `uri` avec un en-tête d'auth optionnel (`Cookie` ou `Authorization`).
async fn send(
    pool: &PgPool,
    method: &str,
    uri: &str,
    auth: Option<(axum::http::HeaderName, String)>,
) -> axum::http::Response<Body> {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some((name, value)) = auth {
        builder = builder.header(name, value);
    }
    app(pool.clone())
        .oneshot(builder.body(Body::empty()).unwrap())
        .await
        .unwrap()
}

fn cookie(c: &str) -> Option<(axum::http::HeaderName, String)> {
    Some((header::COOKIE, c.to_string()))
}

fn bearer(token: &str) -> Option<(axum::http::HeaderName, String)> {
    Some((header::AUTHORIZATION, format!("Bearer {token}")))
}

/// Corps JSON d'une liste d'appareils.
async fn devices_json(response: axum::http::Response<Body>) -> Vec<serde_json::Value> {
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

// --- Parcours fonctionnels ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-AUT-006)]
async fn list_shows_label_platform_and_marks_current_device(pool: PgPool) {
    signup(&pool, "multi@example.com").await;
    let t1 =
        device_token(pair_device(&pool, "multi@example.com", PASSWORD, "Laptop", "desktop").await)
            .await;
    let _t2 =
        device_token(pair_device(&pool, "multi@example.com", PASSWORD, "Phone", "mobile").await)
            .await;

    // Liste vue DEPUIS l'appareil 1 (Bearer t1) : deux appareils, seul « Laptop » est courant.
    let list = send(&pool, "GET", "/api/v1/devices", bearer(&t1)).await;
    assert_eq!(list.status(), StatusCode::OK);
    let devices = devices_json(list).await;
    assert_eq!(devices.len(), 2);
    let laptop = devices.iter().find(|d| d["label"] == "Laptop").unwrap();
    let phone = devices.iter().find(|d| d["label"] == "Phone").unwrap();
    assert_eq!(laptop["platform"], "desktop");
    assert_eq!(laptop["current"], true);
    assert_eq!(phone["current"], false);
    assert!(laptop["last_seen_at"].as_str().unwrap().contains('T'));
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-AUT-006)]
async fn revoking_a_device_invalidates_its_token_immediately(pool: PgPool) {
    signup(&pool, "revoke@example.com").await;
    let token = device_token(
        pair_device(
            &pool,
            "revoke@example.com",
            PASSWORD,
            "Old Laptop",
            "desktop",
        )
        .await,
    )
    .await;

    // Le jeton fonctionne avant révocation.
    assert_eq!(get_me_bearer(&pool, &token).await.status(), StatusCode::OK);

    // Révocation via la session web (l'utilisateur retire un appareil depuis le navigateur).
    let web = login_cookie(&pool, "revoke@example.com").await;
    let devices = devices_json(send(&pool, "GET", "/api/v1/devices", cookie(&web)).await).await;
    let id = devices[0]["id"].as_str().unwrap();
    assert_eq!(
        send(
            &pool,
            "DELETE",
            &format!("/api/v1/devices/{id}"),
            cookie(&web)
        )
        .await
        .status(),
        StatusCode::NO_CONTENT
    );

    // Révocation immédiate : l'ancien jeton ne vaut plus rien, sans délai de propagation.
    assert_eq!(
        get_me_bearer(&pool, &token).await.status(),
        StatusCode::UNAUTHORIZED
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-AUT-006)]
async fn revoking_a_malformed_device_id_is_not_found(pool: PgPool) {
    // Un identifiant mal formé (pas un UUID) est traité comme inexistant : 404, jamais 400/500.
    signup(&pool, "malformed@example.com").await;
    let web = login_cookie(&pool, "malformed@example.com").await;
    assert_eq!(
        send(&pool, "DELETE", "/api/v1/devices/not-a-uuid", cookie(&web))
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
}

// --- Autorisation §9 : listDevices (protégé) ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-AUT-006)]
async fn authz_owner_list_devices(pool: PgPool) {
    signup(&pool, "owner-list@example.com").await;
    let web = login_cookie(&pool, "owner-list@example.com").await;
    assert_eq!(
        send(&pool, "GET", "/api/v1/devices", cookie(&web))
            .await
            .status(),
        StatusCode::OK
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-AUT-006)]
async fn authz_other_list_devices(pool: PgPool) {
    // L'appareil d'un compte n'apparaît jamais dans la liste d'un autre foyer.
    signup(&pool, "first-list@example.com").await;
    signup(&pool, "second-list@example.com").await;
    let _ = pair_device(
        &pool,
        "first-list@example.com",
        PASSWORD,
        "First Laptop",
        "desktop",
    )
    .await;
    let web = login_cookie(&pool, "second-list@example.com").await;
    let devices = devices_json(send(&pool, "GET", "/api/v1/devices", cookie(&web)).await).await;
    assert!(devices.is_empty());
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-AUT-006)]
async fn authz_anon_list_devices(pool: PgPool) {
    assert_eq!(
        send(&pool, "GET", "/api/v1/devices", None).await.status(),
        StatusCode::UNAUTHORIZED
    );
}

// --- Autorisation §9 : revokeDevice (protégé) ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-AUT-006)]
async fn authz_owner_revoke_device(pool: PgPool) {
    signup(&pool, "owner-rev@example.com").await;
    let _ = pair_device(
        &pool,
        "owner-rev@example.com",
        PASSWORD,
        "Laptop",
        "desktop",
    )
    .await;
    let web = login_cookie(&pool, "owner-rev@example.com").await;
    let devices = devices_json(send(&pool, "GET", "/api/v1/devices", cookie(&web)).await).await;
    let id = devices[0]["id"].as_str().unwrap();
    assert_eq!(
        send(
            &pool,
            "DELETE",
            &format!("/api/v1/devices/{id}"),
            cookie(&web)
        )
        .await
        .status(),
        StatusCode::NO_CONTENT
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-AUT-006)]
async fn authz_other_revoke_device(pool: PgPool) {
    // Révoquer l'appareil d'un autre foyer -> 404 (jamais 403 : ne divulgue pas l'existence).
    signup(&pool, "first-rev@example.com").await;
    signup(&pool, "second-rev@example.com").await;
    let owner_web = login_cookie(&pool, "first-rev@example.com").await;
    let _ = pair_device(
        &pool,
        "first-rev@example.com",
        PASSWORD,
        "Laptop",
        "desktop",
    )
    .await;
    let id = devices_json(send(&pool, "GET", "/api/v1/devices", cookie(&owner_web)).await).await[0]
        ["id"]
        .as_str()
        .unwrap()
        .to_string();

    let attacker_web = login_cookie(&pool, "second-rev@example.com").await;
    assert_eq!(
        send(
            &pool,
            "DELETE",
            &format!("/api/v1/devices/{id}"),
            cookie(&attacker_web)
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-AUT-006)]
async fn authz_anon_revoke_device(pool: PgPool) {
    let id = uuid::Uuid::new_v4();
    assert_eq!(
        send(&pool, "DELETE", &format!("/api/v1/devices/{id}"), None)
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
}
