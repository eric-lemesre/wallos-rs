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
use uuid::Uuid;
use wallos_core::requirement;
use wallos_core::{
    DEFAULT_TOMBSTONE_RETENTION_DAYS, SyncCursor, requires_full_resync, retention_cutoff,
};
use wallos_proto::{
    CreateSubscriptionRequest, SyncChangeDto, SyncChangesQuery, SyncChangesResponse,
    SyncOperationInput, SyncOperationResult, SyncPushRequest, SyncPushResponse, TombstoneDto,
    TombstonesQuery, TombstonesResponse, problem,
};
use wallos_storage::{
    CategoryRepository, CreateOutcome, Db, DeleteOutcome, PayerDeleteOutcome, PayerRepository,
    PaymentMethodRepository, RenameOutcome, SubscriptionRepository, SyncChangesRepository,
    TombstoneRepository,
};

use crate::auth::AuthActor;
use crate::idempotency::{self, IdempotencyKey, Outcome};
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

/// Taille de page par défaut du delta incrémental (REQ-SYN-003).
const DEFAULT_SYNC_LIMIT: u32 = 100;
/// Taille de page maximale (garde-fou).
const MAX_SYNC_LIMIT: u32 = 500;

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

/// Récupération incrémentale des changements par curseur (REQ-SYN-003).
///
/// Renvoie une **page** de changements (créations, modifications, suppressions) du foyer, triés par
/// `(horodatage, id)` croissant et **strictement postérieurs** au curseur, avec un `next_cursor` et un
/// drapeau `has_more`. Le client itère avec `next_cursor` jusqu'à `has_more = false`, puis conserve ce
/// dernier curseur comme **watermark**. Pagination **keyset** : stable, ni omission ni duplication
/// (critère #2). Si le curseur précède la fenêtre de rétention des pierres tombales (ADR 0013),
/// `full_resync_required` invite à repartir d'une synchronisation complète. Isolation §9.
#[utoipa::path(
    get,
    path = "/sync/changes",
    operation_id = "getSyncChanges",
    extensions(("x-requirements" = json!(["REQ-SYN-003"]))),
    params(SyncChangesQuery),
    responses(
        (status = 200, description = "Page de delta incrémental", body = SyncChangesResponse, content_type = "application/json"),
        (status = 401, description = "Non authentifié", body = wallos_proto::Problem, content_type = "application/problem+json"),
        (status = 422, description = "Curseur invalide", body = wallos_proto::Problem, content_type = "application/problem+json"),
        (status = 500, description = "Erreur interne", body = wallos_proto::Problem, content_type = "application/problem+json")
    )
)]
#[requirement(REQ-SYN-003)]
pub async fn get_sync_changes(
    AuthActor(actor): AuthActor,
    State(db): State<Db>,
    Query(q): Query<SyncChangesQuery>,
) -> Response {
    // Curseur : absent → première synchronisation (origine) ; présent mais illisible → 422.
    let cursor = match q.cursor.as_deref().filter(|s| !s.is_empty()) {
        None => SyncCursor::beginning(),
        Some(raw) => match SyncCursor::parse(raw) {
            Some(c) => c,
            None => return field_error("cursor", "curseur invalide"),
        },
    };
    // Taille de page bornée : 1..=MAX, défaut DEFAULT.
    let limit = q
        .limit
        .unwrap_or(DEFAULT_SYNC_LIMIT)
        .clamp(1, MAX_SYNC_LIMIT);

    // On demande une ligne de plus que la page pour détecter s'il reste des changements ensuite.
    let fetch = i64::from(limit) + 1;
    let mut rows = match SyncChangesRepository::new(db.pool())
        .changes(&actor, cursor, fetch)
        .await
    {
        Ok(rows) => rows,
        Err(_) => return internal_error(),
    };

    let has_more = rows.len() as i64 > i64::from(limit);
    rows.truncate(limit as usize);

    // Nouveau curseur : position après le dernier changement de la page (ou le curseur d'entrée si vide).
    let next_cursor = rows
        .last()
        .map_or(cursor, |r| SyncCursor {
            timestamp: r.ts,
            id: r.id,
        })
        .encode();

    // Curseur périmé (antérieur à la rétention) → resynchronisation complète conseillée (ADR 0013).
    let now = Utc::now();
    let full_resync_required = requires_full_resync(cursor.timestamp, now, *RETENTION_DAYS);

    // Le corps d'un upsert est une chaîne JSON déjà sérialisée (privée de household_id) : on la reparse
    // en valeur pour la transmettre telle quelle ; un corps illisible n'interrompt pas le delta (null).
    let changes = rows
        .into_iter()
        .map(|r| SyncChangeDto {
            kind: r.kind,
            entity_type: r.entity_type,
            id: r.id.to_string(),
            payload: r
                .payload
                .and_then(|p| serde_json::from_str::<serde_json::Value>(&p).ok()),
        })
        .collect();

    Json(SyncChangesResponse {
        changes,
        next_cursor,
        has_more,
        full_resync_required,
    })
    .into_response()
}

