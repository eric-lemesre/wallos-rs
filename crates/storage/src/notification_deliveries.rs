//! Repository des livraisons de notification en échec (REQ-NOT-007).
//!
//! Seuls les **échecs** sont suivis : le succès immédiat (cas nominal) ne laisse aucune trace, un
//! réessai réussi **supprime** la ligne, la borne de tentatives atteinte la passe en `abandoned`
//! (visible par l'utilisateur — critère #2). Le réessai est **réclamé** par un update conditionnel
//! (le gagnant repousse `next_attempt_at`) : deux instances concurrentes ne rejouent jamais la même
//! livraison (esprit REQ-NOT-002, ADR 0048).

use chrono::{DateTime, NaiveDate, Utc};
use uuid::Uuid;
use wallos_core::actor::Actor;
use wallos_core::requirement;

use crate::StorageError;

/// Une livraison en difficulté (`pending`) ou abandonnée (`abandoned`).
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct NotificationDeliveryRow {
    /// Identifiant stable (UUID).
    pub id: Uuid,
    /// Foyer propriétaire (§9).
    pub household_id: Uuid,
    /// Canal concerné.
    pub channel_id: Uuid,
    /// Date de référence du lot raté.
    pub as_of: NaiveDate,
    /// Charge utile du lot (rejouée à l'identique au réessai).
    pub payload: serde_json::Value,
    /// Tentatives déjà effectuées (initiale comprise).
    pub attempts: i32,
    /// Prochaine tentative (`pending` uniquement).
    pub next_attempt_at: Option<DateTime<Utc>>,
    /// `pending` ou `abandoned`.
    pub status: String,
    /// Code de diagnostic redacté du dernier échec.
    pub last_code: String,
    /// Type du canal concerné (jointure, pour l'affichage).
    pub channel_kind: String,
}

/// Livraison due au réessai, jointe au type de canal (reconstruction du [`Channel`] par l'appelant).
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct DueDeliveryRow {
    /// Identifiant de la livraison.
    pub id: Uuid,
    /// Foyer propriétaire.
    pub household_id: Uuid,
    /// Tentatives effectuées **avant** ce réessai.
    pub attempts: i32,
    /// Charge utile à rejouer.
    pub payload: serde_json::Value,
    /// Type du canal (`webhook`, `email`, …).
    pub kind: String,
    /// Configuration du canal.
    pub config: serde_json::Value,
    /// Ligne de canal reconstituée à la volée (id du canal).
    pub channel_id: Uuid,
}

/// Accès aux livraisons en échec.
pub struct NotificationDeliveryRepository<'a> {
    pool: &'a sqlx::PgPool,
}

impl<'a> NotificationDeliveryRepository<'a> {
    /// Construit le repository sur un pool.
    #[must_use]
    #[requirement(REQ-NOT-007)]
    pub const fn new(pool: &'a sqlx::PgPool) -> Self {
        Self { pool }
    }

    /// Ouvre le suivi d'une livraison **avant** l'envoi initial (pattern *outbox*, revue NOT-002
    /// F1/F2) : si le processus meurt entre la journalisation de l'occurrence et l'envoi, la ligne
    /// `pending` subsiste et la phase de réessai finit par transmettre le lot — une perte
    /// **silencieuse** devient impossible. Le succès immédiat [`Self::resolve`] la ligne dans la
    /// foulée (aucun bruit au nominal). Si un suivi existe déjà pour ce (canal, lot du jour), la
    /// charge utile est rafraîchie sans toucher au rythme de réessai en cours.
    ///
    /// # Errors
    /// `StorageError::Database` en cas d'échec d'insertion.
    #[requirement(REQ-NOT-007)]
    #[requirement(REQ-NOT-002)]
    pub async fn open(
        &self,
        household_id: Uuid,
        channel_id: Uuid,
        as_of: NaiveDate,
        payload: &serde_json::Value,
        next_attempt_at: DateTime<Utc>,
    ) -> Result<Uuid, StorageError> {
        let (id,): (Uuid,) = sqlx::query_as(
            "insert into notification_deliveries \
                 (household_id, channel_id, as_of, payload, last_code, next_attempt_at) \
             values ($1, $2, $3, $4, 'in-flight', $5) \
             on conflict (channel_id, as_of) do update \
                 set payload = excluded.payload, updated_at = now() \
             returning id",
        )
        .bind(household_id)
        .bind(channel_id)
        .bind(as_of)
        .bind(payload)
        .bind(next_attempt_at)
        .fetch_one(self.pool)
        .await?;
        Ok(id)
    }

