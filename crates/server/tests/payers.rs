//! Tests d'intégration des payeurs (REQ-SUB-017).
//!
//! CRUD isolé par foyer ; rattachement d'un abonnement à un payeur reflété dans la vue filtrée ;
//! **suppression d'un payeur référencé refusée** (409, oracle Wallos `household_in_use`). Autorisation
//! §9 : propriétaire 2xx, tiers authentifié 404 (jamais 403), anonyme 401.

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
    let r = send(
        pool,
        "POST",
        "/api/v1/accounts",
        None,
        Some(json!({ "email": email, "password": PASSWORD })),
    )
    .await;
    assert_eq!(r.status(), StatusCode::CREATED);
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

/// Crée un payeur et renvoie son id.
async fn create_payer(pool: &PgPool, cookie: &str, name: &str) -> String {
    let r = send(
        pool,
        "POST",
        "/api/v1/payers",
        Some(cookie),
        Some(json!({ "name": name })),
    )
    .await;
    assert_eq!(r.status(), StatusCode::CREATED);
    body_json(r).await["id"].as_str().unwrap().to_string()
}

/// Crée un abonnement, éventuellement rattaché à un payeur.
async fn create_subscription(
    pool: &PgPool,
    cookie: &str,
    name: &str,
    payer: Option<&str>,
) -> axum::http::Response<Body> {
    let mut body = json!({
        "name": name,
        "amount": "9.99",
        "currency": "EUR",
        "cycle": { "unit": "month", "interval": 1 },
        "first_payment": "2030-01-15",
    });
    if let Some(p) = payer {
        body["payer"] = json!(p);
    }
    send(
        pool,
        "POST",
        "/api/v1/subscriptions",
        Some(cookie),
        Some(body),
    )
    .await
}

// --- CRUD ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-017, case = "création + liste ordonnée")]
async fn create_and_list_payers(pool: PgPool) {
    let a = account(&pool, "a@example.com").await;
    create_payer(&pool, &a, "Sam").await;
    create_payer(&pool, &a, "Alex").await;
    let list = body_json(send(&pool, "GET", "/api/v1/payers", Some(&a), None).await).await;
    let names: Vec<&str> = list
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|p| p["name"].as_str())
        .collect();
    assert_eq!(names, vec!["Alex", "Sam"]); // ordre nom asc
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-017, case = "renommage puis suppression d'un payeur non référencé (204)")]
async fn rename_and_delete_unreferenced_payer(pool: PgPool) {
    let a = account(&pool, "a@example.com").await;
    let id = create_payer(&pool, &a, "Sam").await;
    let renamed = send(
        &pool,
        "PUT",
        &format!("/api/v1/payers/{id}"),
        Some(&a),
        Some(json!({ "name": "Samantha" })),
    )
    .await;
    assert_eq!(renamed.status(), StatusCode::OK);
    assert_eq!(body_json(renamed).await["name"], "Samantha");

    let deleted = send(
        &pool,
        "DELETE",
        &format!("/api/v1/payers/{id}"),
        Some(&a),
        None,
    )
    .await;
    assert_eq!(deleted.status(), StatusCode::NO_CONTENT);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-017, case = "abonnement rattaché à un payeur reflété dans la vue filtrée")]
async fn subscription_attached_to_payer_is_reflected(pool: PgPool) {
    let a = account(&pool, "a@example.com").await;
    let payer = create_payer(&pool, &a, "Alex").await;
    assert_eq!(
        create_subscription(&pool, &a, "Netflix", Some(&payer))
            .await
            .status(),
        StatusCode::CREATED
    );
    // La vue des abonnements filtrée par ce payeur contient l'abonnement rattaché.
    let filtered = body_json(
        send(
            &pool,
            "GET",
            &format!("/api/v1/subscriptions?payer={payer}"),
            Some(&a),
            None,
        )
        .await,
    )
    .await;
    let names: Vec<&str> = filtered["subscriptions"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|s| s["name"].as_str())
        .collect();
    assert_eq!(names, vec!["Netflix"]);
}

// --- Oracle legacy : suppression d'un payeur référencé (REQ-SUB-017-payer.json) ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-017, case = "payeur référencé -> suppression refusée (409), payeur intact")]
async fn referenced_payer_cannot_be_deleted(pool: PgPool) {
    let a = account(&pool, "a@example.com").await;
    let payer = create_payer(&pool, &a, "Alex").await;
    create_subscription(&pool, &a, "Netflix", Some(&payer)).await;

    // Refus : 409 (jamais 204, jamais 404) — comportement capturé sur Wallos (`household_in_use`).
    let deleted = send(
        &pool,
        "DELETE",
        &format!("/api/v1/payers/{payer}"),
        Some(&a),
        None,
    )
    .await;
    assert_eq!(deleted.status(), StatusCode::CONFLICT);
    // Le payeur reste présent.
    let list = body_json(send(&pool, "GET", "/api/v1/payers", Some(&a), None).await).await;
    assert_eq!(list.as_array().unwrap().len(), 1);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-017, case = "payeur redevient supprimable quand plus référencé")]
