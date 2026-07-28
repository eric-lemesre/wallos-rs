//! Tests d'intégration des taux de change (REQ-CUR-003).
//!
//! Couvre les deux critères d'acceptation : (1) la récupération **persiste** les taux avec leur
//! date de validité et leur source ; (2) sans taux (aucun fournisseur), l'agrégation reste
//! fonctionnelle et est **signalée partielle** — jamais un total silencieusement amputé.

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use chrono::{NaiveDate, Utc};
use rust_decimal::Decimal;
use serde_json::json;
use sqlx::PgPool;
use tower::ServiceExt;
use wallos_core::money::{CurrencyCode, Money};
use wallos_core::{ExchangeRate, aggregate_converted};
use wallos_req_macros::verifies;
use wallos_server::app_with_db;
use wallos_server::exchange::{RateSource, RateSourceError, load_rate_table, refresh_rates};
use wallos_storage::{Db, ExchangeRateRepository};

fn cur(code: &str) -> CurrencyCode {
    CurrencyCode::new(code).unwrap()
}

fn dec(s: &str) -> Decimal {
    s.parse().unwrap()
}

fn money(units: &str, code: &str) -> Money {
    Money::new(dec(units), cur(code)).unwrap()
}

fn as_of() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 7, 27).unwrap()
}

/// Fournisseur de taux statique (l'équivalent d'un adaptateur, sans réseau) pour les tests.
struct StaticSource(Vec<ExchangeRate>);

impl RateSource for StaticSource {
    async fn fetch(&self) -> Result<Vec<ExchangeRate>, RateSourceError> {
        Ok(self.0.clone())
    }
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-CUR-003)]
async fn refresh_persists_rates_with_validity_and_source(pool: PgPool) {
    let repo = ExchangeRateRepository::new(&pool);
    let source = StaticSource(vec![
        ExchangeRate::new(cur("EUR"), cur("USD"), dec("1.10"), as_of(), "acme").unwrap(),
        ExchangeRate::new(cur("USD"), cur("GBP"), dec("0.80"), as_of(), "acme").unwrap(),
    ]);

    let count = refresh_rates(&source, &repo, Utc::now()).await.unwrap();
    assert_eq!(count, 2);

    let stored = repo.latest("EUR", "USD").await.unwrap().unwrap();
    assert_eq!(stored.rate, dec("1.10"));
    assert_eq!(stored.as_of, as_of());
    assert_eq!(stored.source, "acme");
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-CUR-003)]
async fn refresh_is_idempotent_upsert(pool: PgPool) {
    let repo = ExchangeRateRepository::new(&pool);
    let first = StaticSource(vec![
        ExchangeRate::new(cur("EUR"), cur("USD"), dec("1.10"), as_of(), "acme").unwrap(),
    ]);
    let updated = StaticSource(vec![
        ExchangeRate::new(cur("EUR"), cur("USD"), dec("1.12"), as_of(), "acme").unwrap(),
    ]);
    refresh_rates(&first, &repo, Utc::now()).await.unwrap();
    refresh_rates(&updated, &repo, Utc::now()).await.unwrap();

    // Même paire + même date de validité : mise à jour, pas de doublon.
    assert_eq!(
        repo.latest("EUR", "USD").await.unwrap().unwrap().rate,
        dec("1.12")
    );
    assert_eq!(repo.all_latest().await.unwrap().len(), 1);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-CUR-003)]
