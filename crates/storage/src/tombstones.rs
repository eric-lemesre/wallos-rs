//! Repository des pierres tombales (REQ-SYN-002).
//!
//! Une pierre tombale trace la **suppression** d'une entité possédée (catégorie, moyen de paiement,
//! payeur) pour qu'un appareil hors ligne applique la suppression au lieu de réintroduire l'entité.
//! Entité **possédée** : lecture filtrée par `household_id` (§9). `deleted_at` est **fourni par le
//! serveur** (`default now()`, REQ-SYN-001). L'enregistrement se fait **dans la transaction** de la
//! suppression (atomicité : jamais de suppression sans pierre tombale) via [`record`], générique sur
//! l'exécuteur (pool ou transaction). La **purge** (rétention, ADR 0013) prend une borne **injectée**
//! pour rester testable sans horloge (REQ-STA-008).

use chrono::{DateTime, Utc};
use uuid::Uuid;
use wallos_core::actor::Actor;
use wallos_core::requirement;

use crate::StorageError;

/// Type d'entité « catégorie » dans une pierre tombale.
pub const ENTITY_CATEGORY: &str = "category";
/// Type d'entité « moyen de paiement ».
pub const ENTITY_PAYMENT_METHOD: &str = "payment_method";
/// Type d'entité « payeur ».
pub const ENTITY_PAYER: &str = "payer";

/// Pierre tombale exposée à la synchronisation.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct TombstoneRow {
    /// Type d'entité supprimée (`category` / `payment_method` / `payer`).
    pub entity_type: String,
    /// Identifiant de l'entité supprimée (UUID stable, REQ-SYN-001).
    pub entity_id: Uuid,
    /// Date de suppression, **fournie par le serveur** — sert de curseur `since`.
    pub deleted_at: DateTime<Utc>,
}

/// Enregistre une pierre tombale pour l'entité `entity_id` de type `entity_type`, dans le foyer
/// `household_id`. Générique sur l'exécuteur : appelée **dans la transaction** d'une suppression (pour
/// l'atomicité), ou sur un pool. Idempotent : resupprimer la même entité (après recréation) **rafraîchit**
/// `deleted_at` plutôt que d'échouer sur la contrainte d'unicité.
///
/// # Errors
/// `StorageError::Database` en cas d'échec d'insertion.
#[requirement(REQ-SYN-002)]
pub async fn record<'e, E>(
    executor: E,
    household_id: Uuid,
    entity_type: &str,
    entity_id: Uuid,
) -> Result<(), StorageError>
where
    E: sqlx::PgExecutor<'e>,
{
    sqlx::query(
        "insert into tombstones (household_id, entity_type, entity_id) values ($1, $2, $3) \
         on conflict (household_id, entity_type, entity_id) do update set deleted_at = now()",
    )
    .bind(household_id)
    .bind(entity_type)
    .bind(entity_id)
    .execute(executor)
    .await?;
    Ok(())
}

/// Accès aux pierres tombales.
pub struct TombstoneRepository<'a> {
    pool: &'a sqlx::PgPool,
}

impl<'a> TombstoneRepository<'a> {
    /// Construit le repository sur un pool.
    #[must_use]
    #[requirement(REQ-SYN-002)]
    pub const fn new(pool: &'a sqlx::PgPool) -> Self {
        Self { pool }
    }

    /// Pierres tombales **du foyer de l'appelant** (§9), ordonnées par date de suppression croissante
    /// (puis id, ordre déterministe). Si `since` est fourni, ne renvoie que les suppressions
    /// **strictement postérieures** (curseur exclusif : le client rappelle avec le `deleted_at` du
    /// dernier reçu sans le redemander).
    ///
    /// # Errors
    /// `StorageError::Database` en cas d'échec de requête.
    #[requirement(REQ-SYN-002)]
    pub async fn list_since(
        &self,
        actor: &Actor,
        since: Option<DateTime<Utc>>,
    ) -> Result<Vec<TombstoneRow>, StorageError> {
        let rows = sqlx::query_as::<_, TombstoneRow>(
            "select entity_type, entity_id, deleted_at from tombstones \
             where household_id = $1 and ($2::timestamptz is null or deleted_at > $2) \
             order by deleted_at asc, entity_id asc",
        )
        .bind(actor.household_id())
        .bind(since)
        .fetch_all(self.pool)
        .await?;
        Ok(rows)
    }

    /// Purge les pierres tombales dont `deleted_at` est **strictement antérieur** à `cutoff` (borne
    /// calculée par l'appelant = `now − rétention`, ADR 0013). Purge **globale** (maintenance serveur,
    /// tous foyers) ; renvoie le nombre de lignes supprimées. Borne injectée → testable sans horloge.
    ///
    /// # Errors
    /// `StorageError::Database` en cas d'échec de requête.
    #[requirement(REQ-SYN-002)]
    pub async fn purge_expired(&self, cutoff: DateTime<Utc>) -> Result<u64, StorageError> {
        let result = sqlx::query("delete from tombstones where deleted_at < $1")
            .bind(cutoff)
            .execute(self.pool)
            .await?;
        Ok(result.rows_affected())
    }
}
