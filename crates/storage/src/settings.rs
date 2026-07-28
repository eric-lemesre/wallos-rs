//! Réglages du foyer (REQ-CUR-001).
//!
//! Réglages portés par le **foyer** (ADR 0012). Isolation §9 : lecture et écriture filtrées par
//! `household_id` de l'appelant (`&Actor`). Le seul réglage à ce jour est la **devise de référence**
//! (devise cible des agrégats) ; d'autres réglages de foyer rejoindront ce repository.

use wallos_core::actor::Actor;
use wallos_core::requirement;

use crate::StorageError;

/// Accès aux réglages du foyer.
pub struct SettingsRepository<'a> {
    pool: &'a sqlx::PgPool,
}

impl<'a> SettingsRepository<'a> {
    /// Construit le repository sur un pool.
    #[must_use]
    #[requirement(REQ-CUR-001)]
    pub const fn new(pool: &'a sqlx::PgPool) -> Self {
        Self { pool }
    }

    /// Lit le code de la devise de référence **du foyer de l'appelant**.
    ///
    /// # Errors
    /// `StorageError::Database` en cas d'échec de requête (le foyer existe toujours : réglage garanti).
    #[requirement(REQ-CUR-001)]
    pub async fn reference_currency(&self, actor: &Actor) -> Result<String, StorageError> {
        let (code,): (String,) =
            sqlx::query_as("select reference_currency from households where id = $1")
                .bind(actor.household_id())
                .fetch_one(self.pool)
                .await?;
        Ok(code)
    }

    /// Fixe la devise de référence **du foyer de l'appelant** (le code doit être déjà validé).
    ///
    /// N'altère aucun montant saisi : seule la devise cible des agrégats change (REQ-CUR-001).
    ///
    /// # Errors
    /// `StorageError::Database` en cas d'échec de mise à jour.
    #[requirement(REQ-CUR-001)]
    pub async fn set_reference_currency(
        &self,
        actor: &Actor,
        code: &str,
    ) -> Result<(), StorageError> {
        sqlx::query("update households set reference_currency = $2 where id = $1")
            .bind(actor.household_id())
            .bind(code)
            .execute(self.pool)
            .await?;
        Ok(())
    }
}
