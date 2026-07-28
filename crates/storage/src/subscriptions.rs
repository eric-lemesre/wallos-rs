//! Repository des abonnements (REQ-SUB-002 / SUB-001).
//!
//! Entité **possédée** : toute méthode exige `&Actor` et filtre par `household_id` (§9). Le montant
//! est un `Decimal` (jamais un flottant, R4). Le cycle est stocké décomposé (unité + intervalle).

use chrono::NaiveDate;
use rust_decimal::Decimal;
use uuid::Uuid;
use wallos_core::Subscription;
use wallos_core::actor::Actor;
use wallos_core::requirement;

use crate::StorageError;

/// Ligne d'abonnement (champs bruts, tels que stockés).
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SubscriptionRow {
    /// Identifiant stable.
    pub id: Uuid,
    /// Nom.
    pub name: String,
    /// Montant (décimal exact).
    pub amount: Decimal,
    /// Code devise ISO 4217.
    pub currency: String,
    /// Unité de cycle (`day`/`week`/`month`/`year`).
    pub cycle_unit: String,
    /// Intervalle du cycle.
    pub cycle_interval: i32,
    /// Date de premier paiement.
    pub first_payment: NaiveDate,
    /// Catégorie rattachée.
    pub category_id: Option<Uuid>,
    /// Moyen de paiement rattaché.
    pub payment_method_id: Option<Uuid>,
    /// Payeur rattaché.
    pub payer_id: Option<Uuid>,
    /// Logo.
    pub logo: Option<String>,
    /// URL.
    pub url: Option<String>,
    /// Notes.
    pub notes: Option<String>,
    /// État actif.
    pub active: bool,
}

/// Accès aux abonnements.
pub struct SubscriptionRepository<'a> {
    pool: &'a sqlx::PgPool,
}

impl<'a> SubscriptionRepository<'a> {
    /// Construit le repository sur un pool.
    #[must_use]
    #[requirement(REQ-SUB-002)]
    pub const fn new(pool: &'a sqlx::PgPool) -> Self {
        Self { pool }
    }

    /// Crée un abonnement **dans le foyer de l'appelant**.
    ///
    /// # Errors
    /// `StorageError::Database` en cas d'échec d'insertion.
    #[requirement(REQ-SUB-002)]
    pub async fn create(&self, actor: &Actor, sub: &Subscription) -> Result<(), StorageError> {
        sqlx::query(
            "insert into subscriptions \
             (id, household_id, name, amount, currency, cycle_unit, cycle_interval, first_payment, \
              category_id, payment_method_id, payer_id, logo, url, notes, active) \
             values ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15)",
        )
        .bind(sub.id())
        .bind(actor.household_id())
        .bind(sub.name())
        .bind(sub.price().amount())
        .bind(sub.price().currency().as_str())
        .bind(sub.cycle().unit().as_str())
        .bind(i32::try_from(sub.cycle().interval()).unwrap_or(i32::MAX))
        .bind(sub.first_payment())
        .bind(sub.category())
        .bind(sub.payment_method())
        .bind(sub.payer())
        .bind(sub.logo())
        .bind(sub.url())
        .bind(sub.notes())
        .bind(sub.is_active())
        .execute(self.pool)
        .await?;
        Ok(())
    }

    /// Lit un abonnement **du foyer de l'appelant** par son id (`None` si absent ou autre foyer → 404).
    ///
    /// # Errors
    /// `StorageError::Database` en cas d'échec de requête.
    #[requirement(REQ-SUB-002)]
    pub async fn get(
        &self,
        actor: &Actor,
        id: Uuid,
    ) -> Result<Option<SubscriptionRow>, StorageError> {
        let row = sqlx::query_as::<_, SubscriptionRow>(
            "select id, name, amount, currency, cycle_unit, cycle_interval, first_payment, \
                    category_id, payment_method_id, payer_id, logo, url, notes, active \
             from subscriptions where id = $1 and household_id = $2",
        )
        .bind(id)
        .bind(actor.household_id())
        .fetch_optional(self.pool)
        .await?;
        Ok(row)
    }
}
