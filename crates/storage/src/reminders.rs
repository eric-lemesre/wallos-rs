//! Repository des rappels avant échéance (REQ-NOT-001).
//!
//! Réglage du **délai de rappel** par foyer (colonne `households.reminder_lead_days`, §9), balayage des
//! abonnements candidats (pour le cron : tous les foyers ; pour la vue : un foyer), et **journal des
//! rappels émis** (`reminder_log`, unicité `(foyer, abonnement, date d'échéance)`) pour ne pas ré-émettre
//! lors d'une ré-exécution du cron le même jour (préparation de REQ-NOT-002).

use chrono::NaiveDate;
use uuid::Uuid;
use wallos_core::actor::Actor;
use wallos_core::requirement;

use crate::StorageError;

/// Ligne de balayage : de quoi calculer la prochaine échéance d'un abonnement **actif** et son délai de
/// rappel effectif (le défaut du foyer).
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ReminderScanRow {
    /// Foyer propriétaire.
    pub household_id: Uuid,
    /// Abonnement.
    pub id: Uuid,
    /// Nom (pour composer le message groupé).
    pub name: String,
    /// Payeur rattaché, le cas échéant (regroupement par payeur, oracle Wallos).
    pub payer_id: Option<Uuid>,
    /// Unité de cycle.
    pub cycle_unit: String,
    /// Intervalle de cycle.
    pub cycle_interval: i32,
    /// Date de premier paiement (ancre de récurrence).
    pub first_payment: NaiveDate,
    /// Date de fin programmée, le cas échéant (REQ-SUB-009).
    pub end_date: Option<NaiveDate>,
    /// Délai de rappel du foyer (jours avant l'échéance).
    pub reminder_lead_days: i32,
}

/// Accès aux rappels.
pub struct ReminderRepository<'a> {
    pool: &'a sqlx::PgPool,
}

impl<'a> ReminderRepository<'a> {
    /// Construit le repository sur un pool.
    #[must_use]
    #[requirement(REQ-NOT-001)]
    pub const fn new(pool: &'a sqlx::PgPool) -> Self {
        Self { pool }
    }

    /// Délai de rappel du foyer de l'appelant (jours). Défaut 1 (parité Wallos).
    ///
    /// # Errors
    /// `StorageError::Database` en cas d'échec de requête.
    #[requirement(REQ-NOT-001)]
    pub async fn lead_days(&self, actor: &Actor) -> Result<i32, StorageError> {
        let (days,): (i32,) =
            sqlx::query_as("select reminder_lead_days from households where id = $1")
                .bind(actor.household_id())
                .fetch_one(self.pool)
                .await?;
        Ok(days)
    }

    /// Fixe le délai de rappel du foyer de l'appelant.
    ///
    /// # Errors
    /// `StorageError::Database` en cas d'échec (la borne 0..=365 est vérifiée en base).
    #[requirement(REQ-NOT-001)]
    pub async fn set_lead_days(&self, actor: &Actor, days: i32) -> Result<(), StorageError> {
        sqlx::query("update households set reminder_lead_days = $2 where id = $1")
            .bind(actor.household_id())
            .bind(days)
            .execute(self.pool)
            .await?;
        Ok(())
    }

    /// Balaye **tous les foyers** : abonnements actifs + délai de rappel du foyer (pour le cron).
    ///
    /// # Errors
    /// `StorageError::Database` en cas d'échec de requête.
    #[requirement(REQ-NOT-001)]
    pub async fn scan_all(&self) -> Result<Vec<ReminderScanRow>, StorageError> {
        let rows = sqlx::query_as::<_, ReminderScanRow>(
            "select s.household_id, s.id, s.name, s.payer_id, s.cycle_unit, s.cycle_interval, \
                    s.first_payment, s.end_date, h.reminder_lead_days \
             from subscriptions s join households h on h.id = s.household_id \
             where s.active = true \
             order by s.household_id, s.id",
        )
        .fetch_all(self.pool)
        .await?;
        Ok(rows)
    }

    /// Balaye le **foyer de l'appelant** uniquement (pour la vue des rappels à venir, §9).
    ///
    /// # Errors
    /// `StorageError::Database` en cas d'échec de requête.
    #[requirement(REQ-NOT-001)]
    pub async fn scan_household(
        &self,
        actor: &Actor,
    ) -> Result<Vec<ReminderScanRow>, StorageError> {
        let rows = sqlx::query_as::<_, ReminderScanRow>(
            "select s.household_id, s.id, s.name, s.payer_id, s.cycle_unit, s.cycle_interval, \
                    s.first_payment, s.end_date, h.reminder_lead_days \
             from subscriptions s join households h on h.id = s.household_id \
             where s.active = true and s.household_id = $1 \
             order by s.id",
        )
        .bind(actor.household_id())
        .fetch_all(self.pool)
        .await?;
        Ok(rows)
    }

    /// Enregistre l'émission d'un rappel. Renvoie `true` si une **nouvelle** ligne a été insérée, `false`
    /// si le rappel avait déjà été émis pour ce `(foyer, abonnement, échéance)` — l'appelant n'émet alors
    /// pas de doublon (préparation REQ-NOT-002).
    ///
    /// # Errors
    /// `StorageError::Database` en cas d'échec d'insertion.
    #[requirement(REQ-NOT-001)]
    pub async fn record_emitted(
        &self,
        household_id: Uuid,
        subscription_id: Uuid,
        due_date: NaiveDate,
    ) -> Result<bool, StorageError> {
        let result = sqlx::query(
            "insert into reminder_log (household_id, subscription_id, due_date) values ($1, $2, $3) \
             on conflict (household_id, subscription_id, due_date) do nothing",
        )
        .bind(household_id)
        .bind(subscription_id)
        .bind(due_date)
        .execute(self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }
}
