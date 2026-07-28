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
    /// Date de fin (annulation programmée), le cas échéant (REQ-SUB-009).
    pub end_date: Option<NaiveDate>,
}

/// Filtres de liste (REQ-SUB-006), tous optionnels et **conjonctifs** : un critère absent (`None`)
/// n'exclut rien ; deux critères présents sont combinés par `ET`. Aucun filtre → tous les abonnements
/// du foyer.
#[derive(Debug, Clone, Default)]
pub struct SubscriptionFilter {
    /// Restreint à une catégorie (UUID) le cas échéant.
    pub category: Option<Uuid>,
    /// Restreint à un payeur (UUID) le cas échéant.
    pub payer: Option<Uuid>,
    /// Restreint à l'état actif/inactif le cas échéant.
    pub active: Option<bool>,
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
              category_id, payment_method_id, payer_id, logo, url, notes, active, end_date) \
             values ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16)",
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
        .bind(sub.end_date())
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
                    category_id, payment_method_id, payer_id, logo, url, notes, active, end_date \
             from subscriptions where id = $1 and household_id = $2",
        )
        .bind(id)
        .bind(actor.household_id())
        .fetch_optional(self.pool)
        .await?;
        Ok(row)
    }

    /// Met à jour un abonnement **du foyer de l'appelant** (REQ-SUB-004).
    ///
    /// Renvoie `true` si une ligne a été modifiée, `false` si l'abonnement est inconnu **ou** hors du
    /// foyer (l'appelant traduit `false` en `404`, jamais `403` — §9). Tous les champs éditables sont
    /// réécrits ; l'`id` et le `household_id` sont la clé de portée, jamais modifiés.
    ///
    /// # Errors
    /// `StorageError::Database` en cas d'échec de mise à jour.
    #[requirement(REQ-SUB-004)]
    pub async fn update(&self, actor: &Actor, sub: &Subscription) -> Result<bool, StorageError> {
        let res = sqlx::query(
            "update subscriptions set \
                name = $3, amount = $4, currency = $5, cycle_unit = $6, cycle_interval = $7, \
                first_payment = $8, category_id = $9, payment_method_id = $10, payer_id = $11, \
                logo = $12, url = $13, notes = $14, active = $15, end_date = $16 \
             where id = $1 and household_id = $2",
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
        .bind(sub.end_date())
        .execute(self.pool)
        .await?;
        Ok(res.rows_affected() > 0)
    }

    /// Liste les abonnements **du foyer de l'appelant**, filtrés de façon **conjonctive** (REQ-SUB-006).
    ///
    /// Chaque filtre est un garde `paramètre IS NULL OR colonne = paramètre` : `None` n'exclut rien,
    /// les critères présents se combinent par `ET`. Ordre déterministe (nom, puis id) pour une vue
    /// stable. La portée par `household_id` garantit qu'aucun abonnement d'un autre foyer n'apparaît (§9).
    ///
    /// # Errors
    /// `StorageError::Database` en cas d'échec de requête.
    #[requirement(REQ-SUB-006)]
    pub async fn list(
        &self,
        actor: &Actor,
        filter: &SubscriptionFilter,
    ) -> Result<Vec<SubscriptionRow>, StorageError> {
        let rows = sqlx::query_as::<_, SubscriptionRow>(
            "select id, name, amount, currency, cycle_unit, cycle_interval, first_payment, \
                    category_id, payment_method_id, payer_id, logo, url, notes, active, end_date \
             from subscriptions \
             where household_id = $1 \
               and ($2::uuid is null or category_id = $2) \
               and ($3::uuid is null or payer_id = $3) \
               and ($4::bool is null or active = $4) \
             order by name asc, id asc",
        )
        .bind(actor.household_id())
        .bind(filter.category)
        .bind(filter.payer)
        .bind(filter.active)
        .fetch_all(self.pool)
        .await?;
        Ok(rows)
    }
}
