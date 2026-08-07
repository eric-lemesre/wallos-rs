//! Repository des canaux de notification (REQ-NOT-005).
//!
//! Entité **possédée** : toute méthode exige le contexte d'appelant (`&Actor`) et filtre par
//! `household_id` (garde-fou d'isolation, §9). Une opération sur un canal d'un autre foyer se comporte
//! comme s'il n'existait pas (→ 404, jamais 403). Le type est porté par `kind`, la configuration propre
//! au type par `config` (jsonb ; webhook = `{ "url": … }`). Le balayage du cron
//! ([`list_enabled_by_household`]) ne renvoie que les canaux **actifs**.

use std::collections::BTreeMap;

use uuid::Uuid;
use wallos_core::actor::Actor;
use wallos_core::requirement;

use crate::StorageError;

/// Canal de notification exposé aux lectures autorisées.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct NotificationChannelRow {
    /// Identifiant stable (UUID).
    pub id: Uuid,
    /// Foyer propriétaire (§9).
    pub household_id: Uuid,
    /// Type de canal (`webhook`, puis `email`/`telegram`/…).
    pub kind: String,
    /// Configuration propre au type (jsonb ; webhook = `{ "url": … }`).
    pub config: serde_json::Value,
    /// Actif : un canal désactivé n'émet aucune requête sortante (NOT-004).
    pub enabled: bool,
}

/// Accès aux canaux de notification.
pub struct NotificationChannelRepository<'a> {
    pool: &'a sqlx::PgPool,
}

impl<'a> NotificationChannelRepository<'a> {
    /// Construit le repository sur un pool.
    #[must_use]
    #[requirement(REQ-NOT-005)]
    pub const fn new(pool: &'a sqlx::PgPool) -> Self {
        Self { pool }
    }

    /// Crée un canal **dans le foyer de l'appelant** et renvoie la ligne insérée.
    ///
    /// # Errors
    /// `StorageError::Database` en cas d'échec d'insertion.
    #[requirement(REQ-NOT-005)]
    pub async fn create(
        &self,
        actor: &Actor,
        kind: &str,
        config: &serde_json::Value,
        enabled: bool,
    ) -> Result<NotificationChannelRow, StorageError> {
        let row = sqlx::query_as::<_, NotificationChannelRow>(
            "insert into notification_channels (household_id, kind, config, enabled) \
             values ($1, $2, $3, $4) \
             returning id, household_id, kind, config, enabled",
        )
        .bind(actor.household_id())
        .bind(kind)
        .bind(config)
        .bind(enabled)
        .fetch_one(self.pool)
        .await?;
        Ok(row)
    }

    /// Liste les canaux **du foyer de l'appelant**, ordre déterministe (création puis id).
    ///
    /// # Errors
    /// `StorageError::Database` en cas d'échec de requête.
    #[requirement(REQ-NOT-005)]
    pub async fn list(&self, actor: &Actor) -> Result<Vec<NotificationChannelRow>, StorageError> {
        let rows = sqlx::query_as::<_, NotificationChannelRow>(
            "select id, household_id, kind, config, enabled from notification_channels \
             where household_id = $1 order by created_at asc, id asc",
        )
        .bind(actor.household_id())
        .fetch_all(self.pool)
        .await?;
        Ok(rows)
    }

    /// Lit un canal **du foyer de l'appelant**. `None` s'il n'existe pas ou appartient à un autre
    /// foyer (→ 404, jamais 403, §9). Sert à l'envoi de test (REQ-NOT-006).
    ///
    /// # Errors
    /// `StorageError::Database` en cas d'échec de requête.
    #[requirement(REQ-NOT-006)]
    pub async fn get(
        &self,
        actor: &Actor,
        id: Uuid,
    ) -> Result<Option<NotificationChannelRow>, StorageError> {
        let row = sqlx::query_as::<_, NotificationChannelRow>(
            "select id, household_id, kind, config, enabled from notification_channels \
             where id = $1 and household_id = $2",
        )
        .bind(id)
        .bind(actor.household_id())
        .fetch_optional(self.pool)
        .await?;
        Ok(row)
    }

    /// Contact du titulaire d'**un** foyer : `(email, langue)` de l'utilisateur le plus ancien
    /// (même règle que [`Self::owner_contacts`], pour un envoi ciblé — test de canal, REQ-NOT-006).
    ///
    /// # Errors
    /// `StorageError::Database` en cas d'échec de requête.
    #[requirement(REQ-NOT-006)]
    pub async fn owner_contact(
        &self,
        household_id: Uuid,
    ) -> Result<Option<(String, String)>, StorageError> {
        let row: Option<(String, String)> = sqlx::query_as(
            "select email::text, coalesce(language, 'en') from users \
             where household_id = $1 order by created_at asc, id asc limit 1",
        )
        .bind(household_id)
        .fetch_optional(self.pool)
        .await?;
        Ok(row)
    }

    /// Supprime un canal **du foyer de l'appelant**. Renvoie `true` si supprimé, `false` s'il n'existe
    /// pas ou appartient à un autre foyer (→ 404, jamais 403, §9).
    ///
    /// # Errors
    /// `StorageError::Database` en cas d'échec de requête.
    #[requirement(REQ-NOT-005)]
    pub async fn delete(&self, actor: &Actor, id: Uuid) -> Result<bool, StorageError> {
        let result =
            sqlx::query("delete from notification_channels where id = $1 and household_id = $2")
                .bind(id)
                .bind(actor.household_id())
                .execute(self.pool)
                .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Contact du **titulaire** de chaque foyer pour le canal e-mail (REQ-NOT-003) : `(email, langue)`
    /// de l'utilisateur le plus ancien du foyer (langue par défaut `en` si non définie, REQ-I18N-004).
    /// Sert au cron pour adresser le rappel au bon destinataire dans la bonne langue.
    ///
    /// # Errors
    /// `StorageError::Database` en cas d'échec de requête.
    #[requirement(REQ-NOT-003)]
    pub async fn owner_contacts(&self) -> Result<BTreeMap<Uuid, (String, String)>, StorageError> {
        let rows: Vec<(Uuid, String, String)> = sqlx::query_as(
            "select distinct on (household_id) household_id, email::text, coalesce(language, 'en') \
             from users order by household_id, created_at asc, id asc",
        )
        .fetch_all(self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|(household_id, email, language)| (household_id, (email, language)))
            .collect())
    }

    /// Balaye **tous les foyers** : canaux **actifs** regroupés par foyer (pour le cron d'émission).
    /// Un canal désactivé est exclu (NOT-004 : aucune requête sortante).
    ///
    /// # Errors
    /// `StorageError::Database` en cas d'échec de requête.
    #[requirement(REQ-NOT-005)]
    pub async fn list_enabled_by_household(
        &self,
    ) -> Result<BTreeMap<Uuid, Vec<NotificationChannelRow>>, StorageError> {
        let rows = sqlx::query_as::<_, NotificationChannelRow>(
            "select id, household_id, kind, config, enabled from notification_channels \
             where enabled = true order by household_id, created_at asc, id asc",
        )
        .fetch_all(self.pool)
        .await?;
        let mut by_household: BTreeMap<Uuid, Vec<NotificationChannelRow>> = BTreeMap::new();
        for row in rows {
            by_household.entry(row.household_id).or_default().push(row);
        }
        Ok(by_household)
    }
}
