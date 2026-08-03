//! Tests d'intégration de l'export/import des données d'un foyer (REQ-SUB-016).
//!
//! `GET /export` sérialise les entités possédées du foyer ; `POST /import` les recrée dans le foyer
//! appelant, tolérant (rapport de rejets). Round-trip **à l'identique** (mêmes identifiants). Auth
//! requise ; isolation §9.

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
const CAT_ID: &str = "11111111-1111-4111-8111-111111111111";
const PM_ID: &str = "22222222-2222-4222-8222-222222222222";
const SUB_ID: &str = "33333333-3333-4333-8333-333333333333";
const SUB_ID2: &str = "44444444-4444-4444-8444-444444444444";

fn app(pool: PgPool) -> Router {
    app_with_db(Db::from_pool(pool))
}

async fn send(
    pool: &PgPool,
    method: &str,
    uri: &str,
    body: Option<Value>,
    cookie: Option<&str>,
) -> axum::http::Response<Body> {
    let mut b = Request::builder().method(method).uri(uri);
    if body.is_some() {
        b = b.header(header::CONTENT_TYPE, "application/json");
    }
    if let Some(c) = cookie {
        b = b.header(header::COOKIE, c);
    }
    let payload = body.map_or_else(Body::empty, |v| Body::from(v.to_string()));
    app(pool.clone())
        .oneshot(b.body(payload).unwrap())
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
            Some(json!({ "email": email, "password": PASSWORD })),
            None,
        )
        .await
        .status(),
        StatusCode::CREATED
    );
    let r = send(
        pool,
        "POST",
        "/api/v1/sessions",
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

/// Amorce un foyer non trivial : devise de référence USD, une catégorie, un moyen de paiement, et un
/// abonnement liant les deux (identifiants fournis pour un round-trip déterministe).
async fn seed_household(pool: &PgPool, cookie: &str) {
    assert_eq!(
        send(
            pool,
            "PUT",
            "/api/v1/settings/reference-currency",
            Some(json!({ "currency": "USD" })),
            Some(cookie),
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        send(
            pool,
            "POST",
            "/api/v1/categories",
            Some(json!({ "id": CAT_ID, "name": "Streaming" })),
            Some(cookie),
        )
        .await
        .status(),
        StatusCode::CREATED
    );
    assert_eq!(
        send(
            pool,
            "POST",
            "/api/v1/payment-methods",
            Some(json!({ "id": PM_ID, "name": "Carte" })),
            Some(cookie),
        )
        .await
        .status(),
        StatusCode::CREATED
    );
    assert_eq!(
        send(
            pool,
            "POST",
            "/api/v1/subscriptions",
            Some(json!({
                "id": SUB_ID,
                "name": "Netflix",
                "amount": "9.99",
                "currency": "USD",
                "cycle": { "unit": "month", "interval": 1 },
                "first_payment": "2025-01-31",
                "category": CAT_ID,
                "payment_method": PM_ID,
                "end_date": "2026-01-31"
            })),
            Some(cookie),
        )
        .await
        .status(),
        StatusCode::CREATED
    );
}

/// Critère #1 : un export réimporté dans un compte vierge reconstruit l'état **à l'identique**.
///
/// On vérifie `export ∘ import = identité` : une enveloppe canonique importée dans un compte vierge,
/// puis réexportée, redonne **exactement** la même enveloppe (mêmes identifiants — préservés,
/// REQ-SYN-001 — mêmes liaisons catégorie/moyen de paiement, même devise de référence). Les échéances
/// sont dérivées à la lecture, jamais stockées.
#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-016)]
async fn round_trip_reproduces_state_identically(pool: PgPool) {
    let b = account(&pool, "b@example.com").await;
    // Enveloppe canonique = forme exacte produite par l'export (champs None omis, `active` présent).
    let bundle = json!({
        "version": 1,
        "reference_currency": "USD",
        "categories": [ { "id": CAT_ID, "name": "Streaming" } ],
        "payment_methods": [ { "id": PM_ID, "name": "Carte" } ],
        "subscriptions": [ {
            "id": SUB_ID,
            "name": "Netflix",
            "amount": "9.99",
            "currency": "USD",
            "cycle": { "unit": "month", "interval": 1 },
            "first_payment": "2025-01-31",
            "category": CAT_ID,
            "payment_method": PM_ID,
            "active": true,
            "end_date": "2026-01-31"
        } ]
    });

    let report = body_json(
        send(
            &pool,
            "POST",
            "/api/v1/import",
            Some(bundle.clone()),
            Some(&b),
        )
        .await,
    )
    .await;
    assert_eq!(report["imported"]["categories"], 1);
    assert_eq!(report["imported"]["payment_methods"], 1);
    assert_eq!(report["imported"]["subscriptions"], 1);
    assert_eq!(report["rejected"].as_array().unwrap().len(), 0);

    // Un compte neuf porte déjà les catégories par défaut (REQ-CAT-002) ; l'identité porte sur les
    // **entités importées** : l'abonnement est reproduit au bit près (mêmes id, montant, cycle, dates,
    // et **liaison** category = CAT_ID préservée), la devise de référence appliquée, et la catégorie
    // « Streaming » présente. Les échéances sont dérivées à la lecture, jamais dans l'enveloppe.
    let exported = body_json(send(&pool, "GET", "/api/v1/export", None, Some(&b)).await).await;
    assert_eq!(exported["version"], 1);
    assert_eq!(exported["reference_currency"], "USD");
    assert_eq!(exported["subscriptions"], bundle["subscriptions"]);
    let has_streaming = exported["categories"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["id"] == CAT_ID && c["name"] == "Streaming");
    assert!(has_streaming, "la catégorie importée doit être présente");
}

/// Critère #2 : les lignes invalides sont rejetées **avec leur raison**, les valides créées.
#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-016)]
async fn import_reports_rejected_rows(pool: PgPool) {
    let a = account(&pool, "a@example.com").await;
    let bundle = json!({
        "version": 1,
        "reference_currency": "XXX",
        "categories": [ { "name": "Ok" } ],
        "subscriptions": [
            { "name": "BadCurrency", "amount": "1.00", "currency": "XXX",
              "cycle": { "unit": "month", "interval": 1 }, "first_payment": "2025-01-01" },
            { "name": "BadDate", "amount": "1.00", "currency": "EUR",
              "cycle": { "unit": "month", "interval": 1 }, "first_payment": "pas-une-date" },
            { "name": "Good", "amount": "5.00", "currency": "EUR",
              "cycle": { "unit": "month", "interval": 1 }, "first_payment": "2025-01-01" }
        ]
    });
    let report =
        body_json(send(&pool, "POST", "/api/v1/import", Some(bundle), Some(&a)).await).await;

    assert_eq!(report["imported"]["categories"], 1);
    assert_eq!(report["imported"]["subscriptions"], 1);
    let rejected = report["rejected"].as_array().unwrap();
    // devise de référence + 2 abonnements invalides.
    assert_eq!(rejected.len(), 3);
    let kinds: Vec<&str> = rejected.iter().filter_map(|r| r["kind"].as_str()).collect();
    assert!(kinds.contains(&"reference_currency"));
    assert_eq!(kinds.iter().filter(|k| **k == "subscription").count(), 2);
    // La raison identifie le champ fautif.
    let bad_currency = rejected
        .iter()
        .find(|r| r["reference"] == "BadCurrency")
        .unwrap();
    assert!(
        bad_currency["reason"]
            .as_str()
            .unwrap()
            .contains("currency")
    );
}

/// Une version de format inconnue est rejetée globalement (`422`), sans rien créer.
#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-016)]
async fn import_rejects_unknown_version(pool: PgPool) {
    let a = account(&pool, "a@example.com").await;
    let resp = send(
        &pool,
        "POST",
        "/api/v1/import",
        Some(json!({ "version": 999, "subscriptions": [] })),
        Some(&a),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

/// Réimporter la même enveloppe rejette les identifiants déjà présents (idempotence défensive).
#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-016)]
async fn import_twice_rejects_duplicate_ids(pool: PgPool) {
    let a = account(&pool, "a@example.com").await;
    seed_household(&pool, &a).await;
    let bundle = body_json(send(&pool, "GET", "/api/v1/export", None, Some(&a)).await).await;

    // Réimport dans le MÊME foyer : les catégories sont **fusionnées par nom** (donc rien de créé ni
    // rejeté côté catégories), tandis que le moyen de paiement et l'abonnement, dont l'`id` est déjà
    // pris, sont rejetés — l'import ne duplique pas.
    let report =
        body_json(send(&pool, "POST", "/api/v1/import", Some(bundle), Some(&a)).await).await;
    assert_eq!(report["imported"]["categories"], 0);
    assert_eq!(report["imported"]["payment_methods"], 0);
    assert_eq!(report["imported"]["subscriptions"], 0);
    let rejected = report["rejected"].as_array().unwrap();
    assert_eq!(rejected.len(), 2);
    assert!(rejected.iter().all(|r| {
        r["reason"]
            .as_str()
            .unwrap()
            .contains("identifiant déjà présent")
    }));
    let kinds: Vec<&str> = rejected.iter().filter_map(|r| r["kind"].as_str()).collect();
    assert!(kinds.contains(&"payment_method"));
    assert!(kinds.contains(&"subscription"));
}

// --- Autorisation (§9) ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-016)]
async fn authz_owner_export_data(pool: PgPool) {
    let a = account(&pool, "owner@example.com").await;
    seed_household(&pool, &a).await;
    let resp = send(&pool, "GET", "/api/v1/export", None, Some(&a)).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let bundle = body_json(resp).await;
    assert_eq!(bundle["subscriptions"].as_array().unwrap().len(), 1);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-016)]
async fn authz_other_export_data(pool: PgPool) {
    let a = account(&pool, "owner@example.com").await;
    seed_household(&pool, &a).await;
    // Un autre foyer n'exporte que SES données : aucun abonnement du foyer A (ses catégories par
    // défaut, REQ-CAT-002, lui sont propres et n'exposent rien de A).
    let other = account(&pool, "other@example.com").await;
    let bundle = body_json(send(&pool, "GET", "/api/v1/export", None, Some(&other)).await).await;
    assert_eq!(bundle["subscriptions"].as_array().unwrap().len(), 0);
    assert_eq!(bundle["payment_methods"].as_array().unwrap().len(), 0);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-016)]
async fn authz_anon_export_data(pool: PgPool) {
    let resp = send(&pool, "GET", "/api/v1/export", None, None).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-016)]
async fn authz_owner_import_data(pool: PgPool) {
    let a = account(&pool, "owner@example.com").await;
    let resp = send(
        &pool,
        "POST",
        "/api/v1/import",
        Some(json!({ "version": 1, "categories": [ { "name": "X" } ] })),
        Some(&a),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-016)]
async fn authz_other_import_data(pool: PgPool) {
    // Deux foyers importent chacun un abonnement distinct : chacun ne voit que le sien (§9).
    let a = account(&pool, "owner@example.com").await;
    let other = account(&pool, "other@example.com").await;
    let sub = |name: &str, id: &str| {
        json!({
            "version": 1,
            "subscriptions": [ {
                "id": id, "name": name, "amount": "1.00", "currency": "EUR",
                "cycle": { "unit": "month", "interval": 1 }, "first_payment": "2025-01-01"
            } ]
        })
    };
    assert_eq!(
        send(
            &pool,
            "POST",
            "/api/v1/import",
            Some(sub("A-sub", SUB_ID)),
            Some(&a)
        )
        .await
        .status(),
        StatusCode::OK
    );
    // Id distinct (la clé primaire des abonnements est globale) : l'import atterrit dans le foyer B.
    assert_eq!(
        send(
            &pool,
            "POST",
            "/api/v1/import",
            Some(sub("B-sub", SUB_ID2)),
            Some(&other)
        )
        .await
        .status(),
        StatusCode::OK
    );
    let bundle_a = body_json(send(&pool, "GET", "/api/v1/export", None, Some(&a)).await).await;
    let bundle_b = body_json(send(&pool, "GET", "/api/v1/export", None, Some(&other)).await).await;
    assert_eq!(bundle_a["subscriptions"][0]["name"], "A-sub");
    assert_eq!(bundle_b["subscriptions"][0]["name"], "B-sub");
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-SUB-016)]
async fn authz_anon_import_data(pool: PgPool) {
    let resp = send(
        &pool,
        "POST",
        "/api/v1/import",
        Some(json!({ "version": 1 })),
        None,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}
