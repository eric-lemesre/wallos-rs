//! Repository des moyens de paiement (REQ-SUB-011).
//!
//! Entité **possédée** : toute méthode exige le contexte d'appelant (`&Actor`) et filtre par
//! `household_id` (§9). Une opération sur un moyen de paiement d'un autre foyer se comporte comme s'il
//! n'existait pas (`false`), que l'appelant traduit en `404`. Calque du repository des catégories.

use uuid::Uuid;
use wallos_core::actor::Actor;
use wallos_core::requirement;

use crate::StorageError;

/// Moyen de paiement exposé aux lectures autorisées.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PaymentMethodRow {
    /// Identifiant stable.
    pub id: Uuid,
    /// Nom.
    pub name: String,
}

/// Accès aux moyens de paiement.
pub struct PaymentMethodRepository<'a> {
    pool: &'a sqlx::PgPool,
}

impl<'a> PaymentMethodRepository<'a> {
    /// Construit le repository sur un pool.
    #[must_use]
    #[requirement(REQ-SUB-011)]
    pub const fn new(pool: &'a sqlx::PgPool) -> Self {
        Self { pool }
    }

    /// Crée un moyen de paiement **dans le foyer de l'appelant**.
    ///
    /// # Errors
    /// `StorageError::Database` en cas d'échec d'insertion.
    #[requirement(REQ-SUB-011)]
    pub async fn create(&self, actor: &Actor, id: Uuid, name: &str) -> Result<(), StorageError> {
        sqlx::query("insert into payment_methods (id, household_id, name) values ($1, $2, $3)")
            .bind(id)
            .bind(actor.household_id())
            .bind(name)
            .execute(self.pool)
            .await?;
        Ok(())
    }

    /// Liste les moyens de paiement **du foyer de l'appelant**, dans un ordre déterministe (nom, id).
    ///
    /// # Errors
    /// `StorageError::Database` en cas d'échec de requête.
    #[requirement(REQ-SUB-011)]
    pub async fn list(&self, actor: &Actor) -> Result<Vec<PaymentMethodRow>, StorageError> {
        let rows = sqlx::query_as::<_, PaymentMethodRow>(
            "select id, name from payment_methods where household_id = $1 order by name asc, id asc",
        )
        .bind(actor.household_id())
        .fetch_all(self.pool)
        .await?;
        Ok(rows)
    }

    /// Renomme un moyen de paiement **du foyer de l'appelant**.
    ///
    /// Renvoie `true` si une ligne a été renommée, `false` si elle n'existe pas *ou* appartient à un
    /// autre foyer — l'appelant traduit ce `false` en `404`, jamais `403` (§9).
    ///
    /// # Errors
    /// `StorageError::Database` en cas d'échec de requête.
    #[requirement(REQ-SUB-011)]
    pub async fn rename(&self, actor: &Actor, id: Uuid, name: &str) -> Result<bool, StorageError> {
        let result =
            sqlx::query("update payment_methods set name = $3 where id = $1 and household_id = $2")
                .bind(id)
                .bind(actor.household_id())
                .bind(name)
                .execute(self.pool)
                .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Supprime un moyen de paiement **du foyer de l'appelant**.
    ///
    /// Renvoie `true` si une ligne a été supprimée, `false` sinon (inconnu ou autre foyer → 404).
    ///
    /// # Errors
    /// `StorageError::Database` en cas d'échec de requête.
    #[requirement(REQ-SUB-011)]
    pub async fn delete(&self, actor: &Actor, id: Uuid) -> Result<bool, StorageError> {
        let result = sqlx::query("delete from payment_methods where id = $1 and household_id = $2")
            .bind(id)
            .bind(actor.household_id())
            .execute(self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}
