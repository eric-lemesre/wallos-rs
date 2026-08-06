//! Tests d'intégration de la poussée de modifications locales (REQ-SYN-004).
//!
//! `POST /sync/push` : lot d'opérations appliquées **indépendamment** (succès partiel), **idempotent**
//! (rejeu = même état final), rejets identifiés par entité. Isolation §9.

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
    idem: Option<&str>,
) -> axum::http::Response<Body> {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(c) = cookie {
        builder = builder.header(header::COOKIE, c);
    }
    if let Some(k) = idem {
        builder = builder.header("idempotency-key", k);
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
            None
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
        None,
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

async fn push(
    pool: &PgPool,
    cookie: Option<&str>,
    ops: Value,
    idem: Option<&str>,
) -> axum::http::Response<Body> {
    send(
        pool,
        "POST",
        "/api/v1/sync/push",
        cookie,
        Some(json!({ "operations": ops })),
        idem,
    )
    .await
}

fn upsert(entity_type: &str, id: &str, payload: Value) -> Value {
    json!({ "op": "upsert", "entity_type": entity_type, "id": id, "payload": payload })
}

fn del(entity_type: &str, id: &str) -> Value {
    json!({ "op": "delete", "entity_type": entity_type, "id": id })
}

/// Statuts des résultats, alignés sur l'ordre des opérations.
fn statuses(body: &Value) -> Vec<String> {
    body["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["status"].as_str().unwrap().to_string())
        .collect()
}

async fn payer_names(pool: &PgPool, cookie: &str) -> Vec<String> {
    let body = body_json(send(pool, "GET", "/api/v1/payers", Some(cookie), None, None).await).await;
    body.as_array()
        .unwrap()
        .iter()
        .map(|p| p["name"].as_str().unwrap().to_string())
        .collect()
}

const SUB1: &str = "11111111-1111-1111-1111-111111111111";
const PAYER1: &str = "22222222-2222-2222-2222-222222222222";

fn sub_payload(name: &str, amount: &str) -> Value {
    json!({
        "id": SUB1, "name": name, "amount": amount, "currency": "EUR",
        "cycle": { "unit": "month", "interval": 1 }, "first_payment": "2030-01-15"
    })
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SYN-004, case = "un lot d'upserts est appliqué (entités créées)")]
async fn applies_a_batch_of_upserts(pool: PgPool) {
    let web = account(&pool, "syn004-apply@example.com").await;
    let ops = json!([
        upsert("payer", PAYER1, json!({ "name": "Alex" })),
        upsert("subscription", SUB1, sub_payload("Netflix", "9.99")),
    ]);
    let body = body_json(push(&pool, Some(&web), ops, None).await).await;
    assert_eq!(statuses(&body), vec!["applied", "applied"]);
    // Persistance réelle : le payeur existe.
    assert!(payer_names(&pool, &web).await.contains(&"Alex".to_string()));
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SYN-004, case = "un upsert sur un id existant met à jour l'entité")]
async fn upsert_updates_existing_entity(pool: PgPool) {
    let web = account(&pool, "syn004-update@example.com").await;
    push(
        &pool,
        Some(&web),
        json!([upsert("payer", PAYER1, json!({ "name": "Alex" }))]),
        None,
    )
    .await;
    let body = body_json(
        push(
            &pool,
            Some(&web),
            json!([upsert("payer", PAYER1, json!({ "name": "Alexandra" }))]),
            None,
        )
        .await,
    )
    .await;
    assert_eq!(statuses(&body), vec!["applied"]);
    let names = payer_names(&pool, &web).await;
    assert!(names.contains(&"Alexandra".to_string()));
    assert!(!names.contains(&"Alex".to_string()));
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SYN-004, case = "un envoi partiellement rejeté identifie l'échec, les autres sont appliqués")]
async fn partial_rejection_identifies_failures(pool: PgPool) {
    let web = account(&pool, "syn004-partial@example.com").await;
    let ops = json!([
        upsert("payer", PAYER1, json!({ "name": "Alex" })), // ok
        upsert(
            "wombat",
            "33333333-3333-3333-3333-333333333333",
            json!({ "name": "X" })
        ), // type inconnu
    ]);
    let body = body_json(push(&pool, Some(&web), ops, None).await).await;
    let results = body["results"].as_array().unwrap();
    assert_eq!(results[0]["status"], "applied");
    assert_eq!(results[1]["status"], "rejected");
    assert!(results[1]["reason"].as_str().unwrap().contains("inconnu"));
    // L'opération valide a bien été appliquée malgré le rejet de l'autre.
    assert!(payer_names(&pool, &web).await.contains(&"Alex".to_string()));
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SYN-004, case = "rejeu avec clé d'idempotence : même réponse, aucun effet de bord supplémentaire")]
async fn replay_with_idempotency_key_is_noop(pool: PgPool) {
    let web = account(&pool, "syn004-idem@example.com").await;
    let ops = json!([upsert("subscription", SUB1, sub_payload("Netflix", "9.99"))]);
    let first = body_json(push(&pool, Some(&web), ops.clone(), Some("key-1")).await).await;
    let second = body_json(push(&pool, Some(&web), ops, Some("key-1")).await).await;
    // Réponse identique au rejeu.
    assert_eq!(first, second);
    // Un seul abonnement (aucune duplication).
    let subs = body_json(
        send(
            &pool,
            "GET",
            "/api/v1/subscriptions",
            Some(&web),
            None,
            None,
        )
        .await,
    )
    .await;
    assert_eq!(subs["subscriptions"].as_array().unwrap().len(), 1);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SYN-004, case = "idempotence naturelle : rejouer le même upsert par id converge (sans clé)")]
