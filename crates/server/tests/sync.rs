//! Tests d'intégration de la synchronisation — pierres tombales (REQ-SYN-002).
//!
//! `GET /sync/tombstones` : une suppression d'entité possédée (catégorie, moyen de paiement, payeur)
//! produit une pierre tombale ; curseur `since` exclusif ; `full_resync_required` si le curseur précède
//! la rétention (ou est absent). Isolation §9 : propriétaire 200, tiers ne voit rien, anonyme 401.

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

async fn create(pool: &PgPool, cookie: &str, path: &str, name: &str) -> String {
    let r = send(
        pool,
        "POST",
        path,
        Some(cookie),
        Some(json!({ "name": name })),
    )
    .await;
    assert_eq!(r.status(), StatusCode::CREATED, "création {path}");
    body_json(r).await["id"].as_str().unwrap().to_string()
}

async fn tombstones(
    pool: &PgPool,
    cookie: Option<&str>,
    query: &str,
) -> axum::http::Response<Body> {
    send(
        pool,
        "GET",
        &format!("/api/v1/sync/tombstones{query}"),
        cookie,
        None,
    )
    .await
}

/// Types d'entités présents dans la réponse.
fn kinds(body: &Value) -> Vec<String> {
    body["tombstones"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["entity_type"].as_str().unwrap().to_string())
        .collect()
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SYN-002, case = "supprimer une entité produit une pierre tombale reçue à la synchro")]
async fn deleting_an_entity_produces_a_tombstone(pool: PgPool) {
    let web = account(&pool, "syn002-basic@example.com").await;
    let payer = create(&pool, &web, "/api/v1/payers", "Alex").await;
    assert_eq!(
        send(
            &pool,
            "DELETE",
            &format!("/api/v1/payers/{payer}"),
            Some(&web),
            None
        )
        .await
        .status(),
        StatusCode::NO_CONTENT
    );

    let body = body_json(tombstones(&pool, Some(&web), "").await).await;
    let list = body["tombstones"].as_array().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0]["entity_type"], "payer");
    assert_eq!(list[0]["entity_id"], payer);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SYN-002, case = "les trois types d'entités supprimables sont tracés")]
async fn all_deletable_entity_types_are_tombstoned(pool: PgPool) {
    let web = account(&pool, "syn002-types@example.com").await;
    let cat = create(&pool, &web, "/api/v1/categories", "Streaming").await;
    let pm = create(&pool, &web, "/api/v1/payment-methods", "Carte").await;
    let payer = create(&pool, &web, "/api/v1/payers", "Alex").await;
    for path in [
        format!("/api/v1/categories/{cat}"),
        format!("/api/v1/payment-methods/{pm}"),
        format!("/api/v1/payers/{payer}"),
    ] {
        assert_eq!(
            send(&pool, "DELETE", &path, Some(&web), None)
                .await
                .status(),
            StatusCode::NO_CONTENT
        );
    }

    let mut got = kinds(&body_json(tombstones(&pool, Some(&web), "").await).await);
    got.sort();
    assert_eq!(got, vec!["category", "payer", "payment_method"]);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SYN-002, case = "le curseur since ne renvoie que les suppressions postérieures")]
async fn since_cursor_returns_only_newer(pool: PgPool) {
    let web = account(&pool, "syn002-cursor@example.com").await;
    let first = create(&pool, &web, "/api/v1/payers", "First").await;
    send(
        &pool,
        "DELETE",
        &format!("/api/v1/payers/{first}"),
        Some(&web),
        None,
    )
    .await;

    // Curseur = instant serveur après la 1re suppression.
    let cursor = body_json(tombstones(&pool, Some(&web), "").await).await["as_of"]
        .as_str()
        .unwrap()
        .to_string();

    let second = create(&pool, &web, "/api/v1/payers", "Second").await;
    send(
        &pool,
        "DELETE",
        &format!("/api/v1/payers/{second}"),
        Some(&web),
        None,
    )
    .await;

    let body = body_json(tombstones(&pool, Some(&web), &format!("?since={cursor}")).await).await;
    let list = body["tombstones"].as_array().unwrap();
    // Seule la 2e suppression est postérieure au curseur.
    assert_eq!(list.len(), 1);
    assert_eq!(list[0]["entity_id"], second);
    // Curseur récent (issu d'un appel précédent) : pas de resynchronisation complète.
    assert_eq!(body["full_resync_required"], false);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SYN-002, case = "première synchronisation (sans curseur) -> resync complet")]
