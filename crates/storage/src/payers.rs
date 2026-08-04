//! Repository des payeurs (REQ-SUB-017).
//!
//! Entité **possédée** : toute méthode exige le contexte d'appelant (`&Actor`) et filtre par
//! `household_id` (garde-fou d'isolation, §9). Une opération sur un payeur d'un autre foyer se comporte
//! comme s'il n'existait pas. Pas d'unicité de nom (parité Wallos). La **suppression d'un payeur
//! référencé est refusée** (oracle REQ-SUB-017-payer.json, jamais cascade).

use chrono::{DateTime, Utc};
use uuid::Uuid;
use wallos_core::actor::Actor;
use wallos_core::requirement;

use crate::StorageError;
use crate::outcomes::CreateOutcome;

/// Payeur exposé aux lectures autorisées.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PayerRow {
    /// Identifiant stable (UUID) — peut être généré côté client (REQ-SYN-001).
    pub id: Uuid,
    /// Nom.
    pub name: String,
    /// Horodatage de dernière modification, **fourni par le serveur** (REQ-SYN-001).
    pub updated_at: DateTime<Utc>,
}

/// Résultat d'une suppression de payeur (REQ-SUB-017).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteOutcome {
    /// Le payeur a été supprimé.
    Deleted,
    /// Aucun payeur correspondant dans le foyer (inconnu ou autre foyer → 404).
    NotFound,
    /// Le payeur est **référencé** par au moins un abonnement du foyer : suppression refusée (→ 409).
    /// Comportement capturé sur Wallos 5.4.2 (`household_in_use`), gelé en fixture (§8.1).
    InUse,
}

/// Accès aux payeurs.
pub struct PayerRepository<'a> {
    pool: &'a sqlx::PgPool,
}

impl<'a> PayerRepository<'a> {
    /// Construit le repository sur un pool.
    #[must_use]
    #[requirement(REQ-SUB-017)]
    pub const fn new(pool: &'a sqlx::PgPool) -> Self {
        Self { pool }
    }

    /// Crée un payeur **dans le foyer de l'appelant**.
    ///
    /// Aucune unicité de nom (parité Wallos) : la seule collision possible est l'`id` fourni déjà pris
    /// (clé primaire) → [`CreateOutcome::DuplicateId`] (→ `409`).
    ///
    /// # Errors
    /// `StorageError::Database` en cas d'échec d'insertion (hors collision de clé primaire).
    #[requirement(REQ-SUB-017)]
    pub async fn create(
        &self,
        actor: &Actor,
        id: Uuid,
        name: &str,
    ) -> Result<CreateOutcome, StorageError> {
        let inserted =
            sqlx::query("insert into payers (id, household_id, name) values ($1, $2, $3)")
                .bind(id)
                .bind(actor.household_id())
                .bind(name)
                .execute(self.pool)
                .await;
        match inserted {
            Ok(_) => Ok(CreateOutcome::Created),
            Err(sqlx::Error::Database(db)) if db.is_unique_violation() => {
                Ok(CreateOutcome::DuplicateId)
            }
            Err(other) => Err(other.into()),
        }
    }

    /// Liste les payeurs **du foyer de l'appelant**, dans un ordre déterministe (nom, puis id).
    ///
    /// # Errors
    /// `StorageError::Database` en cas d'échec de requête.
    #[requirement(REQ-SUB-017)]
    pub async fn list(&self, actor: &Actor) -> Result<Vec<PayerRow>, StorageError> {
        let rows = sqlx::query_as::<_, PayerRow>(
            "select id, name, updated_at from payers \
             where household_id = $1 order by name asc, id asc",
        )
        .bind(actor.household_id())
        .fetch_all(self.pool)
        .await?;
        Ok(rows)
    }

    /// Renomme un payeur **du foyer de l'appelant**. Renvoie `true` si un payeur a été renommé, `false`
    /// s'il n'existe pas ou appartient à un autre foyer (→ 404, jamais 403, §9).
    ///
    /// # Errors
    /// `StorageError::Database` en cas d'échec de requête.
    #[requirement(REQ-SUB-017)]
    pub async fn rename(&self, actor: &Actor, id: Uuid, name: &str) -> Result<bool, StorageError> {
        let result = sqlx::query(
            "update payers set name = $3, updated_at = now() where id = $1 and household_id = $2",
        )
        .bind(id)
        .bind(actor.household_id())
        .bind(name)
        .execute(self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Supprime un payeur **du foyer de l'appelant**, sauf s'il est référencé.
    ///
    /// **Comportement de référence capturé sur Wallos 5.4.2** (§8.1, `handleDeleteMember`) : un payeur
    /// référencé par au moins un abonnement du foyer **ne peut pas être supprimé**
    /// ([`DeleteOutcome::InUse`] → 409, `household_in_use`) — ni réaffectation, ni cascade. Comptage et
    /// suppression sont exécutés dans une **transaction**.
    ///
    /// Isolation (§9) : un payeur inconnu ou d'un autre foyer se comporte comme inexistant
    /// ([`DeleteOutcome::NotFound`] → 404), jamais 403.
    ///
    /// # Errors
    /// `StorageError::Database` en cas d'échec de requête.
    #[requirement(REQ-SUB-017)]
    pub async fn delete(&self, actor: &Actor, id: Uuid) -> Result<DeleteOutcome, StorageError> {
        let mut tx = self.pool.begin().await?;
        let referencing: i64 = sqlx::query_scalar(
            "select count(*) from subscriptions where household_id = $1 and payer_id = $2",
        )
        .bind(actor.household_id())
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;
        if referencing > 0 {
            tx.rollback().await?;
            return Ok(DeleteOutcome::InUse);
        }
        let result = sqlx::query("delete from payers where id = $1 and household_id = $2")
            .bind(id)
            .bind(actor.household_id())
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(if result.rows_affected() > 0 {
            DeleteOutcome::Deleted
        } else {
            DeleteOutcome::NotFound
        })
    }
}
