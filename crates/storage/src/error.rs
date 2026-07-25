//! Erreurs de la couche storage.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("connection error: {0}")]
    Connection(String),
}
