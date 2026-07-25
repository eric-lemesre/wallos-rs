//! Tests d'intégration du repository des comptes (REQ-AUT-001).
//!
//! `#[sqlx::test]` provisionne une base PostgreSQL éphémère par test et applique les migrations
//! de `migrations/`. Nécessite `DATABASE_URL` (fourni en CI par le service PostgreSQL).

use sqlx::PgPool;
use uuid::Uuid;
use wallos_core::actor::Actor;
use wallos_core::verifies;
use wallos_storage::{Db, UserRepository};

const HASH: &str = "$argon2id$v=19$m=19456,t=2,p=1$fakefakefake$deadbeef";

#[sqlx::test]
#[verifies(REQ-AUT-001)]
async fn create_account_persists_user_and_household(pool: PgPool) {
    let repo = UserRepository::new(&pool);
    let created = repo
        .create_account("alice@example.com", HASH)
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

#[sqlx::test]
#[verifies(REQ-AUT-001)]
async fn duplicate_email_does_not_leak_and_creates_nothing(pool: PgPool) {
    let repo = UserRepository::new(&pool);
    repo.create_account("bob@example.com", HASH)
        .await
        .unwrap()
        .expect("first creation succeeds");

    // Deuxième inscription avec le même e-mail : anti-énumération → Ok(None), rien créé.
    let second = repo.create_account("bob@example.com", HASH).await.unwrap();
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
    repo.create_account("Carol@Example.com", HASH)
        .await
        .unwrap()
        .expect("first creation succeeds");
    // citext : la casse ne crée pas un doublon.
    let dup = repo
        .create_account("carol@example.com", HASH)
        .await
        .unwrap();
    assert!(dup.is_none());
}

#[sqlx::test]
#[verifies(REQ-AUT-001)]
async fn read_is_isolated_by_household(pool: PgPool) {
    let repo = UserRepository::new(&pool);
    let created = repo
        .create_account("dave@example.com", HASH)
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
    let result = repo.create_account(&overlong, HASH).await;
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
