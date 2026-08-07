//! Gestion des canaux de notification (REQ-NOT-005, REQ-NOT-003, REQ-NOT-004).
//!
//! CRUD **isolé par foyer** (§9) d'une abstraction de canal unique : webhook générique (NOT-005),
//! e-mail SMTP (NOT-003) et messageries Telegram/Discord/Gotify/Pushover (NOT-004). Toute URL
//! fournie par l'utilisateur (webhook, Discord, Gotify) est validée contre la falsification de
//! requête côté serveur (SSRF, `wallos_notifier::webhook_url_is_safe`) — les adresses
//! internes/bouclage sont refusées (422). Les secrets stockés (mot de passe SMTP, jetons,
//! clé utilisateur) ne sont **jamais renvoyés** au client (redaction en sortie).

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use uuid::Uuid;
use wallos_core::requirement;
use wallos_notifier::{
    ReminderItem, ReminderNotification, diagnose_send_error, webhook_url_is_safe,
};
use wallos_proto::{
    CreateNotificationChannelRequest, NotificationChannelDto, NotificationChannelsResponse,
    TestNotificationChannelResponse, problem,
};
use wallos_storage::{Db, NotificationChannelRepository, NotificationChannelRow};

use crate::auth::AuthActor;
use crate::problem_response;

/// Type de canal webhook générique (REQ-NOT-005).
const KIND_WEBHOOK: &str = "webhook";

/// Types de canaux de messagerie tiers (REQ-NOT-004).
const KIND_TELEGRAM: &str = "telegram";
/// Voir [`KIND_TELEGRAM`].
const KIND_DISCORD: &str = "discord";
/// Voir [`KIND_TELEGRAM`].
const KIND_GOTIFY: &str = "gotify";
/// Voir [`KIND_TELEGRAM`].
const KIND_PUSHOVER: &str = "pushover";

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

/// Projette une ligne stockée en DTO, en **redactant** tout secret de la configuration (le mot de passe
/// SMTP d'un canal e-mail n'est jamais renvoyé au client — REQ-NOT-003 « sans exposer les identifiants »).
#[requirement(REQ-NOT-005)]
#[requirement(REQ-NOT-003)]
#[requirement(REQ-NOT-004)]
fn row_to_dto(row: NotificationChannelRow) -> NotificationChannelDto {
    let mut config = row.config;
    // Secrets par type de canal : mot de passe SMTP (email), jeton de bot (telegram), jeton
    // d'application (gotify/pushover), clé utilisateur (pushover).
    for secret in ["password", "bot_token", "token", "user_key"] {
        if let Some(value) = config.get_mut(secret) {
            *value = serde_json::Value::String("<redacted>".to_string());
        }
    }
    NotificationChannelDto {
        id: row.id.to_string(),
        kind: row.kind,
        config,
        enabled: row.enabled,
    }
}

/// Type de canal e-mail (REQ-NOT-003).
const KIND_EMAIL: &str = "email";

/// Valide+normalise la configuration d'un webhook (REQ-NOT-005) : `url` `http(s)` **publique**
/// (anti-SSRF à l'enregistrement). `Err` = message de champ fautif (422).
#[requirement(REQ-NOT-005)]
fn validate_webhook_config(config: &serde_json::Value) -> Result<serde_json::Value, &'static str> {
    let url = config
        .get("url")
        .and_then(|v| v.as_str())
        .ok_or("config.url: requis (chaîne)")?;
    if !webhook_url_is_safe(url) {
        return Err("config.url: adresse non autorisée (interne/bouclage) ou URL invalide");
    }
    Ok(serde_json::json!({ "url": url }))
}