/// Suite d'une opération de lot : appliquée, ou rejetée avec motif (les autres restent appliquées).
enum Op {
    Applied,
    Rejected(String),
}

/// Nom (obligatoire) d'un upsert d'entité nominative, extrait de la charge utile.
fn name_of(op: &SyncOperationInput) -> Option<String> {
    op.payload
        .as_ref()?
        .get("name")?
        .as_str()
        .map(str::to_string)
}

/// Upsert d'une entité nominative dont le renommage renvoie un simple booléen (payeur, moyen de
/// paiement) : crée, ou met à jour si l'id existe déjà dans le foyer (sinon rejet — id d'un autre foyer).
async fn upsert_bool_named(created: CreateOutcome, renamed: bool) -> Op {
    match created {
        CreateOutcome::Created => Op::Applied,
        CreateOutcome::DuplicateName => Op::Rejected("nom déjà utilisé".to_string()),
        CreateOutcome::DuplicateId => {
            if renamed {
                Op::Applied
            } else {
                Op::Rejected("entité inconnue dans ce foyer".to_string())
            }
        }
    }
}

/// Applique une opération du lot. `Err` = échec d'infrastructure (→ 500) ; `Ok(Op)` = appliquée/rejetée.
#[requirement(REQ-SYN-004)]
async fn apply_op(
    db: &Db,
    actor: &wallos_core::actor::Actor,
    op: &SyncOperationInput,
) -> Result<Op, wallos_storage::StorageError> {
    let Ok(id) = Uuid::parse_str(&op.id) else {
        return Ok(Op::Rejected("identifiant invalide".to_string()));
    };
    let pool = db.pool();
    match (op.op.as_str(), op.entity_type.as_str()) {
        ("upsert", "category") => {
            let Some(name) = name_of(op) else {
                return Ok(Op::Rejected("nom requis".to_string()));
            };
            let repo = CategoryRepository::new(pool);
            match repo.create(actor, id, &name).await? {
                CreateOutcome::Created => Ok(Op::Applied),
                CreateOutcome::DuplicateName => Ok(Op::Rejected("nom déjà utilisé".to_string())),
                CreateOutcome::DuplicateId => match repo.rename(actor, id, &name).await? {
                    RenameOutcome::Renamed => Ok(Op::Applied),
                    RenameOutcome::Duplicate => Ok(Op::Rejected("nom déjà utilisé".to_string())),
                    RenameOutcome::NotFound => {
                        Ok(Op::Rejected("entité inconnue dans ce foyer".to_string()))
                    }
                },
            }
        }
        ("upsert", "payment_method") => {
            let Some(name) = name_of(op) else {
                return Ok(Op::Rejected("nom requis".to_string()));
            };
            let repo = PaymentMethodRepository::new(pool);
            let created = repo.create(actor, id, &name).await?;
            let renamed = matches!(created, CreateOutcome::DuplicateId)
                && repo.rename(actor, id, &name).await?;
            Ok(upsert_bool_named(created, renamed).await)
        }
        ("upsert", "payer") => {
            let Some(name) = name_of(op) else {
                return Ok(Op::Rejected("nom requis".to_string()));
            };
            let repo = PayerRepository::new(pool);
            let created = repo.create(actor, id, &name).await?;
            let renamed = matches!(created, CreateOutcome::DuplicateId)
                && repo.rename(actor, id, &name).await?;
            Ok(upsert_bool_named(created, renamed).await)
        }
        ("upsert", "subscription") => {
            let Some(payload) = op.payload.clone() else {
                return Ok(Op::Rejected("charge utile requise".to_string()));
            };
            let Ok(req) = serde_json::from_value::<CreateSubscriptionRequest>(payload) else {
                return Ok(Op::Rejected("charge utile invalide".to_string()));
            };
            let sub = match req.into_core(id) {
                Ok(sub) => sub,
                Err(err) => return Ok(Op::Rejected(format!("{}: {}", err.field, err.message))),
            };
            let repo = SubscriptionRepository::new(pool);
            match repo.create(actor, &sub).await? {
                CreateOutcome::Created => Ok(Op::Applied),
                CreateOutcome::DuplicateName => Ok(Op::Rejected("doublon".to_string())),
                CreateOutcome::DuplicateId => {
                    if repo.update(actor, &sub).await? {
                        Ok(Op::Applied)
                    } else {
                        Ok(Op::Rejected("entité inconnue dans ce foyer".to_string()))
                    }
                }
            }
        }
        // Une suppression est idempotente : supprimer une entité déjà absente atteint l'état voulu
        // (Appliquée). Seul un refus métier (entité référencée) est un rejet.
        ("delete", "category") => match CategoryRepository::new(pool).delete(actor, id).await? {
            DeleteOutcome::Deleted | DeleteOutcome::NotFound => Ok(Op::Applied),
            DeleteOutcome::InUse => Ok(Op::Rejected("référencée par un abonnement".to_string())),
        },
        ("delete", "payer") => match PayerRepository::new(pool).delete(actor, id).await? {
            PayerDeleteOutcome::Deleted | PayerDeleteOutcome::NotFound => Ok(Op::Applied),
            PayerDeleteOutcome::InUse => {
                Ok(Op::Rejected("référencé par un abonnement".to_string()))
            }
        },
        ("delete", "payment_method") => {
            PaymentMethodRepository::new(pool).delete(actor, id).await?;
            Ok(Op::Applied)
        }
        ("delete", "subscription") => {
            SubscriptionRepository::new(pool).delete(actor, id).await?;
            Ok(Op::Applied)
        }
        _ => Ok(Op::Rejected(
            "opération ou type d'entité inconnu".to_string(),
        )),
    }
}

