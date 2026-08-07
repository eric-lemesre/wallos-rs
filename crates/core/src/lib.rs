//! Domaine pur de wallos-rs.
//!
//! ZÉRO I/O, zéro async, zéro dépendance réseau.
//! Récurrences, échéances, conversion de devises, agrégats statistiques.

#![deny(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![cfg_attr(test, allow(clippy::unwrap_used))]

pub use wallos_req_macros::{requirement, verifies};

pub mod actor;
pub mod billing;
pub mod category;
pub mod currencies;
pub mod error;
pub mod exchange;
pub mod language;
pub mod money;
pub mod password_policy;
pub mod payer;
pub mod payment_method;
pub mod reference_currency;
pub mod reminders;
pub mod schedule;
pub mod stats;
pub mod subscription;
pub mod sync;
pub mod text;

pub use billing::{BillingCycle, BillingUnit};
pub use category::{
    Category, DEFAULT_CATEGORY_COUNT, category_is_deletable, default_category_names,
};
pub use error::DomainError;
pub use exchange::{
    ConvertedTotal, ExchangeRate, RateProvider, RateTable, aggregate_converted, convert,
};
pub use language::Language;
pub use payer::Payer;
pub use payment_method::PaymentMethod;
pub use reference_currency::ReferenceCurrency;
pub use reminders::{
    DEFAULT_REMINDER_LEAD_DAYS, DueReminder, MAX_DELIVERY_ATTEMPTS, ReminderCandidate,
    due_reminders, retry_delay_minutes,
};
pub use schedule::{next_due, occurrences_in_range};
pub use stats::{
    CostSpan, MonthlyCostPoint, RepartitionShare, RepartitionSlice, monthly_cost_evolution,
    repartition,
};
pub use subscription::Subscription;
pub use sync::{
    Arbitration, DEFAULT_TOMBSTONE_RETENTION_DAYS, SyncCursor, arbitrate, requires_full_resync,
    retention_cutoff,
};
pub use text::{fold_for_search, matches_search};
