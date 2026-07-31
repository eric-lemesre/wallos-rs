//! Tests d'intégration du repository des comptes (REQ-AUT-001).
//!
//! `#[sqlx::test]` provisionne une base PostgreSQL éphémère par test et applique les migrations
//! de `migrations/`. Nécessite `DATABASE_URL` (fourni en CI par le service PostgreSQL).

use sqlx::PgPool;
use uuid::Uuid;
use wallos_core::actor::Actor;
use wallos_core::language::Language;
use wallos_core::{DEFAULT_CATEGORY_COUNT, default_category_names, verifies};
use wallos_storage::{Db, UserRepository};

const HASH: &str = "$argon2id$v=19$m=19456,t=2,p=1$fakefakefake$deadbeef";

#[sqlx::test]
#[verifies(REQ-AUT-001)]
async fn create_account_persists_user_and_household(pool: PgPool) {
    let repo = UserRepository::new(&pool);
    let created = repo
        .create_account("alice@example.com", HASH, None)
        .await
        .unwrap()
        .expect("nominal creation returns Some");

    // Le compte est retrouvable au sein de son propre foyer.
    let actor = Actor::new(created.user_id, created.household_id);
    let found = repo
        .find_in_household(&actor, created.user_id)
        .await
        .unwrap()
        .expect("owner can read its account");
    assert_eq!(found.id, created.user_id);
    assert_eq!(found.household_id, created.household_id);
    assert_eq!(found.email, "alice@example.com");
}

/// Noms des catégories d'un foyer, tels que stockés.
async fn category_names(pool: &PgPool, household_id: Uuid) -> Vec<String> {
    sqlx::query_scalar("select name from categories where household_id = $1 order by name asc")
        .bind(household_id)
        .fetch_all(pool)
        .await
        .unwrap()
}

