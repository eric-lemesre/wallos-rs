//! Domaine pur de wallos-rs.
//!
//! ZÉRO I/O, zéro async, zéro dépendance réseau.
//! Récurrences, échéances, conversion de devises, agrégats statistiques.

#![deny(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![cfg_attr(test, allow(clippy::unwrap_used))]

pub use wallos_req_macros::{requirement, verifies};

pub mod actor;
pub mod error;
pub mod money;
pub mod password_policy;
pub mod stats;

pub use error::DomainError;
