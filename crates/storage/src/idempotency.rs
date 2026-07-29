//! Repository d'idempotence des écritures (REQ-SYN-006).
//!
//! Portée **par utilisateur** (`user_id`, §9) : une clé n'interfère jamais entre comptes. La
//! réservation est **atomique** (`insert ... on conflict do nothing`) : un seul appelant obtient
//! [`Reservation::Proceed`], les rejeux concurrents ou ultérieurs obtiennent [`Reservation::Replay`]
//! (même corps, réponse mémorisée) ou [`Reservation::Conflict`] (corps différent, ou exécution encore
//! en cours). Seules les réponses **réussies** sont mémorisées ([`complete`](IdempotencyRepository::complete)) ;
//! sur erreur, la réservation est **relâchée** ([`release`](IdempotencyRepository::release)) pour
//! autoriser un nouvel essai.

use wallos_core::actor::Actor;
use wallos_core::requirement;

use crate::StorageError;

/// Statut sentinelle d'une réservation **en cours** (jamais un vrai code HTTP, tous ≥ 100).
const PENDING: i16 = 0;

/// Durée au-delà de laquelle une réservation **encore en cours** (`PENDING`) est considérée
/// **orpheline** (revue SYN-006 F1) : le handler qui l'a posée est mort avant `complete`/`release`
/// (panic, arrêt du nœud, coupure). Un handler nominal se termine en millisecondes ; 5 minutes est
/// une marge très large qui ne peut jamais recouvrir une exécution légitime en cours. Passé ce délai,
/// la clé est **récupérable** — sinon un incident bloquerait la clé indéfiniment.
const RESERVATION_TTL: &str = "5 minutes";

/// Résultat d'une tentative de réservation d'une clé d'idempotence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reservation {
    /// Clé neuve : l'appelant exécute l'opération, puis `complete` (succès) ou `release` (erreur).
    Proceed,
    /// Clé déjà vue avec le **même** corps et une réponse mémorisée : rejeu → renvoyer telle quelle.
    Replay {
        /// Code HTTP mémorisé de la première réponse réussie.
        status: u16,
        /// Corps (JSON) mémorisé de la première réponse réussie.
        body: String,
    },
    /// Clé réutilisée avec un corps **différent** (→ 409), ou première exécution encore en cours.
    Conflict,
}

/// Accès au registre d'idempotence.
pub struct IdempotencyRepository<'a> {
    pool: &'a sqlx::PgPool,
}

impl<'a> IdempotencyRepository<'a> {
    /// Construit le repository sur un pool.
    #[must_use]
    #[requirement(REQ-SYN-006)]
    pub const fn new(pool: &'a sqlx::PgPool) -> Self {
        Self { pool }
    }

    /// Réserve la clé pour l'utilisateur de l'appelant.
    ///
    /// Insère une réservation « en cours » de façon **atomique**. Si la clé est neuve, renvoie
    /// [`Reservation::Proceed`]. Sinon, décide d'après l'empreinte du corps :
    /// - même empreinte **et** réponse mémorisée → [`Reservation::Replay`] ;
    /// - empreinte différente, ou exécution encore en cours (récente) → [`Reservation::Conflict`] ;
    /// - réservation `PENDING` **orpheline** (plus vieille que [`RESERVATION_TTL`], handler mort) →
    ///   **récupérée** (revue F1) et renvoie [`Reservation::Proceed`].
    ///
    /// # Errors
    /// `StorageError::Database` en cas d'échec de requête.
    #[requirement(REQ-SYN-006)]
    pub async fn reserve(
        &self,
        actor: &Actor,
        key: &str,
        fingerprint: &str,
    ) -> Result<Reservation, StorageError> {
        let inserted = sqlx::query(
            "insert into idempotency_keys \
                 (user_id, idempotency_key, request_fingerprint, response_status, response_body) \
             values ($1, $2, $3, $4, '') on conflict do nothing",
        )
        .bind(actor.user_id())
        .bind(key)
        .bind(fingerprint)
        .bind(PENDING)
        .execute(self.pool)
        .await?;
        if inserted.rows_affected() == 1 {
            return Ok(Reservation::Proceed);
        }
        // La clé existe déjà : rejeu (même corps, réponse prête) ou conflit (corps différent / en cours).
        let existing: Option<(String, i16, String)> = sqlx::query_as(
            "select request_fingerprint, response_status, response_body from idempotency_keys \
             where user_id = $1 and idempotency_key = $2",
        )
        .bind(actor.user_id())
        .bind(key)
        .fetch_optional(self.pool)
        .await?;
        match existing {
            Some((fingerprint_stored, status, body))
                if status != PENDING && fingerprint_stored == fingerprint =>
            {
                Ok(Reservation::Replay {
                    status: u16::try_from(status).unwrap_or(0),
                    body,
                })
            }
            // Réponse mémorisée mais corps différent : conflit franc (409).
            Some((_, status, _)) if status != PENDING => Ok(Reservation::Conflict),
            // Réservation encore `PENDING` : soit une exécution concurrente légitime (récente →
            // conflit), soit un orphelin (handler mort → récupérable). Le `UPDATE` conditionnel sur
            // l'âge tranche **atomiquement** : un seul appelant récupère l'orphelin.
            Some(_) => {
                let reclaimed = sqlx::query(&format!(
                    "update idempotency_keys \
                         set request_fingerprint = $3, response_status = $4, response_body = '', \
                             created_at = now() \
                     where user_id = $1 and idempotency_key = $2 \
                       and response_status = $4 \
                       and created_at < now() - interval '{RESERVATION_TTL}'",
                ))
                .bind(actor.user_id())
                .bind(key)
                .bind(fingerprint)
                .bind(PENDING)
                .execute(self.pool)
                .await?;
                if reclaimed.rows_affected() == 1 {
                    Ok(Reservation::Proceed)
                } else {
                    Ok(Reservation::Conflict)
                }
            }
            // Supprimée entre l'insertion et la relecture (course rare) : traité comme conflit.
            None => Ok(Reservation::Conflict),
        }
    }

    /// Mémorise la réponse **réussie** d'une opération précédemment réservée.
    ///
    /// # Errors
    /// `StorageError::Database` en cas d'échec de requête.
    #[requirement(REQ-SYN-006)]
    pub async fn complete(
        &self,
        actor: &Actor,
        key: &str,
        status: u16,
        body: &str,
    ) -> Result<(), StorageError> {
        sqlx::query(
            "update idempotency_keys set response_status = $3, response_body = $4 \
             where user_id = $1 and idempotency_key = $2",
        )
        .bind(actor.user_id())
        .bind(key)
        .bind(i16::try_from(status).unwrap_or(PENDING))
        .bind(body)
        .execute(self.pool)
        .await?;
        Ok(())
    }

    /// Relâche une réservation (opération non mémorisable : réponse d'erreur) pour permettre un
    /// nouvel essai avec la même clé.
    ///
    /// # Errors
    /// `StorageError::Database` en cas d'échec de requête.
    #[requirement(REQ-SYN-006)]
    pub async fn release(&self, actor: &Actor, key: &str) -> Result<(), StorageError> {
        sqlx::query("delete from idempotency_keys where user_id = $1 and idempotency_key = $2")
            .bind(actor.user_id())
            .bind(key)
            .execute(self.pool)
            .await?;
        Ok(())
    }
}
