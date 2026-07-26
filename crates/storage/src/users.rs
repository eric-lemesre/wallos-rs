//! Repository des comptes et foyers.
//!
//! REQ-AUT-001 — création de compte. Conformément à ADR 0006, les lectures exigent un `&Actor`
//! (contexte d'appelant) : aucune requête ne peut omettre la clause de foyer. Conformément à
//! ADR 0012, chaque compte créé obtient un foyer personnel.

use sqlx::PgPool;
use uuid::Uuid;
use wallos_core::actor::Actor;
use wallos_core::requirement;

use crate::StorageError;

/// Identifiants produits par une création de compte réussie.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CreatedAccount {
    /// Identifiant du compte créé.
    pub user_id: Uuid,
    /// Foyer personnel créé pour ce compte.
    pub household_id: Uuid,
}

/// Utilisateur stocké, exposé aux lectures autorisées.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct StoredUser {
    /// Identifiant du compte.
    pub id: Uuid,
    /// Foyer d'appartenance.
    pub household_id: Uuid,
    /// Adresse e-mail (normalisée par la colonne `citext`).
    pub email: String,
}

/// Identifiants de connexion d'un compte (REQ-AUT-002).
#[derive(Debug, Clone)]
pub struct Credentials {
    /// Contexte d'appelant du compte (utilisateur + foyer).
    pub actor: Actor,
    /// Hash argon2id du mot de passe stocké.
    pub password_hash: String,
}

/// Accès aux comptes.
pub struct UserRepository<'a> {
    pool: &'a PgPool,
}

impl<'a> UserRepository<'a> {
    /// Construit le repository sur un pool.
    #[must_use]
    #[requirement(REQ-AUT-001)]
    pub const fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    /// Crée un compte : un foyer personnel puis l'utilisateur, en une transaction.
    ///
    /// **Anti-énumération** : si l'e-mail est déjà enregistré, renvoie `Ok(None)` **sans rien
    /// créer**, afin que l'appelant produise une réponse identique au cas nominal (REQ-AUT-001,
    /// critère 2). Le `password_hash` est fourni déjà calculé (le hachage argon2id vit côté serveur).
    ///
    /// # Errors
    /// `StorageError::Database` pour toute erreur autre qu'une violation d'unicité.
    #[requirement(REQ-AUT-001)]
    pub async fn create_account(
        &self,
        email: &str,
        password_hash: &str,
    ) -> Result<Option<CreatedAccount>, StorageError> {
        let household_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        let mut tx = self.pool.begin().await?;
        sqlx::query("insert into households (id) values ($1)")
            .bind(household_id)
            .execute(&mut *tx)
            .await?;

        let inserted = sqlx::query(
            "insert into users (id, household_id, email, password_hash) values ($1, $2, $3, $4)",
        )
        .bind(user_id)
        .bind(household_id)
        .bind(email)
        .bind(password_hash)
        .execute(&mut *tx)
        .await;

        match inserted {
            Ok(_) => {
                tx.commit().await?;
                Ok(Some(CreatedAccount {
                    user_id,
                    household_id,
                }))
            }
            Err(sqlx::Error::Database(db)) if db.is_unique_violation() => {
                tx.rollback().await?;
                Ok(None)
            }
            Err(other) => {
                tx.rollback().await?;
                Err(other.into())
            }
        }
    }

    /// Lit un compte **au sein du foyer de l'appelant** (garde-fou d'isolation, ADR 0006/0012).
    ///
    /// Renvoie `None` si le compte n'existe pas *ou* appartient à un autre foyer — l'appelant
    /// traduit ce `None` en `404`, jamais `403` (AGENTS.md §9).
    ///
    /// # Errors
    /// `StorageError::Database` en cas d'erreur de requête.
    #[requirement(REQ-AUT-001)]
    pub async fn find_in_household(
        &self,
        actor: &Actor,
        user_id: Uuid,
    ) -> Result<Option<StoredUser>, StorageError> {
        let user = sqlx::query_as::<_, StoredUser>(
            "select id, household_id, email::text as email \
             from users where id = $1 and household_id = $2",
        )
        .bind(user_id)
        .bind(actor.household_id())
        .fetch_optional(self.pool)
        .await?;
        Ok(user)
    }

    /// Récupère les identifiants d'un compte par e-mail, pour l'authentification (REQ-AUT-002).
    ///
    /// Renvoie `None` si aucun compte ne correspond — l'appelant doit rester **timing-safe**
    /// (vérifier malgré tout un hash factice) pour ne pas divulguer l'existence.
    ///
    /// # Errors
    /// `StorageError::Database` en cas d'échec de requête.
    #[requirement(REQ-AUT-002)]
    pub async fn find_credentials_by_email(
        &self,
        email: &str,
    ) -> Result<Option<Credentials>, StorageError> {
        let row: Option<(Uuid, Uuid, String)> =
            sqlx::query_as("select id, household_id, password_hash from users where email = $1")
                .bind(email)
                .fetch_optional(self.pool)
                .await?;
        Ok(
            row.map(|(user_id, household_id, password_hash)| Credentials {
                actor: Actor::new(user_id, household_id),
                password_hash,
            }),
        )
    }
}
