//! Tests d'intégration de la suppression d'abonnement (REQ-SUB-005).
//!
//! Suppression **traçable** : l'abonnement disparaît de toutes les vues et une pierre tombale est créée
//! (REQ-SYN-002) pour qu'un autre appareil applique la suppression. Isolation §9 : propriétaire 204,
//! tiers 404 (jamais 403, l'abonnement d'autrui reste intact), anonyme 401.

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use serde_json::{Value, json};
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
    body: Option<Value>,
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

async fn body_json(resp: axum::http::Response<Body>) -> Value {
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
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

/// Crée un abonnement et renvoie son id.
async fn create_sub(pool: &PgPool, cookie: &str, name: &str) -> String {
    let r = send(
        pool,
        "POST",
        "/api/v1/subscriptions",
        Some(cookie),
        Some(json!({
            "name": name, "amount": "9.99", "currency": "EUR",
            "cycle": { "unit": "month", "interval": 1 }, "first_payment": "2030-01-15"
        })),
    )
    .await;
    assert_eq!(r.status(), StatusCode::CREATED);
    body_json(r).await["id"].as_str().unwrap().to_string()
}

/// Noms des abonnements listés pour le foyer de l'appelant.
async fn list_names(pool: &PgPool, cookie: &str) -> Vec<String> {
    let body =
        body_json(send(pool, "GET", "/api/v1/subscriptions", Some(cookie), None).await).await;
    body["subscriptions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["name"].as_str().unwrap().to_string())
        .collect()
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-005, case = "l'abonnement supprimé disparaît de la liste")]
async fn delete_removes_from_all_views(pool: PgPool) {
    let web = account(&pool, "sub005-list@example.com").await;
    let id = create_sub(&pool, &web, "Netflix").await;
    assert!(
        list_names(&pool, &web)
            .await
            .contains(&"Netflix".to_string())
    );

    assert_eq!(
        send(
            &pool,
            "DELETE",
            &format!("/api/v1/subscriptions/{id}"),
            Some(&web),
            None
        )
        .await
        .status(),
        StatusCode::NO_CONTENT
    );
    // Disparaît de la liste (et de toute vue dérivée).
    assert!(list_names(&pool, &web).await.is_empty());
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-005, case = "la suppression crée une pierre tombale (REQ-SYN-002)")]
async fn delete_creates_a_tombstone(pool: PgPool) {
    let web = account(&pool, "sub005-tomb@example.com").await;
    let id = create_sub(&pool, &web, "Netflix").await;
    send(
        &pool,
        "DELETE",
        &format!("/api/v1/subscriptions/{id}"),
        Some(&web),
        None,
    )
    .await;

    let body =
        body_json(send(&pool, "GET", "/api/v1/sync/tombstones", Some(&web), None).await).await;
    let list = body["tombstones"].as_array().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0]["entity_type"], "subscription");
    assert_eq!(list[0]["entity_id"], id);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-005, case = "supprimer un abonnement inconnu -> 404")]
async fn deleting_unknown_is_not_found(pool: PgPool) {
    let web = account(&pool, "sub005-unknown@example.com").await;
    let random = uuid::Uuid::new_v4();
    assert_eq!(
        send(
            &pool,
            "DELETE",
            &format!("/api/v1/subscriptions/{random}"),
            Some(&web),
            None
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    // Un identifiant non-UUID est également traité comme inconnu (jamais 500).
    assert_eq!(
        send(
            &pool,
            "DELETE",
            "/api/v1/subscriptions/not-a-uuid",
            Some(&web),
            None
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
}

// --- Autorisation (§9) ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-005)]
async fn authz_owner_delete_subscription(pool: PgPool) {
    let web = account(&pool, "own-sub005@example.com").await;
    let id = create_sub(&pool, &web, "Mine").await;
    assert_eq!(
        send(
            &pool,
            "DELETE",
            &format!("/api/v1/subscriptions/{id}"),
            Some(&web),
            None
        )
        .await
        .status(),
        StatusCode::NO_CONTENT
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-005)]
async fn authz_other_delete_subscription(pool: PgPool) {
    // Le foyer A crée un abonnement ; B ne peut pas le supprimer (404, jamais 403) et il reste intact.
    let a = account(&pool, "a-sub005@example.com").await;
    let id = create_sub(&pool, &a, "A-only").await;
    let b = account(&pool, "b-sub005@example.com").await;
    assert_eq!(
        send(
            &pool,
            "DELETE",
            &format!("/api/v1/subscriptions/{id}"),
            Some(&b),
            None
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    // L'abonnement de A est toujours présent.
    assert!(list_names(&pool, &a).await.contains(&"A-only".to_string()));
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-005)]
async fn authz_anon_delete_subscription(pool: PgPool) {
    let web = account(&pool, "anon-sub005@example.com").await;
    let id = create_sub(&pool, &web, "Mine").await;
    assert_eq!(
        send(
            &pool,
            "DELETE",
            &format!("/api/v1/subscriptions/{id}"),
            None,
            None
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
}
