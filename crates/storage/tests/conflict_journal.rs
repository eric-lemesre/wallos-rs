//! Tests d'intégration du journal des conflits (REQ-SYN-005) : enregistrement, purge par borne
//! **injectée** (testable sans horloge), isolation §9.

use chrono::{Duration, Utc};
use uuid::Uuid;
use wallos_core::actor::Actor;
use wallos_core::verifies;
use wallos_storage::UserRepository;
use wallos_storage::conflict_journal::{ConflictJournalRepository, REASON_OVERWRITTEN};

const HASH: &str = "$argon2id$v=19$m=19456,t=2,p=1$fakefakefake$deadbeef";

async fn seed(pool: &sqlx::PgPool, email: &str) -> Actor {
    let created = UserRepository::new(pool)
        .create_account(email, HASH, None)
        .await
        .unwrap()
        .unwrap();
    Actor::new(created.user_id, created.household_id)
}

#[sqlx::test]
#[verifies(REQ-SYN-005, case = "record puis list renvoie l'entrée du foyer")]
async fn record_then_list(pool: sqlx::PgPool) {
    let actor = seed(&pool, "cj-a@example.com").await;
    let repo = ConflictJournalRepository::new(&pool);
    repo.record(
        &actor,
        "payer",
        Uuid::new_v4(),
        "{\"name\":\"Alex\"}",
        REASON_OVERWRITTEN,
    )
    .await
    .unwrap();
    let rows = repo.list(&actor).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].reason, REASON_OVERWRITTEN);
    assert_eq!(rows[0].lost_payload, "{\"name\":\"Alex\"}");
}

#[sqlx::test]
#[verifies(REQ-SYN-005, case = "la purge retire les entrées antérieures à la borne, garde les récentes")]
async fn purge_removes_only_expired(pool: sqlx::PgPool) {
    let actor = seed(&pool, "cj-p@example.com").await;
    // Deux entrées à dates explicites : une vieille (40 j), une récente (1 j).
    sqlx::query(
        "insert into conflict_journal (household_id, entity_type, entity_id, lost_payload, reason, recorded_at) \
         values ($1,'payer',$2,'{}','overwritten',$3), ($1,'payer',$4,'{}','overwritten',$5)",
    )
    .bind(actor.household_id())
    .bind(Uuid::new_v4())
    .bind(Utc::now() - Duration::days(40))
    .bind(Uuid::new_v4())
    .bind(Utc::now() - Duration::days(1))
    .execute(&pool)
    .await
    .unwrap();

    let repo = ConflictJournalRepository::new(&pool);
    // Borne = now − 30 j : la vieille est purgée, la récente conservée.
    let purged = repo
        .purge_expired(Utc::now() - Duration::days(30))
        .await
        .unwrap();
    assert_eq!(purged, 1);
    assert_eq!(repo.list(&actor).await.unwrap().len(), 1);
}

#[sqlx::test]
#[verifies(REQ-SYN-005, case = "isolation §9 : un foyer ne voit pas le journal d'un autre")]
async fn journal_is_isolated_per_household(pool: sqlx::PgPool) {
    let a = seed(&pool, "cj-iso-a@example.com").await;
    let b = seed(&pool, "cj-iso-b@example.com").await;
    ConflictJournalRepository::new(&pool)
        .record(&a, "payer", Uuid::new_v4(), "{}", REASON_OVERWRITTEN)
        .await
        .unwrap();
    assert!(
        ConflictJournalRepository::new(&pool)
            .list(&b)
            .await
            .unwrap()
            .is_empty()
    );
}
