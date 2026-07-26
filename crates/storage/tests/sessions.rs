//! Tests d'intégration des sessions et des identifiants (REQ-AUT-002).

use chrono::{Duration, Utc};
use wallos_core::actor::Actor;
use wallos_core::verifies;
use wallos_storage::{SessionRepository, UserRepository};

const HASH: &str = "$argon2id$v=19$m=19456,t=2,p=1$fakefakefake$deadbeef";

async fn seed_account(pool: &sqlx::PgPool, email: &str) -> Actor {
    let created = UserRepository::new(pool)
        .create_account(email, HASH)
        .await
        .unwrap()
        .unwrap();
    Actor::new(created.user_id, created.household_id)
}

#[sqlx::test]
#[verifies(REQ-AUT-002)]
async fn valid_session_resolves_to_its_actor(pool: sqlx::PgPool) {
    let actor = seed_account(&pool, "alice@example.com").await;
    let sessions = SessionRepository::new(&pool);
    let now = Utc::now();
    sessions
        .create(&actor, b"token-hash-1", now + Duration::hours(1))
        .await
        .unwrap();

    let resolved = sessions.find_valid(b"token-hash-1", now).await.unwrap();
    let resolved = resolved.expect("valid session resolves");
    assert_eq!(resolved.user_id(), actor.user_id());
    assert_eq!(resolved.household_id(), actor.household_id());
}

#[sqlx::test]
#[verifies(REQ-AUT-002)]
async fn expired_session_is_invisible(pool: sqlx::PgPool) {
    let actor = seed_account(&pool, "bob@example.com").await;
    let sessions = SessionRepository::new(&pool);
    let now = Utc::now();
    // Session déjà expirée (expires_at < now).
    sessions
        .create(&actor, b"token-hash-2", now - Duration::hours(1))
        .await
        .unwrap();

    assert!(
        sessions
            .find_valid(b"token-hash-2", now)
            .await
            .unwrap()
            .is_none()
    );
}

#[sqlx::test]
#[verifies(REQ-AUT-004)]
async fn touch_slides_expiry_and_keeps_session_valid(pool: sqlx::PgPool) {
    let actor = seed_account(&pool, "erin@example.com").await;
    let sessions = SessionRepository::new(&pool);
    let now = Utc::now();
    // Session sur le point d'expirer.
    sessions
        .create(&actor, b"token-slide", now + Duration::seconds(1))
        .await
        .unwrap();

    // Un instant après l'expiration initiale : sans slide, elle serait invalide.
    let later = now + Duration::minutes(5);
    assert!(
        sessions
            .find_valid(b"token-slide", later)
            .await
            .unwrap()
            .is_none()
    );

    // On repousse l'expiration (activité) : la session redevient valide à `later`.
    sessions
        .touch(b"token-slide", later + Duration::minutes(30))
        .await
        .unwrap();
    assert!(
        sessions
            .find_valid(b"token-slide", later)
            .await
            .unwrap()
            .is_some()
    );
}

#[sqlx::test]
#[verifies(REQ-AUT-009)]
async fn delete_invalidates_session_and_is_idempotent(pool: sqlx::PgPool) {
    let actor = seed_account(&pool, "frank@example.com").await;
    let sessions = SessionRepository::new(&pool);
    let now = Utc::now();
    sessions
        .create(&actor, b"token-del", now + Duration::minutes(30))
        .await
        .unwrap();
    assert!(
        sessions
            .find_valid(b"token-del", now)
            .await
            .unwrap()
            .is_some()
    );

    sessions.delete(b"token-del").await.unwrap();
    assert!(
        sessions
            .find_valid(b"token-del", now)
            .await
            .unwrap()
            .is_none()
    );

    // Rejeu : reste Ok (idempotent).
    sessions.delete(b"token-del").await.unwrap();
}

#[sqlx::test]
#[verifies(REQ-AUT-004)]
async fn touch_on_unknown_token_is_a_noop(pool: sqlx::PgPool) {
    let sessions = SessionRepository::new(&pool);
    // Aucun jeton correspondant : ne doit pas échouer.
    sessions
        .touch(b"nope", Utc::now() + Duration::minutes(30))
        .await
        .unwrap();
}

#[sqlx::test]
#[verifies(REQ-AUT-002)]
async fn unknown_token_resolves_to_none(pool: sqlx::PgPool) {
    let sessions = SessionRepository::new(&pool);
    assert!(
        sessions
            .find_valid(b"nope", Utc::now())
            .await
            .unwrap()
            .is_none()
    );
}

#[sqlx::test]
#[verifies(REQ-AUT-002)]
async fn credentials_lookup_returns_hash_for_known_email(pool: sqlx::PgPool) {
    let actor = seed_account(&pool, "carol@example.com").await;
    let creds = UserRepository::new(&pool)
        .find_credentials_by_email("carol@example.com")
        .await
        .unwrap()
        .expect("known email has credentials");
    assert_eq!(creds.actor.user_id(), actor.user_id());
    assert_eq!(creds.password_hash, HASH);
}

#[sqlx::test]
#[verifies(REQ-AUT-002)]
async fn credentials_lookup_returns_none_for_unknown_email(pool: sqlx::PgPool) {
    let creds = UserRepository::new(&pool)
        .find_credentials_by_email("ghost@example.com")
        .await
        .unwrap();
    assert!(creds.is_none());
}
