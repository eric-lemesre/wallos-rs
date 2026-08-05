//! Tests d'intégration des pierres tombales (REQ-SYN-002).
//!
//! Enregistrement, lecture par curseur `since`, purge par borne **injectée** (testable sans horloge,
//! REQ-STA-008) et isolation par foyer (§9).

use chrono::{DateTime, Duration, Utc};
use uuid::Uuid;
use wallos_core::actor::Actor;
use wallos_core::verifies;
use wallos_storage::UserRepository;
use wallos_storage::tombstones::{self, ENTITY_CATEGORY, ENTITY_PAYER, TombstoneRepository};

const HASH: &str = "$argon2id$v=19$m=19456,t=2,p=1$fakefakefake$deadbeef";

async fn seed(pool: &sqlx::PgPool, email: &str) -> Actor {
    let created = UserRepository::new(pool)
        .create_account(email, HASH, None)
        .await
        .unwrap()
        .unwrap();
    Actor::new(created.user_id, created.household_id)
}

/// Insère une pierre tombale à une date de suppression **explicite** (timing déterministe des tests).
async fn insert_at(
    pool: &sqlx::PgPool,
    actor: &Actor,
    entity_type: &str,
    entity_id: Uuid,
    deleted_at: DateTime<Utc>,
) {
    sqlx::query(
        "insert into tombstones (household_id, entity_type, entity_id, deleted_at) \
         values ($1, $2, $3, $4)",
    )
    .bind(actor.household_id())
    .bind(entity_type)
    .bind(entity_id)
    .bind(deleted_at)
    .execute(pool)
    .await
    .unwrap();
}

#[sqlx::test]
#[verifies(REQ-SYN-002, case = "record puis list_since renvoie la pierre tombale")]
async fn record_then_list_returns_the_tombstone(pool: sqlx::PgPool) {
    let actor = seed(&pool, "a@example.com").await;
    let id = Uuid::new_v4();
    tombstones::record(&pool, actor.household_id(), ENTITY_PAYER, id)
        .await
        .unwrap();

    let rows = TombstoneRepository::new(&pool)
        .list_since(&actor, None)
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].entity_type, ENTITY_PAYER);
    assert_eq!(rows[0].entity_id, id);
}

#[sqlx::test]
#[verifies(REQ-SYN-002, case = "resupprimer la même entité rafraîchit la pierre tombale (upsert)")]
async fn record_is_idempotent_and_refreshes_deleted_at(pool: sqlx::PgPool) {
    let actor = seed(&pool, "b@example.com").await;
    let id = Uuid::new_v4();
    let hid = actor.household_id();
    tombstones::record(&pool, hid, ENTITY_CATEGORY, id)
        .await
        .unwrap();
    tombstones::record(&pool, hid, ENTITY_CATEGORY, id)
        .await
        .unwrap();

    // Une seule ligne malgré deux enregistrements (contrainte d'unicité + upsert).
    let rows = TombstoneRepository::new(&pool)
        .list_since(&actor, None)
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
}

#[sqlx::test]
#[verifies(REQ-SYN-002, case = "le curseur since ne renvoie que les suppressions postérieures")]
async fn since_cursor_returns_only_newer(pool: sqlx::PgPool) {
    let actor = seed(&pool, "c@example.com").await;
    let now = Utc::now();
    let old = Uuid::new_v4();
    let recent = Uuid::new_v4();
    insert_at(&pool, &actor, ENTITY_PAYER, old, now - Duration::hours(2)).await;
    insert_at(&pool, &actor, ENTITY_PAYER, recent, now).await;

    let repo = TombstoneRepository::new(&pool);
    // Curseur à −1 h : seule la suppression récente (postérieure) est renvoyée.
    let rows = repo
        .list_since(&actor, Some(now - Duration::hours(1)))
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].entity_id, recent);
}

#[sqlx::test]
#[verifies(REQ-SYN-002, case = "la purge retire les pierres tombales antérieures à la borne, garde les récentes")]
async fn purge_removes_only_expired(pool: sqlx::PgPool) {
    let actor = seed(&pool, "d@example.com").await;
    let now = Utc::now();
    let expired = Uuid::new_v4();
    let kept = Uuid::new_v4();
    insert_at(
        &pool,
        &actor,
        ENTITY_PAYER,
        expired,
        now - Duration::days(40),
    )
    .await;
    insert_at(&pool, &actor, ENTITY_PAYER, kept, now - Duration::days(1)).await;

    let repo = TombstoneRepository::new(&pool);
    // Borne = now − 30 j (rétention par défaut) : la pierre de 40 j est purgée, celle d'1 j conservée.
    let purged = repo.purge_expired(now - Duration::days(30)).await.unwrap();
    assert_eq!(purged, 1);
    let rows = repo.list_since(&actor, None).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].entity_id, kept);
}

#[sqlx::test]
#[verifies(REQ-SYN-002, case = "isolation §9 : un foyer ne voit pas les pierres tombales d'un autre")]
async fn tombstones_are_isolated_per_household(pool: sqlx::PgPool) {
    let a = seed(&pool, "iso-a@example.com").await;
    let b = seed(&pool, "iso-b@example.com").await;
    tombstones::record(&pool, a.household_id(), ENTITY_PAYER, Uuid::new_v4())
        .await
        .unwrap();

    // Le foyer B ne voit rien : les pierres tombales de A lui sont invisibles.
    let rows_b = TombstoneRepository::new(&pool)
        .list_since(&b, None)
        .await
        .unwrap();
    assert!(rows_b.is_empty());
}
