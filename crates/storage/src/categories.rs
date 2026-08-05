//! Repository des catégories (REQ-CAT-001).
//!
//! Entité **possédée** : toute méthode exige le contexte d'appelant (`&Actor`) et filtre par
//! `household_id` (garde-fou d'isolation, ADR 0006/0012, §9). Une opération sur une catégorie d'un
//! autre foyer se comporte comme si elle n'existait pas (`false`), que l'appelant traduit en `404`.

use chrono::{DateTime, Utc};
use uuid::Uuid;
use wallos_core::actor::Actor;
use wallos_core::category_is_deletable;
use wallos_core::requirement;

use crate::StorageError;
use crate::outcomes::CreateOutcome;

/// Catégorie exposée aux lectures autorisées.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CategoryRow {
    /// Identifiant stable (UUID) — peut être généré côté client (REQ-SYN-001).
    pub id: Uuid,
    /// Nom.
    pub name: String,
    /// Horodatage de dernière modification, **fourni par le serveur** (REQ-SYN-001).
    pub updated_at: DateTime<Utc>,
}

/// Résultat d'un renommage de catégorie (REQ-CAT-001 / CAT-004).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenameOutcome {
    /// La catégorie a été renommée.
    Renamed,
    /// Aucune catégorie correspondante dans le foyer (→ 404).
    NotFound,
    /// Le nom entre en collision avec une autre catégorie du foyer (→ 422, unicité CAT-004).
    Duplicate,
}

/// Résultat d'une suppression de catégorie (REQ-CAT-003).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteOutcome {
    /// La catégorie a été supprimée.
    Deleted,
    /// Aucune catégorie correspondante dans le foyer (inconnue ou autre foyer → 404).
    NotFound,
    /// La catégorie est **référencée** par au moins un abonnement du foyer : suppression refusée
    /// (→ 409). Comportement capturé sur Wallos 5.4.2 (`category_in_use`), gelé en fixture (§8.1).
    InUse,
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
    /// Distingue les deux collisions d'unicité possibles (revue SYN-001 F1) : l'`id` fourni déjà pris
    /// (clé primaire) → [`CreateOutcome::DuplicateId`] (→ `409`) ; un nom déjà utilisé dans le foyer,
    /// insensible à la casse (unicité CAT-004) → [`CreateOutcome::DuplicateName`] (→ `422`).
    ///
    /// # Errors
    /// `StorageError::Database` en cas d'échec d'insertion (hors collision d'unicité).
    #[requirement(REQ-CAT-004)]
    pub async fn create(
        &self,
        actor: &Actor,
        id: Uuid,
        name: &str,
    ) -> Result<CreateOutcome, StorageError> {
        let inserted =
            sqlx::query("insert into categories (id, household_id, name) values ($1, $2, $3)")
                .bind(id)
                .bind(actor.household_id())
                .bind(name)
                .execute(self.pool)
                .await;
        match inserted {
            Ok(_) => Ok(CreateOutcome::Created),
            Err(sqlx::Error::Database(db)) if db.is_unique_violation() => {
                // L'index unique de nom porte un nom stable ; toute autre violation est la clé primaire.
                if db.constraint() == Some("categories_household_name_unique") {
                    Ok(CreateOutcome::DuplicateName)
                } else {
                    Ok(CreateOutcome::DuplicateId)
                }
            }
            Err(other) => Err(other.into()),
        }
    }

    /// Liste les catégories **du foyer de l'appelant**, dans un ordre déterministe (nom, puis id).
    ///
    /// # Errors
    /// `StorageError::Database` en cas d'échec de requête.
    #[requirement(REQ-CAT-001)]
    pub async fn list(&self, actor: &Actor) -> Result<Vec<CategoryRow>, StorageError> {
        let rows = sqlx::query_as::<_, CategoryRow>(
            "select id, name, updated_at from categories \
             where household_id = $1 order by name asc, id asc",
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
    #[requirement(REQ-CAT-004)]
    pub async fn rename(
        &self,
        actor: &Actor,
        id: Uuid,
        name: &str,
    ) -> Result<RenameOutcome, StorageError> {
        let result = sqlx::query(
            "update categories set name = $3, updated_at = now() \
                 where id = $1 and household_id = $2",
        )
        .bind(id)
        .bind(actor.household_id())
        .bind(name)
        .execute(self.pool)
        .await;
        match result {
            Ok(r) if r.rows_affected() > 0 => Ok(RenameOutcome::Renamed),
            Ok(_) => Ok(RenameOutcome::NotFound),
            // Renommer vers un nom déjà pris dans le foyer viole l'index unique (CAT-004).
            Err(sqlx::Error::Database(db)) if db.is_unique_violation() => {
                Ok(RenameOutcome::Duplicate)
            }
            Err(other) => Err(other.into()),
        }
    }

    /// Supprime une catégorie **du foyer de l'appelant**, sauf si elle est référencée.
    ///
    /// **Comportement de référence capturé sur Wallos 5.4.2** (§8.1, `handleDeleteCategory`) : une
    /// catégorie référencée par au moins un abonnement du foyer **ne peut pas être supprimée**
    /// ([`DeleteOutcome::InUse`] → 409, `category_in_use`) — ni réaffectation, ni cascade. Le comptage
    /// et la suppression sont exécutés dans une **transaction** pour rester cohérents. La décision de
    /// suppressibilité est déléguée à la politique de domaine [`category_is_deletable`].
    ///
    /// Isolation (§9) : une catégorie inconnue ou d'un autre foyer se comporte comme inexistante
    /// ([`DeleteOutcome::NotFound`] → 404), jamais 403.
    ///
    /// # Errors
    /// `StorageError::Database` en cas d'échec de requête.
    #[requirement(REQ-CAT-003)]
    pub async fn delete(&self, actor: &Actor, id: Uuid) -> Result<DeleteOutcome, StorageError> {
        let mut tx = self.pool.begin().await?;
        // Nombre d'abonnements DU FOYER de l'appelant qui référencent la catégorie (isolation §9 :
        // les abonnements d'un autre foyer ne comptent jamais).
        let referencing: i64 = sqlx::query_scalar(
            "select count(*) from subscriptions where household_id = $1 and category_id = $2",
        )
        .bind(actor.household_id())
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;
        if !category_is_deletable(referencing.max(0).unsigned_abs()) {
            // Référencée : rien n'est modifié (transaction en lecture seule close sans écriture).
            tx.rollback().await?;
            return Ok(DeleteOutcome::InUse);
        }
        let result = sqlx::query("delete from categories where id = $1 and household_id = $2")
            .bind(id)
            .bind(actor.household_id())
            .execute(&mut *tx)
            .await?;
        if result.rows_affected() == 0 {
            tx.rollback().await?;
            return Ok(DeleteOutcome::NotFound);
        }
        // Pierre tombale dans la MÊME transaction (REQ-SYN-002) : jamais de suppression sans trace.
        crate::tombstones::record(
            &mut *tx,
            actor.household_id(),
            crate::tombstones::ENTITY_CATEGORY,
            id,
        )
        .await?;
        tx.commit().await?;
        Ok(DeleteOutcome::Deleted)
    }
}
