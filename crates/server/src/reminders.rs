//! Rappels avant échéance (REQ-NOT-001).
//!
//! Trois faces : le **réglage** du délai de rappel du compte (jours avant l'échéance), la **vue** des
//! rappels dus aujourd'hui pour le foyer (§9), et le **cron** `POST /internal/run-reminders` — déclenché
//! périodiquement par un ordonnanceur externe (choix d'architecture, ADR 0040) — qui balaie **tous** les
//! foyers, émet les rappels dus et les **journalise** pour ne pas ré-émettre le même jour.
//!
//! Règle de déclenchement **exacte** (oracle Wallos, gelé) déléguée au domaine pur `core::due_reminders`
//! (fenêtre injectée : `as_of`, aucun accès horloge — REQ-STA-008). Le regroupement par compte (message
//! unique) est reconstitué ici (critère d'acceptation #2).

use std::collections::BTreeMap;

use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use chrono::{NaiveDate, Utc};
use serde::Deserialize;
use uuid::Uuid;
use wallos_core::billing::{BillingCycle, BillingUnit};
use wallos_core::requirement;
use wallos_core::{DueReminder, ReminderCandidate, due_reminders, next_due};
use wallos_notifier::{
    Channel, Discord, Email, EmailConfig, Gotify, Pushover, ReminderItem, ReminderNotification,
    Telegram, Webhook,
};
use wallos_proto::{
    ReminderDto, ReminderSettingResponse, RemindersResponse, RunRemindersResponse,
    SetReminderSettingRequest, problem,
};
use wallos_storage::{
    Db, NotificationChannelRepository, NotificationChannelRow, ReminderRepository, ReminderScanRow,
};

use crate::auth::AuthActor;
use crate::{CronToken, problem_response};

/// Type de rappel : échéance de paiement (REQ-NOT-001).
const KIND_PAYMENT: &str = "payment";
/// Type de rappel : fin de période d'essai gratuit (REQ-SUB-010), **distinct** du rappel de paiement.
const KIND_TRIAL: &str = "trial_ending";

/// `500` générique.
#[requirement(REQ-NOT-001)]
fn internal_error() -> Response {
    problem_response(
        StatusCode::INTERNAL_SERVER_ERROR,
        problem(500, "about:blank", "Internal Server Error"),
    )
}

/// Prochaine échéance d'une ligne de balayage, si l'abonnement est encore actif à `as_of` (non terminé).
/// `None` si le cycle est illisible ou l'abonnement terminé.
#[requirement(REQ-NOT-001)]
fn candidate_of(row: &ReminderScanRow, as_of: NaiveDate) -> Option<ReminderCandidate> {
    let unit = BillingUnit::parse(&row.cycle_unit).ok()?;
    let interval = u32::try_from(row.cycle_interval).ok()?;
    let cycle = BillingCycle::from_parts(unit, interval).ok()?;
    // Prochaine occurrence à partir de la veille (inclut aujourd'hui), comme l'échéancier.
    let reference = as_of.pred_opt().unwrap_or(as_of);
    let next = next_due(row.first_payment, cycle, reference)?;
    // Un abonnement terminé (échéance au-delà de la date de fin) ne déclenche plus (REQ-SUB-009).
    if row.end_date.is_some_and(|end| next > end) {
        return None;
    }
    let lead_days = u32::try_from(row.reminder_lead_days).unwrap_or(1);
    Some(ReminderCandidate {
        subscription_id: row.id,
        next_due: next,
        lead_days,
    })
}

/// Candidat au rappel de **fin d'essai** (REQ-SUB-010) : la date déclencheuse est `trial_end_date`.
/// `None` si l'abonnement n'a pas d'essai ou si l'essai est déjà terminé (`trial_end < as_of`).
#[requirement(REQ-SUB-010)]
fn trial_candidate_of(row: &ReminderScanRow, as_of: NaiveDate) -> Option<ReminderCandidate> {
    let trial_end = row.trial_end_date?;
    if trial_end < as_of {
        return None;
    }
    let lead_days = u32::try_from(row.reminder_lead_days).unwrap_or(1);
    Some(ReminderCandidate {
        subscription_id: row.id,
        next_due: trial_end,
        lead_days,
    })
}

