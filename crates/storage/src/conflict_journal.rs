//! Journal des conflits de synchronisation (REQ-SYN-005).
//!
//! Conserve les **versions perdues par arbitrage** (dernière écriture gagnante) pour consultation par
//! l'utilisateur : une modification écrasée par une écriture concurrente (`overwritten`) ou écartée par
//! une suppression concurrente (`deleted_remotely`). Isolation §9 : filtré par `household_id`. La purge
//! (rétention, ADR 0013) prend une borne **injectée** (testable sans horloge).

use chrono::{DateTime, Utc};
use uuid::Uuid;
use wallos_core::actor::Actor;
use wallos_core::requirement;

use crate::StorageError;

/// Motif : la version courante a été **écrasée** par une écriture fondée sur une version périmée.
pub const REASON_OVERWRITTEN: &str = "overwritten";
/// Motif : la modification a été **écartée** par une suppression concurrente (la suppression l'emporte).
pub const REASON_DELETED_REMOTELY: &str = "deleted_remotely";

/// Une entrée du journal des conflits.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ConflictRow {
    /// Type d'entité concernée.
    pub entity_type: String,
    /// Identifiant de l'entité concernée.
    pub entity_id: Uuid,
    /// Version perdue (JSON de l'entité, tel que capturé).
    pub lost_payload: String,
    /// Motif (`overwritten` / `deleted_remotely`).
    pub reason: String,
    /// Horodatage serveur d'enregistrement.
    pub recorded_at: DateTime<Utc>,
}

/// Accès au journal des conflits.
pub struct ConflictJournalRepository<'a> {
    pool: &'a sqlx::PgPool,
}

impl<'a> ConflictJournalRepository<'a> {
    /// Construit le repository sur un pool.
    #[must_use]
    #[requirement(REQ-SYN-005)]
    pub const fn new(pool: &'a sqlx::PgPool) -> Self {
        Self { pool }
    }

    /// Journalise une version perdue **dans le foyer de l'appelant** (§9).
    ///
    /// # Errors
    /// `StorageError::Database` en cas d'échec d'insertion.
    #[requirement(REQ-SYN-005)]
    pub async fn record(
        &self,
        actor: &Actor,
        entity_type: &str,
        entity_id: Uuid,
        lost_payload: &str,
        reason: &str,
    ) -> Result<(), StorageError> {
        sqlx::query(
            "insert into conflict_journal (household_id, entity_type, entity_id, lost_payload, reason) \
             values ($1, $2, $3, $4, $5)",
        )
        .bind(actor.household_id())
        .bind(entity_type)
        .bind(entity_id)
        .bind(lost_payload)
        .bind(reason)
        .execute(self.pool)
        .await?;
        Ok(())
    }

    /// Entrées du journal **du foyer de l'appelant**, de la plus récente à la plus ancienne.
    ///
    /// # Errors
    /// `StorageError::Database` en cas d'échec de requête.
    #[requirement(REQ-SYN-005)]
    pub async fn list(&self, actor: &Actor) -> Result<Vec<ConflictRow>, StorageError> {
        let rows = sqlx::query_as::<_, ConflictRow>(
            "select entity_type, entity_id, lost_payload, reason, recorded_at \
             from conflict_journal where household_id = $1 \
             order by recorded_at desc, entity_id asc",
        )
        .bind(actor.household_id())
        .fetch_all(self.pool)
        .await?;
        Ok(rows)
    }

    /// Purge les entrées antérieures à `cutoff` (borne calculée par l'appelant = `now − rétention`).
    /// Renvoie le nombre d'entrées supprimées. Borne injectée → testable sans horloge.
    ///
    /// # Errors
    /// `StorageError::Database` en cas d'échec de requête.
    #[requirement(REQ-SYN-005)]
    pub async fn purge_expired(&self, cutoff: DateTime<Utc>) -> Result<u64, StorageError> {
        let result = sqlx::query("delete from conflict_journal where recorded_at < $1")
            .bind(cutoff)
            .execute(self.pool)
            .await?;
        Ok(result.rows_affected())
    }
}