/// Poussée d'un lot de modifications locales (REQ-SYN-004).
///
/// Chaque opération est appliquée **indépendamment** (succès partiel) : la réponse aligne un résultat par
/// opération, identifiant précisément les entités rejetées tandis que les autres sont appliquées (critère
/// #2). Le lot est **idempotent** : rejoué à l'identique (en-tête `Idempotency-Key`, REQ-SYN-006, ou par
/// nature — les upserts sont clés par `id`, les suppressions sont des no-op), il mène au **même état
/// final** qu'un envoi unique (critère #1). Isolation §9 : chaque opération est portée par le foyer de
/// l'appelant.
#[utoipa::path(
    post,
    path = "/sync/push",
    operation_id = "pushSyncChanges",
    extensions(("x-requirements" = json!(["REQ-SYN-004"]))),
    request_body = SyncPushRequest,
    responses(
        (status = 200, description = "Résultat par opération", body = SyncPushResponse, content_type = "application/json"),
        (status = 401, description = "Non authentifié", body = wallos_proto::Problem, content_type = "application/problem+json"),
        (status = 409, description = "Clé d'idempotence réutilisée avec un corps différent", body = wallos_proto::Problem, content_type = "application/problem+json"),
        (status = 500, description = "Erreur interne", body = wallos_proto::Problem, content_type = "application/problem+json")
    )
)]
#[requirement(REQ-SYN-004)]
pub async fn push_sync_changes(
    AuthActor(actor): AuthActor,
    State(db): State<Db>,
    IdempotencyKey(key): IdempotencyKey,
    Json(req): Json<SyncPushRequest>,
) -> Response {
    // Idempotence au niveau du lot (REQ-SYN-006) : un rejeu à clé + corps identiques renvoie la réponse
    // mémorisée sans réappliquer les opérations.
    if let Outcome::Short(resp) = idempotency::begin(&db, &actor, key.as_deref(), &req).await {
        return resp;
    }

    let mut results = Vec::with_capacity(req.operations.len());
    for op in &req.operations {
        match apply_op(&db, &actor, op).await {
            Ok(Op::Applied) => results.push(SyncOperationResult {
                entity_type: op.entity_type.clone(),
                id: op.id.clone(),
                status: "applied".to_string(),
                reason: None,
            }),
            Ok(Op::Rejected(reason)) => results.push(SyncOperationResult {
                entity_type: op.entity_type.clone(),
                id: op.id.clone(),
                status: "rejected".to_string(),
                reason: Some(reason),
            }),
            Err(_) => {
                // Échec d'infrastructure : on relâche la réservation d'idempotence pour permettre un
                // nouvel essai, et on renvoie 500 (aucune réponse mémorisée).
                idempotency::abort(&db, &actor, key.as_deref()).await;
                return internal_error();
            }
        }
    }

    let response = SyncPushResponse { results };
    let body = serde_json::to_string(&response).unwrap_or_default();
    idempotency::finish(&db, &actor, key.as_deref(), StatusCode::OK, &body).await;
    (
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        body,
    )
        .into_response()
}
