//! Tests d'intégration des moyens de paiement (REQ-SUB-011).
//!
//! CRUD isolé par foyer : create/list/rename/delete n'affectent que les moyens de l'appelant.
//! Autorisation §9 : propriétaire 2xx, tiers authentifié 404 (jamais 403), anonyme 401. Calque CAT-001.

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

async fn send(
    pool: &PgPool,
    method: &str,
    uri: &str,
    cookie: Option<&str>,
    body: Option<serde_json::Value>,
) -> axum::http::Response<Body> {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(c) = cookie {
        builder = builder.header(header::COOKIE, c);
    }
    let body = match body {
        Some(v) => {
            builder = builder.header(header::CONTENT_TYPE, "application/json");
            Body::from(v.to_string())
        }
        None => Body::empty(),
    };
    app(pool.clone())
        .oneshot(builder.body(body).unwrap())
        .await
        .unwrap()
}

async fn account(pool: &PgPool, email: &str) -> String {
    assert_eq!(
        send(
            pool,
            "POST",
            "/api/v1/accounts",
            None,
            Some(json!({ "email": email, "password": PASSWORD })),
        )
        .await
        .status(),
        StatusCode::CREATED
    );
    let r = send(
        pool,
        "POST",
        "/api/v1/sessions",
        None,
        Some(json!({ "email": email, "password": PASSWORD })),
    )
    .await;
    r.headers()
        .get(header::SET_COOKIE)
        .and_then(|v| v.to_str().ok())
        .expect("cookie")
        .split(';')
        .next()
        .unwrap()
        .to_string()
}

async fn create(pool: &PgPool, cookie: &str, name: &str) -> axum::http::Response<Body> {
    send(
        pool,
        "POST",
        "/api/v1/payment-methods",
        Some(cookie),
        Some(json!({ "name": name })),
    )
    .await
}

async fn list(pool: &PgPool, cookie: &str) -> Vec<serde_json::Value> {
    let r = send(pool, "GET", "/api/v1/payment-methods", Some(cookie), None).await;
    let bytes = axum::body::to_bytes(r.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

async fn created_id(r: axum::http::Response<Body>) -> String {
    let bytes = axum::body::to_bytes(r.into_body(), usize::MAX)
        .await
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    v["id"].as_str().unwrap().to_string()
}

// --- Parcours fonctionnels ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-011)]
async fn crud_round_trip(pool: PgPool) {
    let web = account(&pool, "pm@example.com").await;
    // Création, disponible immédiatement dans la liste.
    let id = created_id(create(&pool, &web, "Carte de crédit").await).await;
    let items = list(&pool, &web).await;
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["name"], "Carte de crédit");

    // Renommage.
    let r = send(
        &pool,
        "PUT",
        &format!("/api/v1/payment-methods/{id}"),
        Some(&web),
        Some(json!({ "name": "PayPal" })),
    )
    .await;
    assert_eq!(r.status(), StatusCode::OK);
    assert_eq!(list(&pool, &web).await[0]["name"], "PayPal");

    // Suppression.
    let r = send(
        &pool,
        "DELETE",
        &format!("/api/v1/payment-methods/{id}"),
        Some(&web),
        None,
    )
    .await;
    assert_eq!(r.status(), StatusCode::NO_CONTENT);
    assert!(list(&pool, &web).await.is_empty());
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-011)]
async fn empty_name_is_rejected(pool: PgPool) {
    let web = account(&pool, "pm-empty@example.com").await;
    assert_eq!(
        create(&pool, &web, "   ").await.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-011)]
