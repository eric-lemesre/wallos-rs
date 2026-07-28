#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Couche d'accès aux données (PostgreSQL via sqlx).

#![forbid(unsafe_code)]

pub mod categories;
pub mod db;
pub mod device_tokens;
pub mod error;
pub mod exchange_rates;
pub mod login_attempts;
pub mod payment_methods;
pub mod sessions;
pub mod settings;
pub mod subscriptions;
pub mod users;

pub use categories::{CategoryRepository, CategoryRow, RenameOutcome};
pub use db::Db;
pub use device_tokens::DeviceTokenRepository;
pub use error::StorageError;
pub use exchange_rates::{ExchangeRateRepository, StoredRate};
pub use login_attempts::LoginAttemptRepository;
pub use payment_methods::{PaymentMethodRepository, PaymentMethodRow};
pub use sessions::SessionRepository;
pub use settings::SettingsRepository;
pub use subscriptions::{SubscriptionFilter, SubscriptionRepository, SubscriptionRow};
pub use users::{CreatedAccount, Credentials, StoredUser, UserRepository};
