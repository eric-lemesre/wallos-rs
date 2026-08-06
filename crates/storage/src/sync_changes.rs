//! Récupération incrémentale des changements par curseur (REQ-SYN-003).
//!
//! Un **flux unifié** des changements du foyer — créations et modifications (`upsert`) des catégories,
//! moyens de paiement, payeurs et abonnements, plus les suppressions (`delete`, pierres tombales
//! REQ-SYN-002) — trié par la clé totale `(horodatage, id)`. La pagination est **keyset** (comparaison
//! de tuple Postgres `(ts, id) > (curseur)`) : stable et sans omission ni duplication entre pages
//! (REQ-SYN-003 critère #2). Isolation §9 : filtré par `household_id`. Le corps d'un `upsert` est
//! l'entité (**ligne persistée**) en JSON **moins `household_id`** (jamais divulgué au client). Le champ
//! monétaire `subscriptions.amount` est **coercé en chaîne** (`::text`) : jamais un nombre JSON, qui
//! serait interprété en flottant côté client (R4 — l'argent reste décimal exact).

use chrono::{DateTime, Utc};
use uuid::Uuid;
use wallos_core::SyncCursor;
use wallos_core::actor::Actor;
use wallos_core::requirement;

use crate::StorageError;

/// Un changement du flux de synchronisation.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ChangeRow {
    /// `upsert` (création/modification) ou `delete` (suppression).
    pub kind: String,
    /// Type d'entité (`category` / `payment_method` / `payer` / `subscription`).
    pub entity_type: String,
    /// Identifiant de l'entité.
    pub id: Uuid,
    /// Horodatage du changement (`updated_at` pour un upsert, `deleted_at` pour un delete) : composante
    /// haute de la clé de tri/curseur.
    pub ts: DateTime<Utc>,
    /// Corps JSON de l'entité (upsert), déjà sérialisé et privé de `household_id` ; `None` pour un delete.
    pub payload: Option<String>,
}

/// Accès au flux de changements.
pub struct SyncChangesRepository<'a> {
    pool: &'a sqlx::PgPool,
}

impl<'a> SyncChangesRepository<'a> {
    /// Construit le repository sur un pool.
    #[must_use]
    #[requirement(REQ-SYN-003)]
    pub const fn new(pool: &'a sqlx::PgPool) -> Self {
        Self { pool }
    }

    /// Changements du foyer **strictement postérieurs** au curseur `after`, triés par `(ts, id)`
    /// croissant, limités à `limit` lignes (l'appelant demande `page + 1` pour détecter la présence
    /// d'une page suivante).
    ///
    /// # Errors
    /// `StorageError::Database` en cas d'échec de requête.
    #[requirement(REQ-SYN-003)]
    pub async fn changes(
        &self,
        actor: &Actor,
        after: SyncCursor,
        limit: i64,
    ) -> Result<Vec<ChangeRow>, StorageError> {
        // Flux unifié : upserts (entité entière en JSON moins household_id) + suppressions (tombstones).
        // Keyset `(ts, id) > (curseur)` : pagination stable, ni omission ni duplication.
        let rows = sqlx::query_as::<_, ChangeRow>(
            "select kind, entity_type, id, ts, payload from ( \
                 select 'upsert' as kind, 'category' as entity_type, id, updated_at as ts, \
                        (to_jsonb(c) - 'household_id')::text as payload \
                   from categories c where c.household_id = $1 \
                 union all \
                 select 'upsert', 'payment_method', id, updated_at, (to_jsonb(p) - 'household_id')::text \
                   from payment_methods p where p.household_id = $1 \
                 union all \
                 select 'upsert', 'payer', id, updated_at, (to_jsonb(pa) - 'household_id')::text \
                   from payers pa where pa.household_id = $1 \
                 union all \
                 select 'upsert', 'subscription', id, updated_at, \
                        ((to_jsonb(s) - 'household_id') \
                         || jsonb_build_object('amount', s.amount::text))::text \
                   from subscriptions s where s.household_id = $1 \
                 union all \
                 select 'delete', entity_type, entity_id as id, deleted_at as ts, null::text \
                   from tombstones where household_id = $1 \
             ) changes \
             where (ts, id) > ($2, $3) \
             order by ts asc, id asc \
             limit $4",
        )
        .bind(actor.household_id())
        .bind(after.timestamp)
        .bind(after.id)
        .bind(limit)
        .fetch_all(self.pool)
        .await?;
        Ok(rows)
    }

    /// Version courante `(updated_at, payload JSON)` d'une entité **du foyer de l'appelant** (§9), ou
    /// `None` si absente / d'un type inconnu. Sert à l'arbitrage de conflit (REQ-SYN-005) : détecter un
    /// écrasement fondé sur une version périmée et **capturer** la version qui sera perdue. Le corps
    /// épouse la forme des upserts du delta (moins `household_id`, `amount` en chaîne — R4).
    ///
    /// # Errors
    /// `StorageError::Database` en cas d'échec de requête.
    #[requirement(REQ-SYN-005)]
    pub async fn current(
        &self,
        actor: &Actor,
        entity_type: &str,
        id: Uuid,
    ) -> Result<Option<(DateTime<Utc>, String)>, StorageError> {
        // `entity_type` est un ensemble fermé (jamais une entrée utilisateur libre) : chaque branche
        // vise une table connue.
        let sql = match entity_type {
            "category" => {
                "select updated_at, (to_jsonb(c) - 'household_id')::text \
                 from categories c where c.id = $1 and c.household_id = $2"
            }
            "payment_method" => {
                "select updated_at, (to_jsonb(p) - 'household_id')::text \
                 from payment_methods p where p.id = $1 and p.household_id = $2"
            }
            "payer" => {
                "select updated_at, (to_jsonb(pa) - 'household_id')::text \
                 from payers pa where pa.id = $1 and pa.household_id = $2"
            }
            "subscription" => {
                "select updated_at, \
                        ((to_jsonb(s) - 'household_id') || jsonb_build_object('amount', s.amount::text))::text \
                 from subscriptions s where s.id = $1 and s.household_id = $2"
            }
            _ => return Ok(None),
        };
        let row: Option<(DateTime<Utc>, String)> = sqlx::query_as(sql)
            .bind(id)
            .bind(actor.household_id())
            .fetch_optional(self.pool)
            .await?;
        Ok(row)
    }
}