async fn aggregate_over_loaded_table_flags_partial(pool: PgPool) {
    let repo = ExchangeRateRepository::new(&pool);
    // Seul EUR->USD est connu.
    let source = StaticSource(vec![
        ExchangeRate::new(cur("EUR"), cur("USD"), dec("1.10"), as_of(), "acme").unwrap(),
    ]);
    refresh_rates(&source, &repo, Utc::now()).await.unwrap();

    let table = load_rate_table(&repo).await.unwrap();
    // GBP->USD inconnu : montant exclu, agrégat signalé incomplet.
    let amounts = [money("10", "EUR"), money("100", "GBP")];
    let agg = aggregate_converted(&amounts, cur("USD"), &table);
    assert_eq!(agg.total().amount(), dec("11.00"));
    assert_eq!(agg.converted(), 1);
    assert_eq!(agg.excluded(), 1);
    assert!(!agg.is_complete());
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-CUR-003)]
async fn no_rates_stays_functional_but_partial(pool: PgPool) {
    let repo = ExchangeRateRepository::new(&pool);
    // Aucun fournisseur configuré : base vide -> table vide (jamais une panne).
    let table = load_rate_table(&repo).await.unwrap();

    // Conversion vers une devise étrangère sans taux : exclue + partiel.
    let foreign = aggregate_converted(&[money("10", "EUR")], cur("USD"), &table);
    assert_eq!(foreign.excluded(), 1);
    assert!(!foreign.is_complete());

    // Mais l'agrégation en devise identique reste complète (taux 1).
    let same = aggregate_converted(&[money("10", "EUR")], cur("EUR"), &table);
    assert_eq!(same.total().amount(), dec("10"));
    assert!(same.is_complete());
}

// ============================================================================
// REQ-CUR-004 — endpoint d'agrégation en mode dégradé (fraîcheur + partiel)
//
// `POST /exchange/aggregate` : convertit des montants fournis vers une devise cible en retombant sur
// les **derniers taux connus** persistés (fournisseur indisponible/non configuré). Il expose la
// fraîcheur (`as_of`) et signale explicitement un agrégat partiel — jamais un zéro silencieux.
// ============================================================================

const PASSWORD: &str = "correct horse battery staple";

fn app(pool: PgPool) -> Router {
    app_with_db(Db::from_pool(pool))
}

