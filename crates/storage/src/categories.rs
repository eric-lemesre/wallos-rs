//! Repository des catégories (REQ-CAT-001).
//!
//! Entité **possédée** : toute méthode exige le contexte d'appelant (`&Actor`) et filtre par
//! `household_id` (garde-fou d'isolation, ADR 0006/0012, §9). Une opération sur une catégorie d'un
//! autre foyer se comporte comme si elle n'existait pas (`false`), que l'appelant traduit en `404`.

use uuid::Uuid;
use wallos_core::actor::Actor;
use wallos_core::requirement;

use crate::StorageError;

/// Catégorie exposée aux lectures autorisées.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CategoryRow {
    /// Identifiant stable.
    pub id: Uuid,
    /// Nom.
    pub name: String,
}

/// Accès aux catégories.
pub struct CategoryRepository<'a> {
    pool: &'a sqlx::PgPool,
}

impl<'a> CategoryRepository<'a> {
    /// Construit le repository sur un pool.
    #[must_use]
    #[requirement(REQ-CAT-001)]
    pub const fn new(pool: &'a sqlx::PgPool) -> Self {
        Self { pool }
    }

    /// Crée une catégorie **dans le foyer de l'appelant**.
    ///
    /// # Errors
    /// `StorageError::Database` en cas d'échec d'insertion.
    #[requirement(REQ-CAT-001)]
    pub async fn create(&self, actor: &Actor, id: Uuid, name: &str) -> Result<(), StorageError> {
        sqlx::query("insert into categories (id, household_id, name) values ($1, $2, $3)")
            .bind(id)
            .bind(actor.household_id())
            .bind(name)
            .execute(self.pool)
            .await?;
        Ok(())
    }

    /// Liste les catégories **du foyer de l'appelant**, dans un ordre déterministe (nom, puis id).
    ///
    /// # Errors
    /// `StorageError::Database` en cas d'échec de requête.
    #[requirement(REQ-CAT-001)]
    pub async fn list(&self, actor: &Actor) -> Result<Vec<CategoryRow>, StorageError> {
        let rows = sqlx::query_as::<_, CategoryRow>(
            "select id, name from categories where household_id = $1 order by name asc, id asc",
        )
        .bind(actor.household_id())
        .fetch_all(self.pool)
        .await?;
        Ok(rows)
    }

    /// Renomme une catégorie **du foyer de l'appelant**.
    ///
    /// Renvoie `true` si une catégorie a été renommée, `false` si elle n'existe pas *ou* appartient à
    /// un autre foyer — l'appelant traduit ce `false` en `404`, jamais `403` (§9).
    ///
    /// # Errors
    /// `StorageError::Database` en cas d'échec de requête.
    #[requirement(REQ-CAT-001)]
    pub async fn rename(&self, actor: &Actor, id: Uuid, name: &str) -> Result<bool, StorageError> {
        let result =
            sqlx::query("update categories set name = $3 where id = $1 and household_id = $2")
                .bind(id)
                .bind(actor.household_id())
                .bind(name)
                .execute(self.pool)
                .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Supprime une catégorie **du foyer de l'appelant**.
    ///
    /// Renvoie `true` si une catégorie a été supprimée, `false` sinon (inconnue ou autre foyer → 404).
    ///
    /// # Errors
    /// `StorageError::Database` en cas d'échec de requête.
    #[requirement(REQ-CAT-001)]
    pub async fn delete(&self, actor: &Actor, id: Uuid) -> Result<bool, StorageError> {
        let result = sqlx::query("delete from categories where id = $1 and household_id = $2")
            .bind(id)
            .bind(actor.household_id())
            .execute(self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}
