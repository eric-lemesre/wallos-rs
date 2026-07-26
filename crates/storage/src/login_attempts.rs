//! Repository des tentatives d'authentification échouées (REQ-AUT-008).
//!
//! Journalise chaque tentative échouée (par e-mail et par IP source) pour dériver les compteurs de
//! la fenêtre glissante. L'instant (`now`) est toujours **injecté** — jamais l'horloge — afin de
//! rendre le comptage reproductible et testable de façon déterministe (cohérent avec
//! `SessionRepository::find_valid`).

use chrono::{DateTime, Utc};
use wallos_core::requirement;

use crate::StorageError;

/// Accès aux tentatives d'authentification.
pub struct LoginAttemptRepository<'a> {
    pool: &'a sqlx::PgPool,
}

impl<'a> LoginAttemptRepository<'a> {
    /// Construit le repository sur un pool.
    #[must_use]
    #[requirement(REQ-AUT-008)]
    pub const fn new(pool: &'a sqlx::PgPool) -> Self {
        Self { pool }
    }

    /// Journalise une tentative échouée pour un e-mail (et, si connue, une IP source).
    ///
    /// # Errors
    /// `StorageError::Database` en cas d'échec d'insertion.
    #[requirement(REQ-AUT-008)]
    pub async fn record_failure(
        &self,
        email: &str,
        ip: Option<&str>,
        now: DateTime<Utc>,
    ) -> Result<(), StorageError> {
        sqlx::query("insert into login_attempts (email, ip, created_at) values ($1, $2, $3)")
            .bind(email)
            .bind(ip)
            .bind(now)
            .execute(self.pool)
            .await?;
        Ok(())
    }

    /// Nombre d'échecs et instant du plus ancien pour un e-mail, dans la fenêtre `created_at > since`.
    ///
    /// L'instant du plus ancien permet de calculer le `Retry-After` (fin de fenêtre glissante).
    ///
    /// # Errors
    /// `StorageError::Database` en cas d'échec de requête.
    #[requirement(REQ-AUT-008)]
    pub async fn count_and_earliest_email(
        &self,
        email: &str,
        since: DateTime<Utc>,
    ) -> Result<(i64, Option<DateTime<Utc>>), StorageError> {
        let row: (i64, Option<DateTime<Utc>>) = sqlx::query_as(
            "select count(*), min(created_at) from login_attempts \
             where email = $1 and created_at > $2",
        )
        .bind(email)
        .bind(since)
        .fetch_one(self.pool)
        .await?;
        Ok(row)
    }

    /// Nombre d'échecs et instant du plus ancien pour une IP, dans la fenêtre `created_at > since`.
    ///
    /// # Errors
    /// `StorageError::Database` en cas d'échec de requête.
    #[requirement(REQ-AUT-008)]
    pub async fn count_and_earliest_ip(
        &self,
        ip: &str,
        since: DateTime<Utc>,
    ) -> Result<(i64, Option<DateTime<Utc>>), StorageError> {
        let row: (i64, Option<DateTime<Utc>>) = sqlx::query_as(
            "select count(*), min(created_at) from login_attempts \
             where ip = $1 and created_at > $2",
        )
        .bind(ip)
        .bind(since)
        .fetch_one(self.pool)
        .await?;
        Ok(row)
    }

    /// Réinitialise le compteur d'un e-mail (appelé après une authentification réussie).
    ///
    /// # Errors
    /// `StorageError::Database` en cas d'échec de requête.
    #[requirement(REQ-AUT-008)]
    pub async fn clear_email(&self, email: &str) -> Result<(), StorageError> {
        sqlx::query("delete from login_attempts where email = $1")
            .bind(email)
            .execute(self.pool)
            .await?;
        Ok(())
    }
}
