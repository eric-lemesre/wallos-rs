//! Synchronisation : pierres tombales (REQ-SYN-002).
//!
//! `GET /sync/tombstones` renvoie les **suppressions** qu'un appareil doit appliquer, à partir d'un
//! curseur `since` (exclusif). Le serveur **purge** au passage les pierres tombales plus anciennes que
//! la rétention (ADR 0013, défaut 30 j, `TOMBSTONE_RETENTION_DAYS`) et signale
//! `full_resync_required = true` si le curseur précède cette fenêtre (delta incomplet → resynchronisation
//! complète). Horloge serveur seulement pour « maintenant » ; la logique de péremption est **pure**
//! (`core::requires_full_resync`, fenêtre injectée, REQ-STA-008).

use std::sync::LazyLock;

use axum::Json;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, SecondsFormat, Utc};
use wallos_core::requirement;
use wallos_core::{DEFAULT_TOMBSTONE_RETENTION_DAYS, requires_full_resync, retention_cutoff};
use wallos_proto::{TombstoneDto, TombstonesQuery, TombstonesResponse, problem};
use wallos_storage::{Db, TombstoneRepository};

use crate::auth::AuthActor;
use crate::problem_response;

/// Fenêtre de rétention des pierres tombales (jours), **configurable côté serveur** via
/// `TOMBSTONE_RETENTION_DAYS` (défaut 30, ADR 0013), jamais par le client. Une valeur non entière ou
/// non positive retombe sur le défaut.
static RETENTION_DAYS: LazyLock<i64> = LazyLock::new(|| {
    std::env::var("TOMBSTONE_RETENTION_DAYS")
        .ok()
        .and_then(|v| v.parse::<i64>().ok())
        .filter(|d| *d > 0)
        .unwrap_or(DEFAULT_TOMBSTONE_RETENTION_DAYS)
});

/// `422` identifiant le paramètre fautif.
#[requirement(REQ-SYN-002)]
fn field_error(field: &str, message: &str) -> Response {
    problem_response(
        StatusCode::UNPROCESSABLE_ENTITY,
        problem(422, "about:blank", "Unprocessable Entity")
            .with_detail(format!("{field}: {message}")),
    )
}

/// `500` générique (défaut interne non divulgué).
#[requirement(REQ-SYN-002)]
fn internal_error() -> Response {
    problem_response(
        StatusCode::INTERNAL_SERVER_ERROR,
        problem(500, "about:blank", "Internal Server Error"),
    )
}

/// Pierres tombales à appliquer par un appareil qui se synchronise (REQ-SYN-002).
///
/// Curseur `since` **exclusif** (RFC 3339) : renvoie les suppressions postérieures. Le serveur purge les
/// pierres tombales expirées (rétention, ADR 0013) puis, si `since` précède la fenêtre de rétention (ou
/// est absent → première synchronisation), signale `full_resync_required` : l'appareil se resynchronise
/// entièrement plutôt que d'appliquer un delta silencieusement incomplet. Isolation §9.
#[utoipa::path(
    get,
    path = "/sync/tombstones",
    operation_id = "getTombstones",
    extensions(("x-requirements" = json!(["REQ-SYN-002"]))),
    params(TombstonesQuery),
    responses(
        (status = 200, description = "Suppressions à appliquer", body = TombstonesResponse, content_type = "application/json"),
        (status = 401, description = "Non authentifié", body = wallos_proto::Problem, content_type = "application/problem+json"),
        (status = 422, description = "Curseur invalide", body = wallos_proto::Problem, content_type = "application/problem+json"),
        (status = 500, description = "Erreur interne", body = wallos_proto::Problem, content_type = "application/problem+json")
    )
)]
#[requirement(REQ-SYN-002)]
pub async fn get_tombstones(
    AuthActor(actor): AuthActor,
    State(db): State<Db>,
    Query(q): Query<TombstonesQuery>,
) -> Response {
    // Curseur optionnel : RFC 3339 strict. Une valeur présente mais illisible → 422 (jamais ignorée en
    // silence, ce qui masquerait des suppressions au client).
    let since: Option<DateTime<Utc>> = match q.since.as_deref().filter(|s| !s.is_empty()) {
        None => None,
        Some(raw) => match DateTime::parse_from_rfc3339(raw) {
            Ok(dt) => Some(dt.with_timezone(&Utc)),
            Err(_) => return field_error("since", "horodatage RFC 3339 attendu"),
        },
    };

    let retention = *RETENTION_DAYS;
    let now = Utc::now();
    let repo = TombstoneRepository::new(db.pool());

    // Purge d'entretien : borne calculée (now − rétention), injectée au storage (testable sans horloge).
    if repo
        .purge_expired(retention_cutoff(now, retention))
        .await
        .is_err()
    {
        return internal_error();
    }

    // Curseur périmé (ou absent) → resynchronisation complète requise (ADR 0013).
    let full_resync_required = since.is_none_or(|s| requires_full_resync(s, now, retention));

    let rows = match repo.list_since(&actor, since).await {
        Ok(rows) => rows,
        Err(_) => return internal_error(),
    };
    let tombstones = rows
        .into_iter()
        .map(|r| TombstoneDto {
            entity_type: r.entity_type,
            entity_id: r.entity_id.to_string(),
            // Suffixe `Z` (jamais `+00:00`) : sûr dans une query URL où « + » se décode en espace.
            // Précision microseconde : curseur `since` exclusif sans collision entre suppressions proches.
            deleted_at: r.deleted_at.to_rfc3339_opts(SecondsFormat::Micros, true),
        })
        .collect();

    Json(TombstonesResponse {
        tombstones,
        full_resync_required,
        retention_days: retention,
        as_of: now.to_rfc3339_opts(SecondsFormat::Micros, true),
    })
    .into_response()
}
