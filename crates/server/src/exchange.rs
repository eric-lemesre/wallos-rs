//! Récupération et rafraîchissement des taux de change (REQ-CUR-003).
//!
//! Réconciliation ADR 0014 ↔ `core` (zéro async/I/O) : le trait [`RateProvider`](wallos_core::RateProvider)
//! de `core` est un **lookup pur** (conversion). Ici vit le côté **fetch** : le trait [`RateSource`]
//! (async) modélise un fournisseur ; [`refresh_rates`] persiste ce qu'il fournit (avec date de
//! validité + source) ; [`load_rate_table`] reconstruit une `RateTable` depuis les taux stockés —
//! l'adaptateur « dernier taux connu », **toujours disponible** (l'app reste fonctionnelle sans
//! aucun fournisseur configuré, l'agrégat étant alors signalé partiel).

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use wallos_core::money::{CurrencyCode, Money};
use wallos_core::requirement;
use wallos_core::{ExchangeRate, RateTable, aggregate_converted};
use wallos_proto::{AggregateRequest, ConvertedTotalResponse, MoneyInput, problem};
use wallos_storage::{Db, ExchangeRateRepository, StorageError};

use crate::auth::AuthActor;
use crate::problem_response;

/// Source de taux (fournisseur). Le fetch peut échouer (réseau, quota) — d'où le repli en chaîne
/// vers l'adaptateur « dernier taux connu » (ADR 0014). Les implémentations HTTP concrètes viendront
/// plus tard (chacune sous son ADR).
pub trait RateSource {
    /// Récupère les taux courants du fournisseur.
    fn fetch(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<ExchangeRate>, RateSourceError>> + Send;
}

/// Échec de récupération auprès d'un fournisseur de taux.
#[derive(Debug, thiserror::Error)]
pub enum RateSourceError {
    /// Le fournisseur est indisponible ou a renvoyé une réponse invalide.
    #[error("rate source unavailable: {0}")]
    Unavailable(String),
}

/// Rafraîchit les taux : récupère ceux de `source` et les **persiste** (date de validité + source).
///
/// Renvoie le nombre de taux persistés. C'est l'opération déclenchée par la mise à jour périodique
/// (le déclencheur temporel est du câblage d'ops, hors de ce vertical).
///
/// # Errors
/// [`RateSourceError`] si la récupération échoue ; [`StorageError`] si la persistance échoue.
#[requirement(REQ-CUR-003)]
pub async fn refresh_rates(
    source: &impl RateSource,
    repo: &ExchangeRateRepository<'_>,
    now: DateTime<Utc>,
) -> Result<usize, RefreshError> {
    let rates = source.fetch().await?;
    for rate in &rates {
        repo.upsert(
            rate.base().as_str(),
            rate.quote().as_str(),
            rate.rate(),
            rate.as_of(),
            rate.source(),
            now,
        )
        .await?;
    }
    Ok(rates.len())
}

/// Échec de [`refresh_rates`] : récupération ou persistance.
#[derive(Debug, thiserror::Error)]
pub enum RefreshError {
    /// La récupération auprès du fournisseur a échoué.
    #[error(transparent)]
    Source(#[from] RateSourceError),
    /// La persistance a échoué.
    #[error(transparent)]
    Storage(#[from] StorageError),
}

/// Construit la `RateTable` de `core` à partir des derniers taux connus stockés.
///
/// Toujours disponible : une base vide produit une table vide (les conversions renverront `None`,
/// l'agrégat sera signalé partiel — jamais une panne).
///
/// # Errors
/// [`StorageError`] en cas d'échec de lecture ; ignore silencieusement un taux stocké invalide
/// (`rate <= 0`), qui ne devrait pas exister vu la contrainte d'écriture.
#[requirement(REQ-CUR-003)]
pub async fn load_rate_table(repo: &ExchangeRateRepository<'_>) -> Result<RateTable, StorageError> {
    let stored = repo.all_latest().await?;
    let mut rates = Vec::with_capacity(stored.len());
    for row in stored {
        let (Ok(base), Ok(quote)) = (
            wallos_core::money::CurrencyCode::new(&row.base_currency),
            wallos_core::money::CurrencyCode::new(&row.quote_currency),
        ) else {
            continue;
        };
        if let Ok(rate) = ExchangeRate::new(base, quote, row.rate, row.as_of, row.source) {
            rates.push(rate);
        }
    }
    Ok(RateTable::new(rates))
}

/// Convertit un `MoneyInput` (chaînes) en `Money` du domaine, ou `None` si le montant ou la devise
/// sont invalides (→ 422 côté handler). Aucune tolérance : un montant illisible n'est jamais traité
/// comme zéro.
#[requirement(REQ-CUR-004)]
fn parse_money(input: &MoneyInput) -> Option<Money> {
    let amount = input.amount.parse::<Decimal>().ok()?;
    let currency = CurrencyCode::new(&input.currency).ok()?;
    Money::new(amount, currency).ok()
}

/// Agrège des montants multi-devises vers une devise cible, en **mode dégradé** explicite
/// (REQ-CUR-004).
///
/// La table de taux est reconstruite depuis les **derniers taux connus** persistés
/// ([`load_rate_table`]) : sans fournisseur configuré (ou s'il est indisponible), l'app reste
/// fonctionnelle et retombe sur ces taux. Chaque montant sans taux est **exclu** et l'agrégat est
/// signalé incomplet (`complete = false`), jamais présenté comme un zéro silencieux ; `as_of` porte
/// la fraîcheur (date du taux le plus ancien utilisé) que l'interface doit afficher.
#[utoipa::path(
    post,
    path = "/exchange/aggregate",
    operation_id = "aggregateConverted",
    extensions(("x-requirements" = json!(["REQ-CUR-004"]))),
    request_body = AggregateRequest,
    responses(
        (status = 200, description = "Agrégat converti (éventuellement partiel)", body = ConvertedTotalResponse, content_type = "application/json"),
        (
            status = 401,
            description = "Non authentifié",
            body = wallos_proto::Problem,
            content_type = "application/problem+json"
        ),
        (
            status = 422,
            description = "Devise ou montant invalide",
            body = wallos_proto::Problem,
            content_type = "application/problem+json"
        )
    )
)]
#[requirement(REQ-CUR-004)]
pub async fn aggregate_converted_handler(
    // Authentification requise (anon → 401). Les taux sont une donnée de référence **globale** :
    // pas de portée par foyer, donc l'`Actor` n'est pas utilisé pour filtrer.
    AuthActor(_actor): AuthActor,
    State(db): State<Db>,
    Json(req): Json<AggregateRequest>,
) -> Response {
    let Ok(target) = CurrencyCode::new(&req.target) else {
        return problem_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            problem(422, "about:blank", "Unprocessable Entity"),
        );
    };

    let mut amounts = Vec::with_capacity(req.amounts.len());
    for input in &req.amounts {
        let Some(money) = parse_money(input) else {
            return problem_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                problem(422, "about:blank", "Unprocessable Entity"),
            );
        };
        amounts.push(money);
    }

    let repo = ExchangeRateRepository::new(db.pool());
    let table = match load_rate_table(&repo).await {
        Ok(t) => t,
        Err(_) => {
            return problem_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                problem(500, "about:blank", "Internal Server Error"),
            );
        }
    };

    let agg = aggregate_converted(&amounts, target, &table);
    let body = ConvertedTotalResponse {
        total: agg.total().amount().to_string(),
        currency: target.as_str().to_string(),
        converted: agg.converted() as u32,
        excluded: agg.excluded() as u32,
        complete: agg.is_complete(),
        as_of: agg.as_of().map(|d| d.to_string()),
    };
    (StatusCode::OK, Json(body)).into_response()
}
