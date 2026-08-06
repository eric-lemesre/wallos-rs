//! Tests d'intégration de la résolution de conflit (REQ-SYN-005).
//!
//! Dernière écriture gagnante + concurrence optimiste : un écrasement fondé sur une version périmée
//! journalise la version perdue ; une suppression concurrente l'emporte (modification journalisée).
//! Journal consultable via `GET /sync/conflicts`. Isolation §9.

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
const PAYER1: &str = "22222222-2222-2222-2222-222222222222";

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
            Some(json!({ "email": email, "password": PASSWORD }))
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

async fn push(pool: &PgPool, cookie: &str, ops: Value) -> Value {
    body_json(
        send(
            pool,
            "POST",
            "/api/v1/sync/push",
            Some(cookie),
            Some(json!({ "operations": ops })),
        )
        .await,
    )
    .await
}

fn upsert_payer(name: &str, base_version: Option<&str>) -> Value {
    let mut op = json!({ "op": "upsert", "entity_type": "payer", "id": PAYER1, "payload": { "name": name } });
    if let Some(b) = base_version {
        op["base_version"] = json!(b);
    }
    op
}

async fn conflicts(pool: &PgPool, cookie: Option<&str>) -> axum::http::Response<Body> {
    send(pool, "GET", "/api/v1/sync/conflicts", cookie, None).await
}

async fn payer_names(pool: &PgPool, cookie: &str) -> Vec<String> {
    let body = body_json(send(pool, "GET", "/api/v1/payers", Some(cookie), None).await).await;
    body.as_array()
        .unwrap()
        .iter()
        .map(|p| p["name"].as_str().unwrap().to_string())
        .collect()
}

/// `updated_at` courant du payeur via le delta de synchronisation.
async fn payer_version(pool: &PgPool, cookie: &str) -> String {
    let body = body_json(
        send(
            pool,
            "GET",
            "/api/v1/sync/changes?limit=500",
            Some(cookie),
            None,
        )
        .await,
    )
    .await;
    for c in body["changes"].as_array().unwrap() {
        if c["entity_type"] == "payer" && c["id"] == PAYER1 {
            return c["payload"]["updated_at"].as_str().unwrap().to_string();
        }
    }
    panic!("payeur absent du delta");
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SYN-005, case = "écrasement fondé sur une version périmée : appliqué, version perdue journalisée")]
async fn stale_overwrite_is_applied_and_journaled(pool: PgPool) {
    let web = account(&pool, "syn005-overwrite@example.com").await;
    // v0 : « Alex ».
    push(&pool, &web, json!([upsert_payer("Alex", None)])).await;
    // Écriture fondée sur une version périmée (base ancienne ≠ version courante) : « Alexandra » l'emporte.
    let res = push(
        &pool,
        &web,
        json!([upsert_payer("Alexandra", Some("2000-01-01T00:00:00Z"))]),
    )
    .await;
    assert_eq!(res["results"][0]["status"], "applied");
    assert!(
        payer_names(&pool, &web)
            .await
            .contains(&"Alexandra".to_string())
    );

    // La version écrasée (« Alex ») est conservée au journal.
    let journal = body_json(conflicts(&pool, Some(&web)).await).await;
    let entries = journal["conflicts"].as_array().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["reason"], "overwritten");
    assert_eq!(entries[0]["entity_type"], "payer");
    assert_eq!(entries[0]["lost_payload"]["name"], "Alex");
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SYN-005, case = "édition séquentielle (base concordante ou absente) : pas de conflit journalisé")]
async fn sequential_edit_does_not_journal(pool: PgPool) {
    let web = account(&pool, "syn005-seq@example.com").await;
    push(&pool, &web, json!([upsert_payer("Alex", None)])).await;
    let current = payer_version(&pool, &web).await;
    // Base = version courante : édition normale, aucun conflit.
    let res = push(
        &pool,
        &web,
        json!([upsert_payer("Alexandra", Some(&current))]),
    )
    .await;
    assert_eq!(res["results"][0]["status"], "applied");
    let journal = body_json(conflicts(&pool, Some(&web)).await).await;
    assert!(journal["conflicts"].as_array().unwrap().is_empty());
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SYN-005, case = "la suppression concurrente l'emporte : modification écartée et journalisée")]
async fn deletion_wins_over_modification(pool: PgPool) {
    let web = account(&pool, "syn005-del@example.com").await;
    push(&pool, &web, json!([upsert_payer("Alex", None)])).await;
    // Suppression concurrente.
    push(
        &pool,
        &web,
        json!([{ "op": "delete", "entity_type": "payer", "id": PAYER1 }]),
    )
    .await;
    // Une modification arrive après la suppression : écartée (la suppression l'emporte), et journalisée.
    let res = push(
        &pool,
        &web,
        json!([upsert_payer("Zombie", Some("2000-01-01T00:00:00Z"))]),
    )
    .await;
    assert_eq!(res["results"][0]["status"], "rejected");
    assert!(
        res["results"][0]["reason"]
            .as_str()
            .unwrap()
            .contains("suppression")
    );
    // Le payeur n'est pas ressuscité.
    assert!(payer_names(&pool, &web).await.is_empty());
    // La modification perdue est au journal.
    let journal = body_json(conflicts(&pool, Some(&web)).await).await;
    let entries = journal["conflicts"].as_array().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["reason"], "deleted_remotely");
    assert_eq!(entries[0]["lost_payload"]["name"], "Zombie");
}

// --- Autorisation (§9) ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SYN-005)]
async fn authz_owner_get_sync_conflicts(pool: PgPool) {
    let web = account(&pool, "own-syn005@example.com").await;
    assert_eq!(conflicts(&pool, Some(&web)).await.status(), StatusCode::OK);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SYN-005)]
async fn authz_other_get_sync_conflicts(pool: PgPool) {
    // A génère un conflit ; B ne voit jamais le journal de A (§9).
    let a = account(&pool, "a-syn005@example.com").await;
    push(&pool, &a, json!([upsert_payer("Alex", None)])).await;
    push(
        &pool,
        &a,
        json!([upsert_payer("Alexandra", Some("2000-01-01T00:00:00Z"))]),
    )
    .await;
    let b = account(&pool, "b-syn005@example.com").await;
    let journal = body_json(conflicts(&pool, Some(&b)).await).await;
    assert!(journal["conflicts"].as_array().unwrap().is_empty());
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SYN-005)]
async fn authz_anon_get_sync_conflicts(pool: PgPool) {
    assert_eq!(
        conflicts(&pool, None).await.status(),
        StatusCode::UNAUTHORIZED
    );
}
