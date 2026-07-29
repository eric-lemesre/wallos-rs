//! Idempotence des opérations d'écriture (REQ-SYN-006).
//!
//! Une requête d'écriture peut porter un en-tête `Idempotency-Key`. La première exécution mémorise la
//! réponse **réussie** (2xx) et l'empreinte du corps ; un rejeu à clé + corps identiques renvoie la
//! réponse mémorisée **sans nouvel effet de bord** ; une clé réutilisée avec un corps différent est un
//! conflit (`409`). Sans en-tête, le comportement est inchangé (idempotence *opt-in*).
//!
//! L'orchestration vit **dans les handlers de création** (après `AuthActor`, donc avec l'`Actor` et le
//! corps déjà validables) : pas de middleware, pas de mise en tampon de réponses arbitraires. La portée
//! par utilisateur et la réservation atomique sont assurées par [`IdempotencyRepository`].

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use wallos_core::actor::Actor;
use wallos_core::requirement;
use wallos_proto::problem;
use wallos_storage::{Db, IdempotencyRepository, Reservation};

use crate::problem_response;

/// Nom de l'en-tête de clé d'idempotence.
const HEADER: &str = "idempotency-key";

/// Extracteur **infaillible** de l'en-tête `Idempotency-Key` (absent ou illisible → `None`).
pub struct IdempotencyKey(pub Option<String>);

impl<S> FromRequestParts<S> for IdempotencyKey
where
    S: Send + Sync,
{
    type Rejection = std::convert::Infallible;

    #[requirement(REQ-SYN-006)]
    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let key = parts
            .headers
            .get(HEADER)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string)
            .filter(|s| !s.is_empty());
        Ok(Self(key))
    }
}

/// Suite d'une tentative idempotente.
pub enum Outcome {
    /// Clé neuve (ou absente) : le handler poursuit, puis appelle [`finish`] (succès) ou [`abort`] (erreur).
    Proceed,
    /// Rejeu (réponse mémorisée) ou conflit (`409`) : le handler renvoie cette réponse immédiatement.
    Short(Response),
}

/// Empreinte stable d'un corps de requête (sa sérialisation JSON déterministe).
fn fingerprint<T: Serialize>(req: &T) -> String {
    serde_json::to_string(req).unwrap_or_default()
}

/// `409` : clé d'idempotence réutilisée avec un corps différent (ou exécution concurrente en cours).
#[requirement(REQ-SYN-006)]
fn conflict() -> Response {
    problem_response(
        StatusCode::CONFLICT,
        problem(409, "about:blank", "Conflict")
            .with_detail("Idempotency-Key: déjà utilisée avec une charge utile différente"),
    )
}

/// Construit une réponse de **rejeu** à partir d'un statut + corps JSON mémorisés.
fn replay(status: u16, body: String) -> Response {
    let status = StatusCode::from_u16(status).unwrap_or(StatusCode::OK);
    (status, [(header::CONTENT_TYPE, "application/json")], body).into_response()
}

/// Réserve la clé (si présente) en tête de handler.
///
/// Renvoie [`Outcome::Proceed`] si la clé est absente ou neuve (le handler exécute l'opération), ou
/// [`Outcome::Short`] portant la réponse à renvoyer telle quelle (rejeu mémorisé, conflit, ou erreur
/// interne de réservation).
#[requirement(REQ-SYN-006)]
pub async fn begin<T: Serialize>(db: &Db, actor: &Actor, key: Option<&str>, req: &T) -> Outcome {
    let Some(key) = key else {
        return Outcome::Proceed;
    };
    match IdempotencyRepository::new(db.pool())
        .reserve(actor, key, &fingerprint(req))
        .await
    {
        Ok(Reservation::Proceed) => Outcome::Proceed,
        Ok(Reservation::Replay { status, body }) => Outcome::Short(replay(status, body)),
        Ok(Reservation::Conflict) => Outcome::Short(conflict()),
        Err(_) => Outcome::Short(problem_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            problem(500, "about:blank", "Internal Server Error"),
        )),
    }
}

/// Mémorise la réponse **réussie** d'une opération réservée (no-op si la clé est absente).
///
/// Best-effort **observable** (revue SYN-006 F2) : la réponse a déjà été produite, donc un échec de
/// mémorisation n'est pas bloquant pour la requête courante ; il est **journalisé** (`warn`) et la
/// réservation orpheline résultante est **auto-récupérée** au bout de `RESERVATION_TTL` par
/// `reserve` (F1). Sans cela, un rejeu resterait bloqué en conflit.
#[requirement(REQ-SYN-006)]
pub async fn finish(db: &Db, actor: &Actor, key: Option<&str>, status: StatusCode, body: &str) {
    if let Some(key) = key
        && let Err(err) = IdempotencyRepository::new(db.pool())
            .complete(actor, key, status.as_u16(), body)
            .await
    {
        tracing::warn!(
            error = %err,
            "échec de mémorisation d'idempotence (complete) ; réservation auto-récupérée après TTL"
        );
    }
}

/// Relâche la réservation d'une opération **en échec** (no-op si la clé est absente), pour autoriser
/// un nouvel essai avec la même clé. Un échec de relâche est **journalisé** (`warn`) ; la réservation
/// orpheline reste néanmoins auto-récupérable après `RESERVATION_TTL` (F1/F2).
#[requirement(REQ-SYN-006)]
pub async fn abort(db: &Db, actor: &Actor, key: Option<&str>) {
    if let Some(key) = key
        && let Err(err) = IdempotencyRepository::new(db.pool())
            .release(actor, key)
            .await
    {
        tracing::warn!(
            error = %err,
            "échec de relâche d'idempotence (release) ; réservation auto-récupérée après TTL"
        );
    }
}