#[sqlx::test]
#[verifies(REQ-CAT-002, case = "compte créé avec une langue → jeu par défaut traduit + langue persistée")]
async fn account_creation_seeds_translated_default_categories(pool: PgPool) {
    let repo = UserRepository::new(&pool);
    let created = repo
        .create_account("fr@example.com", HASH, Some(Language::French))
        .await
        .unwrap()
        .expect("nominal creation returns Some");

    // Le foyer possède exactement le jeu par défaut, dans la langue du compte (français).
    // Comparaison en ensembles : l'ordre de restitution dépend de la collation PostgreSQL (les
    // accents ne se trient pas comme le `sort` de Rust), ce qui est hors périmètre ici.
    let names: std::collections::BTreeSet<String> = category_names(&pool, created.household_id)
        .await
        .into_iter()
        .collect();
    assert_eq!(names.len(), DEFAULT_CATEGORY_COUNT);
    let expected: std::collections::BTreeSet<String> = default_category_names(Language::French)
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    assert_eq!(names, expected, "noms français attendus");
    // La sentinelle « No category » n'est PAS semée (modèle NULL de subtrack).
    assert!(
        !names
            .iter()
            .any(|n| n == "No category" || n == "Aucune catégorie")
    );

    // La langue est persistée sur l'utilisateur (REQ-I18N-001).
    let lang: Option<String> = sqlx::query_scalar("select language from users where id = $1")
        .bind(created.user_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(lang.as_deref(), Some("fr"));
}

#[sqlx::test]
#[verifies(REQ-CAT-002, case = "langue absente → jeu par défaut anglais, langue NULL")]
async fn account_creation_without_language_seeds_english_defaults(pool: PgPool) {
    let repo = UserRepository::new(&pool);
    let created = repo
        .create_account("nolang@example.com", HASH, None)
        .await
        .unwrap()
        .unwrap();

    let names = category_names(&pool, created.household_id).await;
    assert_eq!(names.len(), DEFAULT_CATEGORY_COUNT);
    assert!(names.iter().any(|n| n == "Music"), "noms anglais attendus");
    assert!(!names.iter().any(|n| n == "Musique"));

    // Langue non renseignée → colonne NULL (repli langue système côté UI).
    let lang: Option<String> = sqlx::query_scalar("select language from users where id = $1")
        .bind(created.user_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(lang, None);
}

#[sqlx::test]
#[verifies(REQ-CAT-002, case = "e-mail déjà pris → aucune catégorie semée (anti-énumération)")]
async fn duplicate_email_seeds_no_categories(pool: PgPool) {
    let repo = UserRepository::new(&pool);
    repo.create_account("dup-seed@example.com", HASH, Some(Language::English))
        .await
        .unwrap()
        .unwrap();
    // Deuxième inscription même e-mail : rien créé, donc pas de catégories supplémentaires.
    let second = repo
        .create_account("dup-seed@example.com", HASH, Some(Language::French))
        .await
        .unwrap();
    assert!(second.is_none());
    let total: i64 = sqlx::query_scalar("select count(*) from categories")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        total, DEFAULT_CATEGORY_COUNT as i64,
        "seul le premier compte a semé son jeu par défaut"
    );
}

#[sqlx::test]
#[verifies(REQ-AUT-001)]
async fn duplicate_email_does_not_leak_and_creates_nothing(pool: PgPool) {
    let repo = UserRepository::new(&pool);
    repo.create_account("bob@example.com", HASH, None)
        .await
        .unwrap()
        .expect("first creation succeeds");

    // Deuxième inscription avec le même e-mail : anti-énumération → Ok(None), rien créé.
    let second = repo
        .create_account("bob@example.com", HASH, None)
        .await
        .unwrap();
    assert!(second.is_none(), "duplicate email must return None");

    let user_count: i64 = sqlx::query_scalar("select count(*) from users")
        .fetch_one(&pool)
        .await
        .unwrap();
    let household_count: i64 = sqlx::query_scalar("select count(*) from households")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(user_count, 1, "no extra user created");
    assert_eq!(household_count, 1, "no orphan household created");
}

#[sqlx::test]
#[verifies(REQ-AUT-001)]
async fn email_is_case_insensitive(pool: PgPool) {
    let repo = UserRepository::new(&pool);
    repo.create_account("Carol@Example.com", HASH, None)
        .await
        .unwrap()
        .expect("first creation succeeds");
    // citext : la casse ne crée pas un doublon.
    let dup = repo
        .create_account("carol@example.com", HASH, None)
        .await
        .unwrap();
    assert!(dup.is_none());
}

#[sqlx::test]
#[verifies(REQ-AUT-001)]
async fn read_is_isolated_by_household(pool: PgPool) {
    let repo = UserRepository::new(&pool);
    let created = repo
        .create_account("dave@example.com", HASH, None)
        .await
        .unwrap()
        .unwrap();

    // Un acteur d'un autre foyer ne voit pas le compte (isolation → None → 404).
    let intruder = Actor::new(Uuid::new_v4(), Uuid::new_v4());
    let hidden = repo
        .find_in_household(&intruder, created.user_id)
        .await
        .unwrap();
    assert!(hidden.is_none(), "cross-household read must be invisible");
}

#[sqlx::test]
#[verifies(REQ-AUT-001)]
async fn non_unique_database_error_is_propagated(pool: PgPool) {
    // Un e-mail hors bornes viole la contrainte CHECK (erreur DB non-unique) : elle doit remonter
    // en Err, jamais être confondue avec l'anti-énumération (Ok(None)).
    let repo = UserRepository::new(&pool);
    let overlong = format!("{}@example.com", "a".repeat(300));
    let result = repo.create_account(&overlong, HASH, None).await;
    assert!(
        matches!(result, Err(wallos_storage::StorageError::Database(_))),
        "a non-unique constraint violation must surface as an error, got {result:?}"
    );
}

#[sqlx::test]
#[verifies(REQ-AUT-001)]
async fn migrate_is_idempotent(pool: PgPool) {
    // Les migrations ont déjà été appliquées par sqlx::test ; les rejouer reste Ok (no-op).
    let db = Db::from_pool(pool);
    db.migrate().await.unwrap();
    assert!(!db.pool().is_closed());
}

#[tokio::test]
#[verifies(REQ-AUT-001)]
async fn connect_opens_a_usable_pool() {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL set for integration tests");
    let db = Db::connect(&url).await.unwrap();
    let one: i32 = sqlx::query_scalar("select 1")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(one, 1);
}