async fn payer_deletable_once_unreferenced(pool: PgPool) {
    let a = account(&pool, "a@example.com").await;
    let payer = create_payer(&pool, &a, "Alex").await;
    let sub = create_subscription(&pool, &a, "Netflix", Some(&payer)).await;
    let sub_id = body_json(sub).await["id"].as_str().unwrap().to_string();
    // Tant que référencé : refus.
    assert_eq!(
        send(
            &pool,
            "DELETE",
            &format!("/api/v1/payers/{payer}"),
            Some(&a),
            None
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
    // On détache l'abonnement du payeur (PUT sans `payer` -> payer_id NULL)...
    let detach = send(
        &pool,
        "PUT",
        &format!("/api/v1/subscriptions/{sub_id}"),
        Some(&a),
        Some(json!({
            "name": "Netflix",
            "amount": "9.99",
            "currency": "EUR",
            "cycle": { "unit": "month", "interval": 1 },
            "first_payment": "2030-01-15",
        })),
    )
    .await;
    assert_eq!(detach.status(), StatusCode::OK);
    // ...le payeur devient supprimable (204).
    assert_eq!(
        send(
            &pool,
            "DELETE",
            &format!("/api/v1/payers/{payer}"),
            Some(&a),
            None
        )
        .await
        .status(),
        StatusCode::NO_CONTENT
    );
}

// --- Autorisation §9 : createPayer ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-017)]
async fn authz_owner_create_payer(pool: PgPool) {
    let a = account(&pool, "owner@example.com").await;
    let r = send(
        &pool,
        "POST",
        "/api/v1/payers",
        Some(&a),
        Some(json!({ "name": "Alex" })),
    )
    .await;
    assert_eq!(r.status(), StatusCode::CREATED);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-017)]
async fn authz_other_create_payer(pool: PgPool) {
    // Un tiers authentifié crée dans SON foyer (aucun accès à celui d'autrui) : 201 isolé.
    account(&pool, "owner@example.com").await;
    let other = account(&pool, "other@example.com").await;
    let r = send(
        &pool,
        "POST",
        "/api/v1/payers",
        Some(&other),
        Some(json!({ "name": "Sam" })),
    )
    .await;
    assert_eq!(r.status(), StatusCode::CREATED);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-017)]
async fn authz_anon_create_payer(pool: PgPool) {
    let r = send(
        &pool,
        "POST",
        "/api/v1/payers",
        None,
        Some(json!({ "name": "Alex" })),
    )
    .await;
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
}

// --- Autorisation §9 : listPayers ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-017)]
async fn authz_owner_list_payers(pool: PgPool) {
    let a = account(&pool, "owner@example.com").await;
    create_payer(&pool, &a, "Alex").await;
    let r = send(&pool, "GET", "/api/v1/payers", Some(&a), None).await;
    assert_eq!(r.status(), StatusCode::OK);
    assert_eq!(body_json(r).await.as_array().unwrap().len(), 1);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-017)]
async fn authz_other_list_payers(pool: PgPool) {
    let a = account(&pool, "owner@example.com").await;
    create_payer(&pool, &a, "Alex").await;
    let other = account(&pool, "other@example.com").await;
    // Le tiers ne voit que SON foyer (vide), jamais le payeur du propriétaire.
    let list = body_json(send(&pool, "GET", "/api/v1/payers", Some(&other), None).await).await;
    assert_eq!(list.as_array().unwrap().len(), 0);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-017)]
async fn authz_anon_list_payers(pool: PgPool) {
    let r = send(&pool, "GET", "/api/v1/payers", None, None).await;
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
}

// --- Autorisation §9 : renamePayer ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-017)]
async fn authz_owner_rename_payer(pool: PgPool) {
    let a = account(&pool, "owner@example.com").await;
    let id = create_payer(&pool, &a, "Alex").await;
    let r = send(
        &pool,
        "PUT",
        &format!("/api/v1/payers/{id}"),
        Some(&a),
        Some(json!({ "name": "Sam" })),
    )
    .await;
    assert_eq!(r.status(), StatusCode::OK);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-017)]
async fn authz_other_rename_payer(pool: PgPool) {
    let a = account(&pool, "owner@example.com").await;
    let id = create_payer(&pool, &a, "Alex").await;
    let other = account(&pool, "other@example.com").await;
    // Le payeur d'autrui est traité comme inexistant : 404 (jamais 403).
    let r = send(
        &pool,
        "PUT",
        &format!("/api/v1/payers/{id}"),
        Some(&other),
        Some(json!({ "name": "Hack" })),
    )
    .await;
    assert_eq!(r.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-017)]
async fn authz_anon_rename_payer(pool: PgPool) {
    let a = account(&pool, "owner@example.com").await;
    let id = create_payer(&pool, &a, "Alex").await;
    let r = send(
        &pool,
        "PUT",
        &format!("/api/v1/payers/{id}"),
        None,
        Some(json!({ "name": "Hack" })),
    )
    .await;
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
}

// --- Autorisation §9 : deletePayer ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-017)]
async fn authz_owner_delete_payer(pool: PgPool) {
    let a = account(&pool, "owner@example.com").await;
    let id = create_payer(&pool, &a, "Alex").await;
    let r = send(
        &pool,
        "DELETE",
        &format!("/api/v1/payers/{id}"),
        Some(&a),
        None,
    )
    .await;
    assert_eq!(r.status(), StatusCode::NO_CONTENT);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-017)]
async fn authz_other_delete_payer(pool: PgPool) {
    let a = account(&pool, "owner@example.com").await;
    let id = create_payer(&pool, &a, "Alex").await;
    let other = account(&pool, "other@example.com").await;
    let r = send(
        &pool,
        "DELETE",
        &format!("/api/v1/payers/{id}"),
        Some(&other),
        None,
    )
    .await;
    assert_eq!(r.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-017)]
async fn authz_anon_delete_payer(pool: PgPool) {
    let a = account(&pool, "owner@example.com").await;
    let id = create_payer(&pool, &a, "Alex").await;
    let r = send(&pool, "DELETE", &format!("/api/v1/payers/{id}"), None, None).await;
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
}
