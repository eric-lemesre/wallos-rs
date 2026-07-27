//! Récupération et rafraîchissement des taux de change (REQ-CUR-003).
//!
//! Réconciliation ADR 0014 ↔ `core` (zéro async/I/O) : le trait [`RateProvider`](wallos_core::RateProvider)
//! de `core` est un **lookup pur** (conversion). Ici vit le côté **fetch** : le trait [`RateSource`]
//! (async) modélise un fournisseur ; [`refresh_rates`] persiste ce qu'il fournit (avec date de
//! validité + source) ; [`load_rate_table`] reconstruit une `RateTable` depuis les taux stockés —
//! l'adaptateur « dernier taux connu », **toujours disponible** (l'app reste fonctionnelle sans
//! aucun fournisseur configuré, l'agrégat étant alors signalé partiel).

use chrono::{DateTime, Utc};
use wallos_core::requirement;
use wallos_core::{ExchangeRate, RateTable};
use wallos_storage::{ExchangeRateRepository, StorageError};

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