/// Rappels **dus** à `as_of` pour un lot de lignes, avec leur type (`payment` / `trial_ending`).
/// Le rappel de fin d'essai est distinct du rappel d'échéance (REQ-SUB-010, critère #2).
#[requirement(REQ-NOT-001)]
fn due_with_kind(rows: &[&ReminderScanRow], as_of: NaiveDate) -> Vec<(DueReminder, &'static str)> {
    let payment: Vec<ReminderCandidate> =
        rows.iter().filter_map(|r| candidate_of(r, as_of)).collect();
    let mut out: Vec<(DueReminder, &'static str)> = due_reminders(&payment, as_of)
        .into_iter()
        .map(|d| (d, KIND_PAYMENT))
        .collect();
    let trial: Vec<ReminderCandidate> = rows
        .iter()
        .filter_map(|r| trial_candidate_of(r, as_of))
        .collect();
    out.extend(
        due_reminders(&trial, as_of)
            .into_iter()
            .map(|d| (d, KIND_TRIAL)),
    );
    out
}

/// Construit un canal d'envoi ([`Channel`]) à partir d'une ligne stockée (REQ-NOT-005 webhook,
/// REQ-NOT-003 e-mail, REQ-NOT-004 messageries). `contact` = `(email, langue)` du titulaire du
/// foyer : requis pour l'e-mail, la langue localise aussi les messageries (repli anglais sinon).
/// `None` si le type est inconnu, la configuration illisible, ou le contact manquant — ne fait jamais
/// échouer le cron.
#[requirement(REQ-NOT-005)]
#[requirement(REQ-NOT-003)]
#[requirement(REQ-NOT-004)]
fn channel_from_row(
    row: &NotificationChannelRow,
    contact: Option<&(String, String)>,
) -> Option<Channel> {
    let language = contact.map_or("en", |(_, language)| language.as_str());
    let get = |key: &str| row.config.get(key).and_then(|v| v.as_str());
    match row.kind.as_str() {
        "webhook" => Some(Channel::Webhook(Webhook::new(get("url")?))),
        "email" => {
            let (recipient, language) = contact?;
            let config = email_config_from(&row.config)?;
            Some(Channel::Email(Email::new(
                config,
                recipient.clone(),
                language.clone(),
            )))
        }
        "telegram" => {
            let mut telegram = Telegram::new(get("bot_token")?, get("chat_id")?, language);
            // Base d'API repointable en test uniquement (posable par SQL direct, jamais via l'API —
            // la validation à l'enregistrement jette toute clé inconnue).
            if let Some(api_base) = get("api_base") {
                telegram = telegram.with_api_base(api_base);
            }
            Some(Channel::Telegram(telegram))
        }
        "discord" => Some(Channel::Discord(Discord::new(
            get("url")?,
            get("username").map(String::from),
            get("avatar_url").map(String::from),
            language,
        ))),
        "gotify" => Some(Channel::Gotify(Gotify::new(
            get("url")?,
            get("token")?,
            language,
        ))),
        "pushover" => {
            let mut pushover = Pushover::new(get("user_key")?, get("token")?, language);
            if let Some(api_base) = get("api_base") {
                pushover = pushover.with_api_base(api_base);
            }
            Some(Channel::Pushover(pushover))
        }
        _ => None,
    }
}

/// Reconstruit la configuration SMTP depuis la config jsonb stockée (REQ-NOT-003). `None` si une clé
/// requise manque ou est mal typée (canal ignoré silencieusement plutôt que d'échouer le cron).
#[requirement(REQ-NOT-003)]
fn email_config_from(config: &serde_json::Value) -> Option<EmailConfig> {
    Some(EmailConfig {
        host: config.get("host")?.as_str()?.to_string(),
        port: u16::try_from(config.get("port")?.as_u64()?).ok()?,
        username: config.get("username")?.as_str()?.to_string(),
        password: config.get("password")?.as_str()?.to_string(),
        from: config.get("from")?.as_str()?.to_string(),
        starttls: config
            .get("starttls")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true),
    })
}