async fn post_json(
    pool: &PgPool,
    uri: &str,
    body: serde_json::Value,
    auth: Option<String>,
) -> axum::http::Response<Body> {
    let mut builder = Request::builder()
        .method("POST")
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(cookie) = auth {
        builder = builder.header(header::COOKIE, cookie);
    }
    app(pool.clone())
        .oneshot(builder.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap()
}

async fn signup(pool: &PgPool, email: &str) {
    let response = post_json(
        pool,
        "/api/v1/accounts",
        json!({ "email": email, "password": PASSWORD }),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
}

async fn login_cookie(pool: &PgPool, email: &str) -> String {
    let response = post_json(
        pool,
        "/api/v1/sessions",
        json!({ "email": email, "password": PASSWORD }),
        None,
    )
    .await;
    let set_cookie = response
        .headers()
        .get(header::SET_COOKIE)
        .and_then(|v| v.to_str().ok())
        .expect("login sets a session cookie");
    set_cookie.split(';').next().unwrap().to_string()
}

/// `POST /exchange/aggregate` avec une session (cookie) et un corps donné.
async fn aggregate(
    pool: &PgPool,
    cookie: &str,
    body: serde_json::Value,
) -> axum::http::Response<Body> {
    post_json(
        pool,
        "/api/v1/exchange/aggregate",
        body,
        Some(cookie.to_string()),
    )
    .await
}

async fn body_json(response: axum::http::Response<Body>) -> serde_json::Value {
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

/// Persiste un taux (dernier taux connu) via la voie normale de rafraîchissement.
async fn seed_rate(pool: &PgPool, base: &str, quote: &str, rate: &str, as_of: NaiveDate) {
    let repo = ExchangeRateRepository::new(pool);
    let source = StaticSource(vec![
        ExchangeRate::new(cur(base), cur(quote), dec(rate), as_of, "acme").unwrap(),
    ]);
    refresh_rates(&source, &repo, Utc::now()).await.unwrap();
}

// --- Parcours fonctionnels ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-CUR-004)]
async fn aggregate_reports_total_and_staleness_date(pool: PgPool) {
    // Critère #1 : le dernier taux connu est utilisé ET sa date est remontée (fraîcheur).
    signup(&pool, "agg-ok@example.com").await;
    seed_rate(&pool, "EUR", "USD", "1.10", day(2026, 7, 20)).await;
    let web = login_cookie(&pool, "agg-ok@example.com").await;

    let res = aggregate(
        &pool,
        &web,
        json!({
            "target": "USD",
            "amounts": [
                { "amount": "10", "currency": "EUR" },
                { "amount": "5", "currency": "USD" }
            ]
        }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = body_json(res).await;
    // 10 EUR * 1.10 = 11.00 + 5 USD = 16.00 (montant en CHAÎNE, jamais un nombre JSON).
    assert_eq!(body["total"], "16.00");
    assert_eq!(body["currency"], "USD");
    assert_eq!(body["converted"], 2);
    assert_eq!(body["excluded"], 0);
    assert_eq!(body["complete"], true);
    assert_eq!(body["as_of"], "2026-07-20");
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-CUR-004)]
async fn aggregate_without_rate_flags_incomplete_never_silent_zero(pool: PgPool) {
    // Critère #2 : sans taux pour une devise, le montant est exclu et l'agrégat EXPLICITEMENT
    // incomplet — la part convertible est conservée, jamais un total silencieusement amputé/nul.
    signup(&pool, "agg-partial@example.com").await;
    let web = login_cookie(&pool, "agg-partial@example.com").await;

    let res = aggregate(
        &pool,
        &web,
        json!({
            "target": "USD",
            "amounts": [
                { "amount": "10", "currency": "EUR" },
                { "amount": "20", "currency": "USD" }
            ]
        }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = body_json(res).await;
    // EUR->USD inconnu (exclu) ; USD (identité) conservé -> 20, MAIS signalé incomplet.
    assert_eq!(body["total"], "20");
    assert_eq!(body["converted"], 1);
    assert_eq!(body["excluded"], 1);
    assert_eq!(body["complete"], false);
    assert_eq!(body["as_of"], serde_json::Value::Null);
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-CUR-004)]
async fn aggregate_with_invalid_input_is_422(pool: PgPool) {
    signup(&pool, "agg-bad@example.com").await;
    let web = login_cookie(&pool, "agg-bad@example.com").await;

    // Devise cible mal formée (pas 3 lettres) -> 422.
    assert_eq!(
        aggregate(&pool, &web, json!({ "target": "US", "amounts": [] }))
            .await
            .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    // Devise bien formée mais HORS RÉFÉRENTIEL supporté : rejetée côté serveur (REQ-CUR-007 #1).
    assert_eq!(
        aggregate(&pool, &web, json!({ "target": "ZZZ", "amounts": [] }))
            .await
            .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    // Montant illisible : jamais traité comme zéro, rejeté en 422.
    assert_eq!(
        aggregate(
            &pool,
            &web,
            json!({ "target": "USD", "amounts": [{ "amount": "not-a-number", "currency": "EUR" }] })
        )
        .await
        .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
}

// --- Autorisation §9 : aggregateConverted (protégé ; taux = donnée globale, pas de portée foyer) ---

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-CUR-004)]
async fn authz_owner_aggregate_converted(pool: PgPool) {
    signup(&pool, "owner-agg@example.com").await;
    let web = login_cookie(&pool, "owner-agg@example.com").await;
    assert_eq!(
        aggregate(&pool, &web, json!({ "target": "EUR", "amounts": [] }))
            .await
            .status(),
        StatusCode::OK
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-CUR-004)]
async fn authz_other_aggregate_converted(pool: PgPool) {
    // Les taux sont une donnée de référence globale : un autre compte authentifié y accède aussi
    // (aucune entité possédée n'est exposée, comme l'endpoint de santé).
    signup(&pool, "other-agg@example.com").await;
    let web = login_cookie(&pool, "other-agg@example.com").await;
    assert_eq!(
        aggregate(&pool, &web, json!({ "target": "EUR", "amounts": [] }))
            .await
            .status(),
        StatusCode::OK
    );
}

#[sqlx::test(migrations = "../storage/migrations")]
#[verifies(REQ-CUR-004)]
async fn authz_anon_aggregate_converted(pool: PgPool) {
    // Sans session : 401 (l'endpoint exige une authentification).
    let res = post_json(
        &pool,
        "/api/v1/exchange/aggregate",
        json!({ "target": "EUR", "amounts": [] }),
        None,
    )
    .await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

fn day(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap()
}