/// Valide+normalise la configuration SMTP d'un canal e-mail (REQ-NOT-003). On ne persiste que les
/// clés connues (jamais le corps brut). `Err` = message de champ fautif (422).
#[requirement(REQ-NOT-003)]
fn validate_email_config(config: &serde_json::Value) -> Result<serde_json::Value, &'static str> {
    let host = config
        .get("host")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or("config.host: requis (chaîne non vide)")?;
    let port = config
        .get("port")
        .and_then(serde_json::Value::as_u64)
        .filter(|p| (1..=65535).contains(p))
        .ok_or("config.port: requis (entier 1..=65535)")?;
    let username = config
        .get("username")
        .and_then(|v| v.as_str())
        .ok_or("config.username: requis (chaîne)")?;
    let password = config
        .get("password")
        .and_then(|v| v.as_str())
        .ok_or("config.password: requis (chaîne)")?;
    let from = config
        .get("from")
        .and_then(|v| v.as_str())
        .ok_or("config.from: requis (adresse e-mail)")?;
    // L'adresse d'expéditeur doit être analysable (sinon l'envoi échouerait systématiquement).
    if !wallos_notifier::is_valid_email_address(from) {
        return Err("config.from: adresse e-mail invalide");
    }
    // STARTTLS par défaut (587) ; TLS implicite si explicitement false.
    let starttls = config
        .get("starttls")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true);
    Ok(serde_json::json!({
        "host": host,
        "port": port,
        "username": username,
        "password": password,
        "from": from,
        "starttls": starttls,
    }))
}

/// Extrait une chaîne **non vide** de la configuration (aide aux validateurs NOT-004) :
/// clé absente, mal typée, vide ou blanche → `None`. La valeur est **trimmée** avant stockage
/// (revue NOT-004 F7 : `" "` ne doit pas passer pour un jeton).
#[requirement(REQ-NOT-004)]
fn non_empty_string<'a>(config: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    config
        .get(key)
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

/// Valide+normalise la configuration d'un canal Telegram (REQ-NOT-004) : jeton de bot et
/// identifiant de conversation, tous deux requis (oracle legacy). L'API cible est fixe
/// (`api.telegram.org`) — aucune URL utilisateur, donc pas de garde SSRF nécessaire.
#[requirement(REQ-NOT-004)]
fn validate_telegram_config(config: &serde_json::Value) -> Result<serde_json::Value, &'static str> {
    let bot_token = non_empty_string(config, "bot_token")
        .ok_or("config.bot_token: requis (chaîne non vide)")?;
    // Le jeton est interpolé dans le chemin de l'URL de l'API Bot : format strict exigé
    // (revue NOT-004 F2).
    if !wallos_notifier::telegram_bot_token_is_valid(bot_token) {
        return Err("config.bot_token: format de jeton de bot invalide (attendu <id>:<jeton>)");
    }
    let chat_id =
        non_empty_string(config, "chat_id").ok_or("config.chat_id: requis (chaîne non vide)")?;
    Ok(serde_json::json!({ "bot_token": bot_token, "chat_id": chat_id }))
}

/// Valide+normalise la configuration d'un canal Discord (REQ-NOT-004) : URL de webhook `http(s)`
/// **publique** (anti-SSRF, même garde que le webhook générique) ; nom et avatar du bot optionnels
/// (oracle legacy).
#[requirement(REQ-NOT-004)]
fn validate_discord_config(config: &serde_json::Value) -> Result<serde_json::Value, &'static str> {
    let url = config
        .get("url")
        .and_then(|v| v.as_str())
        .ok_or("config.url: requis (chaîne)")?;
    if !webhook_url_is_safe(url) {
        return Err("config.url: adresse non autorisée (interne/bouclage) ou URL invalide");
    }
    let mut normalized = serde_json::json!({ "url": url });
    if let Some(username) = non_empty_string(config, "username") {
        normalized["username"] = serde_json::Value::String(username.to_string());
    }
    if let Some(avatar_url) = non_empty_string(config, "avatar_url") {
        // Transmise telle quelle à Discord : au moins une URL http(s) analysable
        // (revue NOT-004 F8 — pas de `javascript:`/`file:` relayé).
        if !wallos_notifier::is_http_url(avatar_url) {
            return Err("config.avatar_url: URL http(s) invalide");
        }
        normalized["avatar_url"] = serde_json::Value::String(avatar_url.to_string());
    }
    Ok(normalized)
}

