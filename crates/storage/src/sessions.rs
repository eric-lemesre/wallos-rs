//! Repository des sessions serveur (REQ-AUT-002).
//!
//! Le jeton opaque n'est jamais stocké : seule son empreinte SHA-256 (`token_hash`, calculée côté
//! `server`, ADR 0018) l'est. `find_valid` filtre l'expiration avec un instant **injecté** (jamais
//! l'horloge — cohérent avec le principe de reproductibilité de REQ-STA-008).

use chrono::{DateTime, Utc};
use uuid::Uuid;
use wallos_core::actor::Actor;
use wallos_core::requirement;

use crate::StorageError;

/// Accès aux sessions.
pub struct SessionRepository<'a> {
    pool: &'a sqlx::PgPool,
}

impl<'a> SessionRepository<'a> {
    /// Construit le repository sur un pool.
    #[must_use]
    #[requirement(REQ-AUT-002)]
    pub const fn new(pool: &'a sqlx::PgPool) -> Self {
        Self { pool }
    }

    /// Ouvre une session pour un utilisateur.
    ///
    /// # Errors
    /// `StorageError::Database` en cas d'échec d'insertion.
    #[requirement(REQ-AUT-002)]
    pub async fn create(
        &self,
        actor: &Actor,
        token_hash: &[u8],
        expires_at: DateTime<Utc>,
    ) -> Result<(), StorageError> {
        sqlx::query(
            "insert into sessions (token_hash, user_id, household_id, expires_at) \
             values ($1, $2, $3, $4)",
        )
        .bind(token_hash)
        .bind(actor.user_id())
        .bind(actor.household_id())
        .bind(expires_at)
        .execute(self.pool)
        .await?;
        Ok(())
    }

    /// Retrouve le contexte d'appelant d'une session **valide** (non expirée à `now`).
    ///
    /// Renvoie `None` si le jeton est inconnu ou la session expirée — l'appelant traduit en `401`.
    ///
    /// # Errors
    /// `StorageError::Database` en cas d'échec de requête.
    #[requirement(REQ-AUT-002)]
    pub async fn find_valid(
        &self,
        token_hash: &[u8],
        now: DateTime<Utc>,
    ) -> Result<Option<Actor>, StorageError> {
        let row: Option<(Uuid, Uuid)> = sqlx::query_as(
            "select user_id, household_id from sessions \
             where token_hash = $1 and expires_at > $2",
        )
        .bind(token_hash)
        .bind(now)
        .fetch_optional(self.pool)
        .await?;
        Ok(row.map(|(user_id, household_id)| Actor::new(user_id, household_id)))
    }
}
