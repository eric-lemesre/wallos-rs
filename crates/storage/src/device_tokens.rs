//! Repository des jetons d'appareil (REQ-AUT-005 / REQ-AUT-006).
//!
//! Un jeton d'appareil est un jeton opaque long, propre à une coquille native, révocable
//! individuellement par son `id`. Comme pour les sessions, le jeton clair n'est jamais stocké :
//! seule son empreinte SHA-256 (calculée côté `server`, ADR 0018) l'est. L'instant (`now`) est
//! toujours **injecté** — jamais l'horloge — pour un comportement reproductible.

use chrono::{DateTime, Utc};
use uuid::Uuid;
use wallos_core::actor::Actor;
use wallos_core::requirement;

use crate::StorageError;

/// Appareil appairé, exposé aux lectures autorisées (REQ-AUT-006).
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct DeviceRow {
    /// Identifiant de l'appareil (clé de révocation).
    pub id: Uuid,
    /// Libellé lisible.
    pub label: String,
    /// Plateforme.
    pub platform: String,
    /// Dernière activité observée.
    pub last_seen_at: DateTime<Utc>,
}

/// Accès aux jetons d'appareil.
pub struct DeviceTokenRepository<'a> {
    pool: &'a sqlx::PgPool,
}

impl<'a> DeviceTokenRepository<'a> {
    /// Construit le repository sur un pool.
    #[must_use]
    #[requirement(REQ-AUT-005)]
    pub const fn new(pool: &'a sqlx::PgPool) -> Self {
        Self { pool }
    }

    /// Appaire un appareil : enregistre son jeton (empreinte), avec libellé et plateforme.
    ///
    /// # Errors
    /// `StorageError::Database` en cas d'échec d'insertion.
    #[requirement(REQ-AUT-005)]
    pub async fn create(
        &self,
        actor: &Actor,
        id: Uuid,
        token_hash: &[u8],
        label: &str,
        platform: &str,
        now: DateTime<Utc>,
    ) -> Result<(), StorageError> {
        sqlx::query(
            "insert into device_tokens \
             (id, token_hash, user_id, household_id, label, platform, last_seen_at) \
             values ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(id)
        .bind(token_hash)
        .bind(actor.user_id())
        .bind(actor.household_id())
        .bind(label)
        .bind(platform)
        .bind(now)
        .execute(self.pool)
        .await?;
        Ok(())
    }

    /// Valide un jeton d'appareil et rafraîchit sa dernière activité (`last_seen_at = now`).
    ///
    /// Renvoie le contexte d'appelant **et l'identifiant de l'appareil** (pour distinguer l'appareil
    /// courant, REQ-AUT-006). `None` si le jeton est inconnu ou révoqué — l'appelant traduit en `401`.
    ///
    /// # Errors
    /// `StorageError::Database` en cas d'échec de requête.
    #[requirement(REQ-AUT-005)]
    pub async fn find_valid(
        &self,
        token_hash: &[u8],
        now: DateTime<Utc>,
    ) -> Result<Option<(Actor, Uuid)>, StorageError> {
        let row: Option<(Uuid, Uuid, Uuid)> = sqlx::query_as(
            "update device_tokens set last_seen_at = $2 where token_hash = $1 \
             returning user_id, household_id, id",
        )
        .bind(token_hash)
        .bind(now)
        .fetch_optional(self.pool)
        .await?;
        Ok(row.map(|(user_id, household_id, id)| (Actor::new(user_id, household_id), id)))
    }

    /// Liste les appareils appairés **du foyer de l'appelant** (garde-fou d'isolation, ADR 0006/0012).
    ///
    /// Triés par activité décroissante (le plus récemment vu en premier).
    ///
    /// # Errors
    /// `StorageError::Database` en cas d'échec de requête.
    #[requirement(REQ-AUT-006)]
    pub async fn list(&self, actor: &Actor) -> Result<Vec<DeviceRow>, StorageError> {
        let rows = sqlx::query_as::<_, DeviceRow>(
            "select id, label, platform, last_seen_at from device_tokens \
             where household_id = $1 order by last_seen_at desc",
        )
        .bind(actor.household_id())
        .fetch_all(self.pool)
        .await?;
        Ok(rows)
    }

    /// Révoque un appareil **du foyer de l'appelant** (révocation immédiate, REQ-AUT-006).
    ///
    /// Renvoie `true` si un appareil a été supprimé, `false` s'il n'existe pas *ou* appartient à un
    /// autre foyer — l'appelant traduit ce `false` en `404`, jamais `403` (AGENTS.md §9).
    ///
    /// # Errors
    /// `StorageError::Database` en cas d'échec de requête.
    #[requirement(REQ-AUT-006)]
    pub async fn revoke(&self, actor: &Actor, id: Uuid) -> Result<bool, StorageError> {
        let result = sqlx::query("delete from device_tokens where id = $1 and household_id = $2")
            .bind(id)
            .bind(actor.household_id())
            .execute(self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}