/// Valide+normalise la configuration d'un canal Gotify (REQ-NOT-004) : URL du serveur `http(s)`
/// **publique** (anti-SSRF — un serveur Gotify est fourni par l'utilisateur, même surface qu'un
/// webhook) et jeton d'application requis. L'option legacy `ignore_ssl` n'est **pas** reprise
/// (divergence de sécurité assumée, voir ADR) ; comme toute clé inconnue, elle est jetée.
#[requirement(REQ-NOT-004)]
fn validate_gotify_config(config: &serde_json::Value) -> Result<serde_json::Value, &'static str> {
    let url = config
        .get("url")
        .and_then(|v| v.as_str())
        .ok_or("config.url: requis (chaîne)")?;
    if !webhook_url_is_safe(url) {
        return Err("config.url: adresse non autorisée (interne/bouclage) ou URL invalide");
    }
    let token =
        non_empty_string(config, "token").ok_or("config.token: requis (chaîne non vide)")?;
    Ok(serde_json::json!({ "url": url, "token": token }))
}

/// Valide+normalise la configuration d'un canal Pushover (REQ-NOT-004) : clé utilisateur et jeton
/// d'application requis (oracle legacy). L'API cible est fixe (`api.pushover.net`) — pas d'URL
/// utilisateur, donc pas de garde SSRF nécessaire.
#[requirement(REQ-NOT-004)]
fn validate_pushover_config(config: &serde_json::Value) -> Result<serde_json::Value, &'static str> {
    let user_key =
        non_empty_string(config, "user_key").ok_or("config.user_key: requis (chaîne non vide)")?;
    let token =
        non_empty_string(config, "token").ok_or("config.token: requis (chaîne non vide)")?;
    Ok(serde_json::json!({ "user_key": user_key, "token": token }))
}

