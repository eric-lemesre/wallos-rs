#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Couche d'accès aux données (PostgreSQL via sqlx).

#![forbid(unsafe_code)]

pub mod db;
pub mod device_tokens;
pub mod error;
pub mod login_attempts;
pub mod sessions;
pub mod users;

pub use db::Db;
pub use device_tokens::DeviceTokenRepository;
pub use error::StorageError;
pub use login_attempts::LoginAttemptRepository;
pub use sessions::SessionRepository;
pub use users::{CreatedAccount, Credentials, StoredUser, UserRepository};