/// Réglage du délai de rappel du compte (REQ-NOT-001).
#[utoipa::path(
    get,
    path = "/settings/reminder",
    operation_id = "getReminderSetting",
    extensions(("x-requirements" = json!(["REQ-NOT-001"]))),
    responses(
        (status = 200, description = "Délai de rappel", body = ReminderSettingResponse, content_type = "application/json"),
        (status = 401, description = "Non authentifié", body = wallos_proto::Problem, content_type = "application/problem+json"),
        (status = 500, description = "Erreur interne", body = wallos_proto::Problem, content_type = "application/problem+json")
    )
)]
#[requirement(REQ-NOT-001)]
pub async fn get_reminder_setting(AuthActor(actor): AuthActor, State(db): State<Db>) -> Response {
    match ReminderRepository::new(db.pool()).lead_days(&actor).await {
        Ok(lead_days) => Json(ReminderSettingResponse { lead_days }).into_response(),
        Err(_) => internal_error(),
    }
}

/// Met à jour le délai de rappel du compte (REQ-NOT-001).
#[utoipa::path(
    put,
    path = "/settings/reminder",
    operation_id = "setReminderSetting",
    extensions(("x-requirements" = json!(["REQ-NOT-001"]))),
    request_body = SetReminderSettingRequest,
    responses(
        (status = 200, description = "Délai mis à jour", body = ReminderSettingResponse, content_type = "application/json"),
        (status = 401, description = "Non authentifié", body = wallos_proto::Problem, content_type = "application/problem+json"),
        (status = 422, description = "Délai hors bornes", body = wallos_proto::Problem, content_type = "application/problem+json"),
        (status = 500, description = "Erreur interne", body = wallos_proto::Problem, content_type = "application/problem+json")
    )
)]
#[requirement(REQ-NOT-001)]
pub async fn set_reminder_setting(
    AuthActor(actor): AuthActor,
    State(db): State<Db>,
    Json(req): Json<SetReminderSettingRequest>,
) -> Response {
    if !(0..=365).contains(&req.lead_days) {
        return problem_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            problem(422, "about:blank", "Unprocessable Entity")
                .with_detail("lead_days: hors bornes (0..=365)"),
        );
    }
    let repo = ReminderRepository::new(db.pool());
    if repo.set_lead_days(&actor, req.lead_days).await.is_err() {
        return internal_error();
    }
    Json(ReminderSettingResponse {
        lead_days: req.lead_days,
    })
    .into_response()
}

