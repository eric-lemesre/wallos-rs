//! Repository des taux de change (REQ-CUR-003).
//!
//! Donnée de **référence globale** (marché) : pas de contexte d'appelant `&Actor` — l'isolation §9
//! protège les données *de compte*, pas les taux, partagés par tous. `fetched_at` est un instant
//! **injecté** (jamais l'horloge). Les taux persistés alimentent la `RateTable` de `core`
//! (adaptateur « dernier taux connu »).

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use wallos_core::requirement;

use crate::StorageError;

/// Taux stocké, exposé aux lectures.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct StoredRate {
    /// Devise de base (code ISO, dénominateur).
    pub base_currency: String,
    /// Devise cotée (numérateur).
    pub quote_currency: String,
    /// Valeur du taux (`quote` par unité de `base`).
    pub rate: Decimal,
    /// Date de validité du taux.
    pub as_of: NaiveDate,
    /// Origine du taux.
    pub source: String,
}

/// Accès aux taux de change.
pub struct ExchangeRateRepository<'a> {
    pool: &'a sqlx::PgPool,
}

impl<'a> ExchangeRateRepository<'a> {
    /// Construit le repository sur un pool.
    #[must_use]
    #[requirement(REQ-CUR-003)]
    pub const fn new(pool: &'a sqlx::PgPool) -> Self {
        Self { pool }
    }

    /// Enregistre (ou met à jour) un taux pour une paire à une date de validité donnée.
    ///
    /// # Errors
    /// `StorageError::Database` en cas d'échec de requête.
    #[requirement(REQ-CUR-003)]
    pub async fn upsert(
        &self,
        base: &str,
        quote: &str,
        rate: Decimal,
        as_of: NaiveDate,
        source: &str,
        now: DateTime<Utc>,
    ) -> Result<(), StorageError> {
        sqlx::query(
            "insert into exchange_rates \
             (base_currency, quote_currency, rate, as_of, source, fetched_at) \
             values ($1, $2, $3, $4, $5, $6) \
             on conflict (base_currency, quote_currency, as_of) \
             do update set rate = excluded.rate, source = excluded.source, \
                           fetched_at = excluded.fetched_at",
        )
        .bind(base)
        .bind(quote)
        .bind(rate)
        .bind(as_of)
        .bind(source)
        .bind(now)
        .execute(self.pool)
        .await?;
        Ok(())
    }

    /// Taux le plus récent (par date de validité) pour une paire, s'il existe.
    ///
    /// # Errors
    /// `StorageError::Database` en cas d'échec de requête.
    #[requirement(REQ-CUR-003)]
    pub async fn latest(
        &self,
        base: &str,
        quote: &str,
    ) -> Result<Option<StoredRate>, StorageError> {
        let row = sqlx::query_as::<_, StoredRate>(
            "select base_currency, quote_currency, rate, as_of, source \
             from exchange_rates where base_currency = $1 and quote_currency = $2 \
             order by as_of desc limit 1",
        )
        .bind(base)
        .bind(quote)
        .fetch_optional(self.pool)
        .await?;
        Ok(row)
    }

    /// Dernier taux connu de **chaque** paire (pour construire la `RateTable` de `core`).
    ///
    /// # Errors
    /// `StorageError::Database` en cas d'échec de requête.
    #[requirement(REQ-CUR-003)]
    pub async fn all_latest(&self) -> Result<Vec<StoredRate>, StorageError> {
        let rows = sqlx::query_as::<_, StoredRate>(
            "select distinct on (base_currency, quote_currency) \
             base_currency, quote_currency, rate, as_of, source \
             from exchange_rates \
             order by base_currency, quote_currency, as_of desc",
        )
        .fetch_all(self.pool)
        .await?;
        Ok(rows)
    }
}