/// Crée un canal de notification dans le foyer de l'appelant (REQ-NOT-005 webhook, REQ-NOT-003
/// e-mail, REQ-NOT-004 messageries Telegram/Discord/Gotify/Pushover).
///
/// - **webhook** : `config.url` doit être une URL `http(s)` **publique** ; les adresses internes, de
///   bouclage, privées ou `localhost` sont refusées (422) pour prévenir la SSRF (NOT-005 critère #2).
/// - **email** : `config` doit porter `host`, `port`, `username`, `password`, `from` (adresse valide) ;
///   `starttls` optionnel (défaut vrai).
/// - **telegram** : `config.bot_token` et `config.chat_id` requis (API Bot Telegram).
/// - **discord** : `config.url` (webhook Discord, même garde SSRF) ; `username` et `avatar_url`
///   optionnels.
/// - **gotify** : `config.url` (serveur Gotify, même garde SSRF) et `config.token` requis.
/// - **pushover** : `config.user_key` et `config.token` requis.
#[utoipa::path(
    post,
    path = "/notifications/channels",
    operation_id = "createNotificationChannel",
    extensions(("x-requirements" = json!(["REQ-NOT-005", "REQ-NOT-003", "REQ-NOT-004"]))),
    request_body = CreateNotificationChannelRequest,
    responses(
        (status = 201, description = "Canal créé", body = NotificationChannelDto, content_type = "application/json"),
        (status = 401, description = "Non authentifié", body = wallos_proto::Problem, content_type = "application/problem+json"),
        (status = 422, description = "Type non supporté, configuration invalide, ou URL refusée (SSRF)", body = wallos_proto::Problem, content_type = "application/problem+json"),
        (status = 500, description = "Erreur interne", body = wallos_proto::Problem, content_type = "application/problem+json")
    )
)]
#[requirement(REQ-NOT-005)]
#[requirement(REQ-NOT-003)]
#[requirement(REQ-NOT-004)]
pub async fn create_notification_channel(
    AuthActor(actor): AuthActor,
    State(db): State<Db>,
    Json(req): Json<CreateNotificationChannelRequest>,
) -> Response {
    // Normalise la configuration selon le type ; tout autre type est refusé explicitement.
    let validated = match req.kind.as_str() {
        KIND_WEBHOOK => validate_webhook_config(&req.config).map(|c| (KIND_WEBHOOK, c)),
        KIND_EMAIL => validate_email_config(&req.config).map(|c| (KIND_EMAIL, c)),
        KIND_TELEGRAM => validate_telegram_config(&req.config).map(|c| (KIND_TELEGRAM, c)),
        KIND_DISCORD => validate_discord_config(&req.config).map(|c| (KIND_DISCORD, c)),
        KIND_GOTIFY => validate_gotify_config(&req.config).map(|c| (KIND_GOTIFY, c)),
        KIND_PUSHOVER => validate_pushover_config(&req.config).map(|c| (KIND_PUSHOVER, c)),
        _ => Err(
            "kind: type de canal non supporté (webhook, email, telegram, discord, gotify ou pushover attendu)",
        ),
    };
    let (kind, config) = match validated {
        Ok(pair) => pair,
        Err(detail) => return invalid(detail),
    };
    let enabled = req.enabled.unwrap_or(true);
    match NotificationChannelRepository::new(db.pool())
        .create(&actor, kind, &config, enabled)
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

/// Notification **factice** pour l'envoi de test (REQ-NOT-006) : un abonnement fictif échéant dans
/// 5 jours (esprit de la « fake subscription » du legacy). Passe par le même chemin d'envoi et les
/// mêmes gabarits localisés que le cron — tester, c'est exercer exactement ce qui sera émis.
#[requirement(REQ-NOT-006)]
fn test_notification(as_of: chrono::NaiveDate) -> ReminderNotification {
    let due = as_of
        .checked_add_days(chrono::Days::new(5))
        .unwrap_or(as_of);
    ReminderNotification::new(
        as_of.to_string(),
        vec![ReminderItem {
            subscription_id: Uuid::nil().to_string(),
            name: "Test subscription".to_string(),
            due_date: due.to_string(),
            // Dérivé de la date réelle (revue NOT-006 F7 : cohérent même si l'addition sature).
            days_until: (due - as_of).num_days(),
            kind: "payment".to_string(),
        }],
    )
}

/// Envois de test autorisés par foyer sur la fenêtre glissante (revue NOT-006 F1) : l'endpoint
/// déclenche une requête sortante vers la cible du canal — sans limite, il servirait
/// d'amplificateur de spam et d'oracle de connectivité.
const TEST_RATELIMIT_MAX: i64 = 5;
/// Fenêtre de la limitation des envois de test (secondes).
const TEST_RATELIMIT_WINDOW_SECS: i64 = 300;

/// `429` avec `Retry-After` (secondes) — même forme que la limitation d'authentification
/// (REQ-AUT-008), appliquée aux envois de test (REQ-NOT-006).
#[requirement(REQ-NOT-006)]
fn too_many_tests(retry_after: i64) -> Response {
    let mut response = problem_response(
        StatusCode::TOO_MANY_REQUESTS,
        problem(429, "about:blank", "Too Many Requests"),
    );
    response.headers_mut().insert(
        axum::http::header::RETRY_AFTER,
        axum::http::HeaderValue::from(retry_after),
    );
    response
}

/// Envoie un message de **test** sur un canal enregistré du foyer (REQ-NOT-006) et renvoie un
/// diagnostic exploitable : `sent`, ou un code d'échec stable (`http-status` + code, `timeout`,
/// `connection-failed`, `smtp-failed`, `send-failed`) — jamais le texte brut de l'erreur (il peut
/// refléter l'URL cible, donc un jeton). Un canal **désactivé** reste testable : le test sert
/// précisément à valider une configuration avant de l'activer. Limité à 5 envois de test par
/// foyer et par 5 minutes (429 + `Retry-After` au-delà).
#[utoipa::path(
    post,
    path = "/notifications/channels/{id}/test",
    operation_id = "testNotificationChannel",
    params(("id" = String, Path, description = "Identifiant (UUID) du canal")),
    extensions(("x-requirements" = json!(["REQ-NOT-006"]))),
    responses(
        (status = 200, description = "Test exécuté (voir `ok` et `code`)", body = TestNotificationChannelResponse, content_type = "application/json"),
        (status = 401, description = "Non authentifié", body = wallos_proto::Problem, content_type = "application/problem+json"),
        (status = 404, description = "Canal inconnu ou hors du foyer", body = wallos_proto::Problem, content_type = "application/problem+json"),
        (status = 422, description = "Configuration stockée illisible pour ce type de canal", body = wallos_proto::Problem, content_type = "application/problem+json"),
        (status = 429, description = "Trop d'envois de test (Retry-After en secondes)", body = wallos_proto::Problem, content_type = "application/problem+json"),
        (status = 500, description = "Erreur interne", body = wallos_proto::Problem, content_type = "application/problem+json")
    )
)]
#[requirement(REQ-NOT-006)]
pub async fn test_notification_channel(
    AuthActor(actor): AuthActor,
    State(db): State<Db>,
    Path(id): Path<String>,
) -> Response {
    let Ok(channel_id) = Uuid::parse_str(&id) else {
        return channel_not_found();
    };
    let repo = NotificationChannelRepository::new(db.pool());
    let row = match repo.get(&actor, channel_id).await {
        Ok(Some(row)) => row,
        Ok(None) => return channel_not_found(),
        Err(_) => return internal_error(),
    };
    // Contact du titulaire : destinataire du canal e-mail, langue des messages (repli anglais).
    let contact = match repo.owner_contact(row.household_id).await {
        Ok(contact) => contact,
        Err(_) => return internal_error(),
    };
    let Some(channel) = crate::reminders::channel_from_row(&row, contact.as_ref()) else {
        // Config stockée illisible pour ce type (ne devrait pas arriver : validée à la création).
        return invalid("config: configuration stockée illisible pour ce canal");
    };
    // Limitation de taux par foyer (revue F1) : fenêtre glissante persistante. Une tentative
    // refusée n'est pas journalisée ; une tentative acceptée l'est AVANT l'envoi (une instance
    // concurrente ne peut pas dépasser la limite pendant qu'un envoi est en cours).
    let now = chrono::Utc::now();
    let window = chrono::Duration::seconds(TEST_RATELIMIT_WINDOW_SECS);
    match repo
        .count_and_earliest_test_attempts(row.household_id, now - window)
        .await
    {
        Ok((count, earliest)) if count >= TEST_RATELIMIT_MAX => {
            let retry_after = earliest
                .map(|e| (e + window - now).num_seconds().max(1))
                .unwrap_or(TEST_RATELIMIT_WINDOW_SECS);
            return too_many_tests(retry_after);
        }
        Ok(_) => {}
        Err(_) => return internal_error(),
    }
    if repo.record_test_attempt(row.household_id).await.is_err() {
        return internal_error();
    }
    let notification = test_notification(now.date_naive());
    let response = match channel.send(&notification).await {
        Ok(()) => TestNotificationChannelResponse {
            ok: true,
            code: "sent".to_string(),
            http_status: None,
        },
        Err(err) => {
            let (code, http_status) = diagnose_send_error(&err);
            TestNotificationChannelResponse {
                ok: false,
                code: code.to_string(),
                http_status,
            }
        }
    };
    Json(response).into_response()
}
