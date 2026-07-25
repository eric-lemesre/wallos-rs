//! Connexion à la base et migrations.
//!
//! REQ-AUT-001 — la couche `storage` (PostgreSQL via sqlx, ADR 0010) est introduite avec le
//! premier vertical persistant. `Db` encapsule le pool ; les migrations sont embarquées à la
//! compilation depuis `migrations/`.

use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use wallos_core::requirement;

use crate::StorageError;

/// Poignée de base de données : un pool de connexions PostgreSQL.
#[derive(Debug, Clone)]
pub struct Db {
    pool: PgPool,
}

impl Db {
    /// Ouvre un pool vers l'URL PostgreSQL fournie.
    ///
    /// # Errors
    /// `StorageError::Database` si la connexion initiale échoue.
    #[requirement(REQ-AUT-001)]
    pub async fn connect(url: &str) -> Result<Self, StorageError> {
        let pool = PgPoolOptions::new().connect(url).await?;
        Ok(Self { pool })
    }

    /// Construit une poignée à partir d'un pool existant (tests, injection).
    #[must_use]
    #[requirement(REQ-AUT-001)]
    pub const fn from_pool(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Applique les migrations embarquées.
    ///
    /// # Errors
    /// `StorageError::Migration` si une migration échoue.
    #[requirement(REQ-AUT-001)]
    pub async fn migrate(&self) -> Result<(), StorageError> {
        sqlx::migrate!()
            .run(&self.pool)
            .await
            .map_err(|e| StorageError::Migration(e.to_string()))
    }

    /// Accès en lecture au pool sous-jacent.
    #[must_use]
    #[requirement(REQ-AUT-001)]
    pub const fn pool(&self) -> &PgPool {
        &self.pool
    }
}
