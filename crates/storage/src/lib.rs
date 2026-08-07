#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Couche d'accès aux données (PostgreSQL via sqlx).

#![forbid(unsafe_code)]

pub mod categories;
pub mod conflict_journal;
pub mod db;
pub mod device_tokens;
pub mod error;
pub mod exchange_rates;
pub mod idempotency;
pub mod login_attempts;
pub mod notification_channels;
pub mod notification_deliveries;
pub mod outcomes;
pub mod payers;
pub mod payment_methods;
pub mod reminders;
pub mod sessions;
pub mod settings;
pub mod subscriptions;
pub mod sync_changes;
pub mod tombstones;
pub mod users;

pub use categories::{CategoryRepository, CategoryRow, DeleteOutcome, RenameOutcome};
pub use conflict_journal::{ConflictJournalRepository, ConflictRow};
pub use db::Db;
pub use device_tokens::DeviceTokenRepository;
pub use error::StorageError;
pub use exchange_rates::{ExchangeRateRepository, StoredRate};
pub use idempotency::{IdempotencyRepository, Reservation};
pub use login_attempts::LoginAttemptRepository;
pub use notification_channels::{NotificationChannelRepository, NotificationChannelRow};
pub use notification_deliveries::{
    DueDeliveryRow, NotificationDeliveryRepository, NotificationDeliveryRow,
};
pub use outcomes::CreateOutcome;
pub use payers::{DeleteOutcome as PayerDeleteOutcome, PayerRepository, PayerRow};
pub use payment_methods::{PaymentMethodRepository, PaymentMethodRow};
pub use reminders::{ReminderRepository, ReminderScanRow};
pub use sessions::SessionRepository;
pub use settings::SettingsRepository;
pub use subscriptions::{SubscriptionFilter, SubscriptionRepository, SubscriptionRow};
pub use sync_changes::{ChangeRow, SyncChangesRepository};
pub use tombstones::{TombstoneRepository, TombstoneRow};
pub use users::{CreatedAccount, Credentials, StoredUser, UserRepository};