/// Rappels dus aujourd'hui pour le compte (REQ-NOT-001), vue lisible du message groupé.
#[utoipa::path(
    get,
    path = "/reminders",
    operation_id = "getReminders",
    extensions(("x-requirements" = json!(["REQ-NOT-001"]))),
    responses(
        (status = 200, description = "Rappels dus aujourd'hui", body = RemindersResponse, content_type = "application/json"),
        (status = 401, description = "Non authentifié", body = wallos_proto::Problem, content_type = "application/problem+json"),
        (status = 500, description = "Erreur interne", body = wallos_proto::Problem, content_type = "application/problem+json")
    )
)]
#[requirement(REQ-NOT-001)]
pub async fn get_reminders(AuthActor(actor): AuthActor, State(db): State<Db>) -> Response {
    let today = Utc::now().date_naive();
    let rows = match ReminderRepository::new(db.pool())
        .scan_household(&actor)
        .await
    {
        Ok(rows) => rows,
        Err(_) => return internal_error(),
    };
    let refs: Vec<&ReminderScanRow> = rows.iter().collect();
    // Rappels de paiement ET de fin d'essai (distincts, REQ-SUB-010).
    let due = due_with_kind(&refs, today);
    // Retrouve nom + payeur par id pour composer la vue.
    let by_id: BTreeMap<Uuid, &ReminderScanRow> = rows.iter().map(|r| (r.id, r)).collect();
    let reminders = due
        .into_iter()
        .filter_map(|(d, kind)| {
            let row = by_id.get(&d.subscription_id)?;
            Some(ReminderDto {
                kind: kind.to_string(),
                subscription_id: d.subscription_id.to_string(),
                name: row.name.clone(),
                due_date: d.due_date.format("%Y-%m-%d").to_string(),
                days_until: d.days_until,
                payer_id: row.payer_id.map(|p| p.to_string()),
            })
        })
        .collect();
    Json(RemindersResponse {
        as_of: today.format("%Y-%m-%d").to_string(),
        reminders,
    })
    .into_response()
}

/// Paramètres du cron (date de référence injectable pour les tests).
#[derive(Debug, Deserialize)]
pub struct RunRemindersQuery {
    /// Date de référence (`YYYY-MM-DD`) ; défaut : aujourd'hui (horloge serveur).
    #[serde(default)]
    pub as_of: Option<String>,
}