    /// **Réclame** les livraisons dues au réessai (`pending`, échéance atteinte, canal **actif** —
    /// un canal désactivé n'émet aucune requête, REQ-NOT-004) : incrémente `attempts` et repousse
    /// `next_attempt_at` à `provisional_next` en un seul update — une instance concurrente ne peut
    /// pas réclamer la même ligne (REQ-NOT-002). L'appelant tente l'envoi puis conclut par
    /// [`Self::resolve`] (succès) ou [`Self::record_retry_failure`] (échec/abandon).
    ///
    /// # Errors
    /// `StorageError::Database` en cas d'échec de requête.
    #[requirement(REQ-NOT-007)]
    pub async fn claim_due_retries(
        &self,
        now: DateTime<Utc>,
        provisional_next: DateTime<Utc>,
    ) -> Result<Vec<DueDeliveryRow>, StorageError> {
        let rows = sqlx::query_as::<_, DueDeliveryRow>(
            "update notification_deliveries d \
             set attempts = d.attempts + 1, next_attempt_at = $2, updated_at = now() \
             from notification_channels c \
             where c.id = d.channel_id and c.enabled = true and d.id in ( \
                 select id from notification_deliveries \
                 where status = 'pending' and next_attempt_at <= $1 \
                 order by next_attempt_at asc limit 500) \
               and d.status = 'pending' and d.next_attempt_at <= $1 \
             returning d.id, d.household_id, d.attempts, d.payload, \
                       c.kind, c.config, c.id as channel_id",
        )
        .bind(now)
        .bind(provisional_next)
        .fetch_all(self.pool)
        .await?;
        Ok(rows)
    }

    /// Purge les livraisons **abandonnées** dont la dernière activité précède `before` (revue
    /// NOT-007 F3) : l'abandon reste visible un temps borné, la table ne croît pas indéfiniment.
    ///
    /// # Errors
    /// `StorageError::Database` en cas d'échec de requête.
    #[requirement(REQ-NOT-007)]
    pub async fn purge_abandoned_before(&self, before: DateTime<Utc>) -> Result<u64, StorageError> {
        let result = sqlx::query(
            "delete from notification_deliveries where status = 'abandoned' and updated_at < $1",
        )
        .bind(before)
        .execute(self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    /// Clôt une livraison **réussie** au réessai : le suivi disparaît (plus rien à signaler).
    ///
    /// # Errors
    /// `StorageError::Database` en cas d'échec de requête.
    #[requirement(REQ-NOT-007)]
    pub async fn resolve(&self, id: Uuid) -> Result<(), StorageError> {
        sqlx::query("delete from notification_deliveries where id = $1")
            .bind(id)
            .execute(self.pool)
            .await?;
        Ok(())
    }

    /// Journalise l'échec d'un **réessai** : met à jour le diagnostic et l'échéance suivante
    /// (`next_attempt_at`, calculée par la politique pure) ; `None` = borne atteinte, la livraison
    /// passe en `abandoned` (visible, critère #2) et ne sera plus réessayée.
    ///
    /// # Errors
    /// `StorageError::Database` en cas d'échec de requête.
    #[requirement(REQ-NOT-007)]
    pub async fn record_retry_failure(
        &self,
        id: Uuid,
        last_code: &str,
        next_attempt_at: Option<DateTime<Utc>>,
    ) -> Result<(), StorageError> {
        match next_attempt_at {
            Some(next) => {
                sqlx::query(
                    "update notification_deliveries \
                     set last_code = $2, next_attempt_at = $3, updated_at = now() \
                     where id = $1",
                )
                .bind(id)
                .bind(last_code)
                .bind(next)
                .execute(self.pool)
                .await?
            }
            None => {
                sqlx::query(
                    "update notification_deliveries \
                     set status = 'abandoned', next_attempt_at = null, last_code = $2, \
                         updated_at = now() \
                     where id = $1",
                )
                .bind(id)
                .bind(last_code)
                .execute(self.pool)
                .await?
            }
        };
        Ok(())
    }

    /// Liste les livraisons en difficulté **du foyer de l'appelant** (§9), abandons d'abord puis
    /// par échéance de réessai — la matière du critère #2 (« visible dans l'interface »).
    ///
    /// # Errors
    /// `StorageError::Database` en cas d'échec de requête.
    #[requirement(REQ-NOT-007)]
    pub async fn list(&self, actor: &Actor) -> Result<Vec<NotificationDeliveryRow>, StorageError> {
        let rows = sqlx::query_as::<_, NotificationDeliveryRow>(
            "select d.id, d.household_id, d.channel_id, d.as_of, d.payload, d.attempts, \
                    d.next_attempt_at, d.status, d.last_code, c.kind as channel_kind \
             from notification_deliveries d join notification_channels c on c.id = d.channel_id \
             where d.household_id = $1 \
             order by d.status asc, d.as_of desc, d.id asc",
        )
        .bind(actor.household_id())
        .fetch_all(self.pool)
        .await?;
        Ok(rows)
    }
}