async fn upsert_is_naturally_idempotent_by_id(pool: PgPool) {
    let web = account(&pool, "syn004-natural@example.com").await;
    let ops = json!([upsert("subscription", SUB1, sub_payload("Netflix", "9.99"))]);
    push(&pool, Some(&web), ops.clone(), None).await;
    push(&pool, Some(&web), ops, None).await;
    let subs = body_json(
        send(
            &pool,
            "GET",
            "/api/v1/subscriptions",
            Some(&web),
            None,
            None,
        )
        .await,
    )
    .await;
    // Rejeu sans clé : toujours un seul abonnement (clé = id).
    assert_eq!(subs["subscriptions"].as_array().unwrap().len(), 1);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SYN-004, case = "suppression idempotente ; suppression d'un référencé rejetée")]
async fn delete_semantics(pool: PgPool) {
    let web = account(&pool, "syn004-del@example.com").await;
    // Supprimer une entité absente est une opération idempotente -> appliquée (no-op).
    let body = body_json(push(&pool, Some(&web), json!([del("payer", PAYER1)]), None).await).await;
    assert_eq!(statuses(&body), vec!["applied"]);

    // Un payeur référencé par un abonnement ne peut pas être supprimé -> rejeté.
    push(&pool, Some(&web), json!([
        upsert("payer", PAYER1, json!({ "name": "Alex" })),
        upsert("subscription", SUB1, json!({
            "id": SUB1, "name": "Netflix", "amount": "9.99", "currency": "EUR",
            "cycle": { "unit": "month", "interval": 1 }, "first_payment": "2030-01-15", "payer": PAYER1
        })),
    ]), None).await;
    let body = body_json(push(&pool, Some(&web), json!([del("payer", PAYER1)]), None).await).await;
    assert_eq!(body["results"][0]["status"], "rejected");
    assert!(
        body["results"][0]["reason"]
            .as_str()
            .unwrap()
            .contains("référencé")
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SYN-004, case = "isolation §9 : un upsert sur un id d'un autre foyer est rejeté")]
async fn cross_household_upsert_is_rejected(pool: PgPool) {
    let a = account(&pool, "a-syn004@example.com").await;
    push(
        &pool,
        Some(&a),
        json!([upsert("payer", PAYER1, json!({ "name": "A-payer" }))]),
        None,
    )
    .await;

    // B tente d'écraser le même id (clé primaire globale) : rejeté, et le payeur de A est intact.
    let b = account(&pool, "b-syn004@example.com").await;
    let body = body_json(
        push(
            &pool,
            Some(&b),
            json!([upsert("payer", PAYER1, json!({ "name": "hijack" }))]),
            None,
        )
        .await,
    )
    .await;
    assert_eq!(body["results"][0]["status"], "rejected");
    assert!(payer_names(&pool, &b).await.is_empty());
    assert!(
        payer_names(&pool, &a)
            .await
            .contains(&"A-payer".to_string())
    );
}

// --- Autorisation (§9) ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SYN-004)]
async fn authz_owner_push_sync_changes(pool: PgPool) {
    let web = account(&pool, "own-syn004@example.com").await;
    let r = push(
        &pool,
        Some(&web),
        json!([upsert("payer", PAYER1, json!({ "name": "Mine" }))]),
        None,
    )
    .await;
    assert_eq!(r.status(), StatusCode::OK);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SYN-004)]
async fn authz_other_push_sync_changes(pool: PgPool) {
    // Couvert fonctionnellement par cross_household_upsert_is_rejected ; ici on vérifie que le lot d'un
    // tiers n'affecte jamais le foyer d'autrui.
    let a = account(&pool, "a2-syn004@example.com").await;
    push(
        &pool,
        Some(&a),
        json!([upsert("payer", PAYER1, json!({ "name": "A-payer" }))]),
        None,
    )
    .await;
    let b = account(&pool, "b2-syn004@example.com").await;
    push(&pool, Some(&b), json!([del("payer", PAYER1)]), None).await;
    // La suppression poussée par B (no-op chez B) ne touche pas le payeur de A.
    assert!(
        payer_names(&pool, &a)
            .await
            .contains(&"A-payer".to_string())
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SYN-004)]
async fn authz_anon_push_sync_changes(pool: PgPool) {
    let r = push(
        &pool,
        None,
        json!([upsert("payer", PAYER1, json!({ "name": "X" }))]),
        None,
    )
    .await;
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
}
