//! Repository des comptes et foyers.
//!
//! REQ-AUT-001 — création de compte. Conformément à ADR 0006, les lectures exigent un `&Actor`
//! (contexte d'appelant) : aucune requête ne peut omettre la clause de foyer. Conformément à
//! ADR 0012, chaque compte créé obtient un foyer personnel.

use sqlx::PgPool;
use uuid::Uuid;
use wallos_core::actor::Actor;
use wallos_core::language::Language;
use wallos_core::{default_category_names, requirement};

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

    /// Crée un compte : un foyer personnel, l'utilisateur, puis son jeu de catégories par défaut,
    /// en une **seule transaction**.
    ///
    /// **Anti-énumération** : si l'e-mail est déjà enregistré, renvoie `Ok(None)` **sans rien
    /// créer** (ni foyer, ni catégories), afin que l'appelant produise une réponse identique au cas
    /// nominal (REQ-AUT-001, critère 2). Le `password_hash` est fourni déjà calculé (le hachage
    /// argon2id vit côté serveur).
    ///
    /// `language` est la langue choisie à l'inscription (REQ-I18N-001) : elle est persistée sur
    /// l'utilisateur *et* détermine la langue du jeu de catégories par défaut semé (REQ-CAT-002).
    /// `None` = non renseignée → colonne `language` à `NULL` (repli langue système côté UI) et jeu par
    /// défaut en anglais (langue de base).
    ///
    /// # Errors
    /// `StorageError::Database` pour toute erreur autre qu'une violation d'unicité.
    #[requirement(REQ-AUT-001)]
    pub async fn create_account(
        &self,
        email: &str,
        password_hash: &str,
        language: Option<Language>,
    ) -> Result<Option<CreatedAccount>, StorageError> {
        let household_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        let mut tx = self.pool.begin().await?;
        sqlx::query("insert into households (id) values ($1)")
            .bind(household_id)
            .execute(&mut *tx)
            .await?;

        // REQ-CAT-002 — jeu de catégories par défaut semé dans la MÊME transaction (atomicité :
        // catégories ↔ compte tout-ou-rien). Semé **AVANT** l'insertion `users` — qui, elle, détecte
        // le doublon d'e-mail — pour que le chemin « e-mail déjà pris » exécute **le même travail** puis
        // rollback : les deux chemins ont un coût DB identique, sans canal de timing exploitable pour
        // l'énumération (durcit REQ-AUT-001 critère 2 ; revue 2026-07-31-cat-002 F1).
        seed_default_categories(&mut tx, household_id, language.unwrap_or(Language::English))
            .await?;

        let inserted = sqlx::query(
            "insert into users (id, household_id, email, password_hash, language) \
             values ($1, $2, $3, $4, $5)",
        )
        .bind(user_id)
        .bind(household_id)
        .bind(email)
        .bind(password_hash)
        .bind(language.map(Language::code))
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
            // Doublon d'e-mail : rollback → foyer ET catégories déjà semées sont annulés (anti-énumération :
            // rien n'est créé, réponse identique au cas nominal).
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

    /// Lit l'e-mail et le hash de mot de passe **du compte de l'appelant** (REQ-AUT-007).
    ///
    /// L'e-mail sert de clé de limitation de taux (REQ-AUT-008) au changement de mot de passe ; le
    /// hash sert à vérifier le mot de passe actuel. Filtré sur le foyer de l'`Actor` (isolation).
    ///
    /// # Errors
    /// `StorageError::Database` en cas d'échec de requête.
    #[requirement(REQ-AUT-007)]
    pub async fn find_email_and_password_hash(
        &self,
        actor: &Actor,
    ) -> Result<Option<(String, String)>, StorageError> {
        let row: Option<(String, String)> = sqlx::query_as(
            "select email::text, password_hash from users where id = $1 and household_id = $2",
        )
        .bind(actor.user_id())
        .bind(actor.household_id())
        .fetch_optional(self.pool)
        .await?;
        Ok(row)
    }

    /// Remplace le hash de mot de passe **du compte de l'appelant** (REQ-AUT-007).
    ///
    /// # Errors
    /// `StorageError::Database` en cas d'échec de requête.
    #[requirement(REQ-AUT-007)]
    pub async fn update_password(
        &self,
        actor: &Actor,
        new_password_hash: &str,
    ) -> Result<(), StorageError> {
        sqlx::query("update users set password_hash = $3 where id = $1 and household_id = $2")
            .bind(actor.user_id())
            .bind(actor.household_id())
            .bind(new_password_hash)
            .execute(self.pool)
            .await?;
        Ok(())
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

/// Sème le jeu de catégories par défaut d'un foyer nouvellement créé (REQ-CAT-002), dans la langue
/// fournie. Chaque catégorie reçoit un UUID serveur. **Une seule requête** (`unnest`) : le coût DB est
/// constant et faible, ce qui rend le durcissement anti-timing de l'appelant d'autant plus net (F9).
/// Exécuté dans la transaction de création de compte (l'appelant en garantit l'atomicité) : la moindre
/// erreur propage et annule tout.
///
/// # Errors
/// `StorageError::Database` en cas d'échec d'insertion.
#[requirement(REQ-CAT-002)]
async fn seed_default_categories(
    conn: &mut sqlx::PgConnection,
    household_id: Uuid,
    language: Language,
) -> Result<(), StorageError> {
    let names = default_category_names(language);
    let ids: Vec<Uuid> = names.iter().map(|_| Uuid::new_v4()).collect();
    let names: Vec<&str> = names.to_vec();
    sqlx::query(
        "insert into categories (id, household_id, name) \
         select id, $2, name from unnest($1::uuid[], $3::text[]) as t(id, name)",
    )
    .bind(&ids)
    .bind(household_id)
    .bind(&names)
    .execute(&mut *conn)
    .await?;
    Ok(())
}