async fn first_sync_requires_full_resync(pool: PgPool) {
    let web = account(&pool, "syn002-first@example.com").await;
    let body = body_json(tombstones(&pool, Some(&web), "").await).await;
    assert_eq!(body["full_resync_required"], true);
    assert_eq!(body["retention_days"], 30);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SYN-002, case = "curseur périmé (avant la rétention) -> resync complet")]
async fn stale_cursor_requires_full_resync(pool: PgPool) {
    let web = account(&pool, "syn002-stale@example.com").await;
    // Curseur bien au-delà de la rétention (30 j) : resynchronisation complète imposée.
    let body = body_json(tombstones(&pool, Some(&web), "?since=2000-01-01T00:00:00Z").await).await;
    assert_eq!(body["full_resync_required"], true);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SYN-002, case = "curseur since illisible -> 422")]
async fn invalid_since_is_rejected(pool: PgPool) {
    let web = account(&pool, "syn002-bad@example.com").await;
    assert_eq!(
        tombstones(&pool, Some(&web), "?since=not-a-date")
            .await
            .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SYN-002, case = "une suppression refusée (référencé, 409) ne laisse pas de pierre tombale")]
async fn refused_delete_leaves_no_tombstone(pool: PgPool) {
    let web = account(&pool, "syn002-refused@example.com").await;
    let payer = create(&pool, &web, "/api/v1/payers", "Alex").await;
    // Rattache un abonnement au payeur : sa suppression sera refusée (409).
    let sub = json!({
        "name": "Netflix", "amount": "9.99", "currency": "EUR",
        "cycle": { "unit": "month", "interval": 1 },
        "first_payment": "2030-01-15", "payer": payer
    });
    assert_eq!(
        send(
            &pool,
            "POST",
            "/api/v1/subscriptions",
            Some(&web),
            Some(sub)
        )
        .await
        .status(),
        StatusCode::CREATED
    );
    assert_eq!(
        send(
            &pool,
            "DELETE",
            &format!("/api/v1/payers/{payer}"),
            Some(&web),
            None
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );

    // Aucune suppression effective -> aucune pierre tombale.
    let body = body_json(tombstones(&pool, Some(&web), "").await).await;
    assert!(body["tombstones"].as_array().unwrap().is_empty());
}

// --- Autorisation (§9) ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SYN-002)]
async fn authz_owner_get_tombstones(pool: PgPool) {
    let web = account(&pool, "own-syn002@example.com").await;
    let r = tombstones(&pool, Some(&web), "").await;
    assert_eq!(r.status(), StatusCode::OK);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SYN-002)]
async fn authz_other_get_tombstones(pool: PgPool) {
    // Le foyer A supprime un payeur ; le foyer B ne voit jamais cette pierre tombale (§9).
    let a = account(&pool, "a-syn002@example.com").await;
    let payer = create(&pool, &a, "/api/v1/payers", "A-only").await;
    send(
        &pool,
        "DELETE",
        &format!("/api/v1/payers/{payer}"),
        Some(&a),
        None,
    )
    .await;

    let b = account(&pool, "b-syn002@example.com").await;
    let body = body_json(tombstones(&pool, Some(&b), "").await).await;
    assert!(body["tombstones"].as_array().unwrap().is_empty());
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SYN-002)]
async fn authz_anon_get_tombstones(pool: PgPool) {
    assert_eq!(
        tombstones(&pool, None, "").await.status(),
        StatusCode::UNAUTHORIZED
    );
}
