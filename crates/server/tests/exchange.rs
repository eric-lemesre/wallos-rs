//! Tests d'intégration des taux de change (REQ-CUR-003).
//!
//! Couvre les deux critères d'acceptation : (1) la récupération **persiste** les taux avec leur
//! date de validité et leur source ; (2) sans taux (aucun fournisseur), l'agrégation reste
//! fonctionnelle et est **signalée partielle** — jamais un total silencieusement amputé.

use chrono::{NaiveDate, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;
use wallos_core::money::{CurrencyCode, Money};
use wallos_core::{ExchangeRate, aggregate_converted};
use wallos_req_macros::verifies;
use wallos_server::exchange::{RateSource, RateSourceError, load_rate_table, refresh_rates};
use wallos_storage::ExchangeRateRepository;

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