/// Exécute le cron de rappel (REQ-NOT-001) : balaie **tous** les foyers, émet les rappels dus et les
/// journalise (idempotent le même jour). **Endpoint d'opérateur** : authentifié par le secret
/// `X-Cron-Token` (configuré côté serveur, `CRON_TOKEN`) ; désactivé (404) si le secret n'est pas
/// configuré. Déclenché périodiquement par un ordonnanceur externe (ADR 0040).
#[utoipa::path(
    post,
    path = "/internal/run-reminders",
    operation_id = "runReminders",
    extensions(("x-requirements" = json!(["REQ-NOT-001"]))),
    params(("as_of" = Option<String>, Query, description = "Date de référence YYYY-MM-DD (tests)")),
    responses(
        (status = 200, description = "Rappels émis", body = RunRemindersResponse, content_type = "application/json"),
        (status = 401, description = "Secret de cron absent ou invalide", body = wallos_proto::Problem, content_type = "application/problem+json"),
        (status = 404, description = "Cron désactivé (aucun secret configuré)", body = wallos_proto::Problem, content_type = "application/problem+json"),
        (status = 500, description = "Erreur interne", body = wallos_proto::Problem, content_type = "application/problem+json")
    )
)]
#[requirement(REQ-NOT-001)]
pub async fn run_reminders(
    State(db): State<Db>,
    Extension(CronToken(cron_token)): Extension<CronToken>,
    headers: HeaderMap,
    Query(q): Query<RunRemindersQuery>,
) -> Response {
    // Cron désactivé si aucun secret n'est configuré (jamais ouvert par défaut).
    let Some(expected) = cron_token.as_deref() else {
        return problem_response(
            StatusCode::NOT_FOUND,
            problem(404, "about:blank", "Not Found"),
        );
    };
    let presented = headers.get("x-cron-token").and_then(|v| v.to_str().ok());
    if presented != Some(expected) {
        return problem_response(
            StatusCode::UNAUTHORIZED,
            problem(401, "about:blank", "Unauthorized"),
        );
    }

    let as_of = match q.as_of.as_deref().filter(|s| !s.is_empty()) {
        Some(raw) => match NaiveDate::parse_from_str(raw, "%Y-%m-%d") {
            Ok(d) => d,
            Err(_) => {
                return problem_response(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    problem(422, "about:blank", "Unprocessable Entity")
                        .with_detail("as_of: date YYYY-MM-DD attendue"),
                );
            }
        },
        None => Utc::now().date_naive(),
    };

    let repo = ReminderRepository::new(db.pool());
    let rows = match repo.scan_all().await {
        Ok(rows) => rows,
        Err(_) => return internal_error(),
    };

    // Canaux **actifs** par foyer (REQ-NOT-005), chargés une fois pour tout le balayage.
    let channel_repo = NotificationChannelRepository::new(db.pool());
    let channels_by_household = match channel_repo.list_enabled_by_household().await {
        Ok(map) => map,
        Err(_) => return internal_error(),
    };
    // Contact (e-mail, langue) du titulaire de chaque foyer, pour adresser le canal e-mail (REQ-NOT-003).
    let contacts = match channel_repo.owner_contacts().await {
        Ok(map) => map,
        Err(_) => return internal_error(),
    };

    // Regroupe par foyer (message unique par compte, critère #2).
    let mut by_household: BTreeMap<Uuid, Vec<&ReminderScanRow>> = BTreeMap::new();
    for row in &rows {
        by_household.entry(row.household_id).or_default().push(row);
    }

    let mut emitted = 0usize;
    let mut accounts_notified = 0usize;
    for (household_id, household_rows) in by_household {
        let by_id: BTreeMap<Uuid, &ReminderScanRow> =
            household_rows.iter().map(|r| (r.id, *r)).collect();
        // Rappels de paiement (REQ-NOT-001) ET de fin d'essai (REQ-SUB-010), chacun avec son type.
        let due = due_with_kind(&household_rows, as_of);
        let mut account_emitted = 0usize;
        // Rappels **nouvellement émis** ce jour (hors doublons) → charge utile des canaux (REQ-NOT-005).
        let mut fresh: Vec<ReminderItem> = Vec::new();
        for (d, kind) in due {
            // Journalise ; n'émet (compte) que si nouvellement inséré (pas de doublon le même jour, par type).
            match repo
                .record_emitted(household_id, d.subscription_id, d.due_date, kind)
                .await
            {
                Ok(true) => {
                    emitted += 1;
                    account_emitted += 1;
                    if let Some(row) = by_id.get(&d.subscription_id) {
                        fresh.push(ReminderItem {
                            subscription_id: d.subscription_id.to_string(),
                            name: row.name.clone(),
                            due_date: d.due_date.format("%Y-%m-%d").to_string(),
                            days_until: i64::from(d.days_until),
                            kind: kind.to_string(),
                        });
                    }
                }
                Ok(false) => {}
                Err(_) => return internal_error(),
            }
        }
        if account_emitted > 0 {
            accounts_notified += 1;
        }
        // Émission sortante best-effort (REQ-NOT-005) : envoie le lot nouvellement émis aux canaux
        // actifs du foyer. Un échec est journalisé sans interrompre les autres canaux ni le cron
        // (esprit REQ-NOT-003 ; réessai = REQ-NOT-007, différé).
        if !fresh.is_empty()
            && let Some(channels) = channels_by_household.get(&household_id)
        {
            let notification =
                ReminderNotification::new(as_of.format("%Y-%m-%d").to_string(), fresh);
            let contact = contacts.get(&household_id);
            for row in channels {
                let Some(channel) = channel_from_row(row, contact) else {
                    continue;
                };
                // Best-effort : on ne journalise PAS l'erreur brute (elle peut contenir l'URL du canal,
                // potentiellement porteuse d'un secret) — seulement le type de canal et le foyer. Le
                // détail exploitable et redacté relèvera de REQ-NOT-007 (réessai/diagnostic).
                if channel.send(&notification).await.is_err() {
                    tracing::warn!(
                        household_id = %household_id,
                        channel = channel.kind(),
                        "échec d'envoi d'un canal de notification"
                    );
                }
            }
        }
    }

    Json(RunRemindersResponse {
        as_of: as_of.format("%Y-%m-%d").to_string(),
        emitted,
        accounts_notified,
    })
    .into_response()
}