async fn edge_cases_are_rejected(pool: PgPool) {
    // Revue SUB-011 #4 : nom trop long (> 100) refusé ; UUID malformé sur rename/delete -> 404.
    let web = account(&pool, "pm-edge@example.com").await;
    assert_eq!(
        create(&pool, &web, &"a".repeat(101)).await.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        send(
            &pool,
            "PUT",
            "/api/v1/payment-methods/not-a-uuid",
            Some(&web),
            Some(json!({ "name": "X" })),
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        send(
            &pool,
            "DELETE",
            "/api/v1/payment-methods/not-a-uuid",
            Some(&web),
            None,
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
}

// --- Autorisation §9 : createPaymentMethod ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-011)]
async fn authz_owner_create_payment_method(pool: PgPool) {
    let web = account(&pool, "own-cpm@example.com").await;
    assert_eq!(
        create(&pool, &web, "Carte").await.status(),
        StatusCode::CREATED
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-011)]
async fn authz_other_create_payment_method(pool: PgPool) {
    // Chaque compte crée ses propres moyens de paiement.
    let web = account(&pool, "other-cpm@example.com").await;
    assert_eq!(
        create(&pool, &web, "Carte").await.status(),
        StatusCode::CREATED
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-011)]
async fn authz_anon_create_payment_method(pool: PgPool) {
    assert_eq!(
        send(
            &pool,
            "POST",
            "/api/v1/payment-methods",
            None,
            Some(json!({ "name": "Carte" })),
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
}

// --- Autorisation §9 : listPaymentMethods ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-011)]
async fn authz_owner_list_payment_methods(pool: PgPool) {
    let web = account(&pool, "own-lpm@example.com").await;
    assert_eq!(
        send(&pool, "GET", "/api/v1/payment-methods", Some(&web), None)
            .await
            .status(),
        StatusCode::OK
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-011)]
async fn authz_other_list_payment_methods(pool: PgPool) {
    // Un autre foyer ne voit jamais les moyens d'autrui : sa liste est la sienne (vide ici).
    let owner = account(&pool, "owner-lpm@example.com").await;
    let _ = create(&pool, &owner, "Secret").await;
    let other = account(&pool, "other-lpm@example.com").await;
    assert!(list(&pool, &other).await.is_empty());
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-011)]
async fn authz_anon_list_payment_methods(pool: PgPool) {
    assert_eq!(
        send(&pool, "GET", "/api/v1/payment-methods", None, None)
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
}

// --- Autorisation §9 : renamePaymentMethod ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-011)]
async fn authz_owner_rename_payment_method(pool: PgPool) {
    let web = account(&pool, "own-rpm@example.com").await;
    let id = created_id(create(&pool, &web, "Carte").await).await;
    assert_eq!(
        send(
            &pool,
            "PUT",
            &format!("/api/v1/payment-methods/{id}"),
            Some(&web),
            Some(json!({ "name": "PayPal" })),
        )
        .await
        .status(),
        StatusCode::OK
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-011)]
async fn authz_other_rename_payment_method(pool: PgPool) {
    // Le moyen d'un autre foyer est traité comme inexistant : 404 (jamais 403), non modifié.
    let owner = account(&pool, "owner-rpm@example.com").await;
    let id = created_id(create(&pool, &owner, "Carte").await).await;
    let other = account(&pool, "other-rpm@example.com").await;
    assert_eq!(
        send(
            &pool,
            "PUT",
            &format!("/api/v1/payment-methods/{id}"),
            Some(&other),
            Some(json!({ "name": "Piraté" })),
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(list(&pool, &owner).await[0]["name"], "Carte");
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-011)]
async fn authz_anon_rename_payment_method(pool: PgPool) {
    assert_eq!(
        send(
            &pool,
            "PUT",
            "/api/v1/payment-methods/00000000-0000-0000-0000-000000000001",
            None,
            Some(json!({ "name": "X" })),
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
}

// --- Autorisation §9 : deletePaymentMethod ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-011)]
async fn authz_owner_delete_payment_method(pool: PgPool) {
    let web = account(&pool, "own-dpm@example.com").await;
    let id = created_id(create(&pool, &web, "Carte").await).await;
    assert_eq!(
        send(
            &pool,
            "DELETE",
            &format!("/api/v1/payment-methods/{id}"),
            Some(&web),
            None,
        )
        .await
        .status(),
        StatusCode::NO_CONTENT
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-011)]
async fn authz_other_delete_payment_method(pool: PgPool) {
    let owner = account(&pool, "owner-dpm@example.com").await;
    let id = created_id(create(&pool, &owner, "Carte").await).await;
    let other = account(&pool, "other-dpm@example.com").await;
    assert_eq!(
        send(
            &pool,
            "DELETE",
            &format!("/api/v1/payment-methods/{id}"),
            Some(&other),
            None,
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    // Toujours présent chez le propriétaire.
    assert_eq!(list(&pool, &owner).await.len(), 1);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-011)]
async fn authz_anon_delete_payment_method(pool: PgPool) {
    assert_eq!(
        send(
            &pool,
            "DELETE",
            "/api/v1/payment-methods/00000000-0000-0000-0000-000000000001",
            None,
            None,
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
}
