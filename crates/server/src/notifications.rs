//! Gestion des canaux de notification (REQ-NOT-005).
//!
//! CRUD **isolé par foyer** (§9) d'une abstraction de canal unique. La première implémentation est le
//! **webhook générique** : à la création, l'URL est validée contre la falsification de requête côté
//! serveur (SSRF, `wallos_notifier::webhook_url_is_safe`) — les adresses internes/bouclage sont refusées
//! (422). Les autres types de canaux (e-mail NOT-003, messageries NOT-004) réutiliseront ce module.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use uuid::Uuid;
use wallos_core::requirement;
use wallos_notifier::webhook_url_is_safe;
use wallos_proto::{
    CreateNotificationChannelRequest, NotificationChannelDto, NotificationChannelsResponse, problem,
};
use wallos_storage::{Db, NotificationChannelRepository, NotificationChannelRow};

use crate::auth::AuthActor;
use crate::problem_response;

/// Seul type de canal implémenté pour l'instant (REQ-NOT-005). Les autres → 422.
const KIND_WEBHOOK: &str = "webhook";

/// `500` générique (défaut interne non divulgué).
#[requirement(REQ-NOT-005)]
fn internal_error() -> Response {
    problem_response(
        StatusCode::INTERNAL_SERVER_ERROR,
        problem(500, "about:blank", "Internal Server Error"),
    )
}

/// `404` générique pour un canal inconnu ou hors du foyer — ne divulgue rien (§9).
#[requirement(REQ-NOT-005)]
fn channel_not_found() -> Response {
    problem_response(
        StatusCode::NOT_FOUND,
        problem(404, "about:blank", "Not Found"),
    )
}

/// `422` avec le champ fautif (RFC 9457 `detail`).
#[requirement(REQ-NOT-005)]
fn invalid(detail: &str) -> Response {
    problem_response(
        StatusCode::UNPROCESSABLE_ENTITY,
        problem(422, "about:blank", "Unprocessable Entity").with_detail(detail.to_string()),
    )
}

/// Projette une ligne stockée en DTO.
#[requirement(REQ-NOT-005)]
fn row_to_dto(row: NotificationChannelRow) -> NotificationChannelDto {
    NotificationChannelDto {
        id: row.id.to_string(),
        kind: row.kind,
        config: row.config,
        enabled: row.enabled,
    }
}

/// Crée un canal de notification dans le foyer de l'appelant (REQ-NOT-005).
///
/// Webhook : `config.url` doit être une URL `http(s)` **publique** ; les adresses internes, de
/// bouclage, privées ou `localhost` sont refusées (422) pour prévenir la SSRF (critère #2).
#[utoipa::path(
    post,
    path = "/notifications/channels",
    operation_id = "createNotificationChannel",
    extensions(("x-requirements" = json!(["REQ-NOT-005"]))),
    request_body = CreateNotificationChannelRequest,
    responses(
        (status = 201, description = "Canal créé", body = NotificationChannelDto, content_type = "application/json"),
        (status = 401, description = "Non authentifié", body = wallos_proto::Problem, content_type = "application/problem+json"),
        (status = 422, description = "Type non supporté, configuration invalide, ou URL refusée (SSRF)", body = wallos_proto::Problem, content_type = "application/problem+json"),
        (status = 500, description = "Erreur interne", body = wallos_proto::Problem, content_type = "application/problem+json")
    )
)]
#[requirement(REQ-NOT-005)]
pub async fn create_notification_channel(
    AuthActor(actor): AuthActor,
    State(db): State<Db>,
    Json(req): Json<CreateNotificationChannelRequest>,
) -> Response {
    // Seul le webhook est implémenté ; tout autre type est refusé explicitement.
    if req.kind != KIND_WEBHOOK {
        return invalid("kind: type de canal non supporté (webhook attendu)");
    }
    // `config.url` requis, chaîne, et **sûr** (anti-SSRF, validé à l'enregistrement).
    let Some(url) = req.config.get("url").and_then(|v| v.as_str()) else {
        return invalid("config.url: requis (chaîne)");
    };
    if !webhook_url_is_safe(url) {
        return invalid("config.url: adresse non autorisée (interne/bouclage) ou URL invalide");
    }
    // Config normalisée : on ne persiste que les clés connues du type (jamais le corps brut du client).
    let config = serde_json::json!({ "url": url });
    let enabled = req.enabled.unwrap_or(true);
    match NotificationChannelRepository::new(db.pool())
        .create(&actor, KIND_WEBHOOK, &config, enabled)
        .await
    {
        Ok(row) => (StatusCode::CREATED, Json(row_to_dto(row))).into_response(),
        Err(_) => internal_error(),
    }
}

/// Liste les canaux de notification du foyer de l'appelant (REQ-NOT-005).
#[utoipa::path(
    get,
    path = "/notifications/channels",
    operation_id = "listNotificationChannels",
    extensions(("x-requirements" = json!(["REQ-NOT-005"]))),
    responses(
        (status = 200, description = "Canaux du foyer", body = NotificationChannelsResponse, content_type = "application/json"),
        (status = 401, description = "Non authentifié", body = wallos_proto::Problem, content_type = "application/problem+json"),
        (status = 500, description = "Erreur interne", body = wallos_proto::Problem, content_type = "application/problem+json")
    )
)]
#[requirement(REQ-NOT-005)]
pub async fn list_notification_channels(
    AuthActor(actor): AuthActor,
    State(db): State<Db>,
) -> Response {
    match NotificationChannelRepository::new(db.pool())
        .list(&actor)
        .await
    {
        Ok(rows) => Json(NotificationChannelsResponse {
            channels: rows.into_iter().map(row_to_dto).collect(),
        })
        .into_response(),
        Err(_) => internal_error(),
    }
}

/// Supprime un canal de notification du foyer de l'appelant (REQ-NOT-005).
#[utoipa::path(
    delete,
    path = "/notifications/channels/{id}",
    operation_id = "deleteNotificationChannel",
    params(("id" = String, Path, description = "Identifiant (UUID) du canal")),
    extensions(("x-requirements" = json!(["REQ-NOT-005"]))),
    responses(
        (status = 204, description = "Canal supprimé"),
        (status = 401, description = "Non authentifié", body = wallos_proto::Problem, content_type = "application/problem+json"),
        (status = 404, description = "Canal inconnu ou hors du foyer", body = wallos_proto::Problem, content_type = "application/problem+json"),
        (status = 500, description = "Erreur interne", body = wallos_proto::Problem, content_type = "application/problem+json")
    )
)]
#[requirement(REQ-NOT-005)]
pub async fn delete_notification_channel(
    AuthActor(actor): AuthActor,
    State(db): State<Db>,
    Path(id): Path<String>,
) -> Response {
    let Ok(channel_id) = Uuid::parse_str(&id) else {
        return channel_not_found();
    };
    match NotificationChannelRepository::new(db.pool())
        .delete(&actor, channel_id)
        .await
    {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => channel_not_found(),
        Err(_) => internal_error(),
    }
}
