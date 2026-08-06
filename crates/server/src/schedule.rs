//! Calcul de la prochaine échéance (REQ-SUB-012).
//!
//! Face publique du calcul d'échéance ancré+clampé (ADR 0022). Donnée sans état (pas d'accès base) :
//! le handler valide les entrées et délègue à `core::next_due`. Auth requise (anon → 401).

use axum::Json;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use chrono::{Days, NaiveDate, Utc};
use wallos_core::billing::{BillingCycle, BillingUnit};
use wallos_core::requirement;
use wallos_core::{next_due, occurrences_in_range};
use wallos_proto::{
    NextDueRequest, NextDueResponse, UpcomingPayment, UpcomingPaymentsQuery,
    UpcomingPaymentsResponse, problem,
};
use wallos_storage::{Db, SubscriptionFilter, SubscriptionRepository};

use crate::auth::AuthActor;
use crate::problem_response;

/// Fenêtre maximale de l'échéancier (jours). Garde-fou contre une énumération démesurée ; au-delà,
/// l'entrée est refusée (422).
const MAX_WINDOW_DAYS: u32 = 366;

#[requirement(REQ-SUB-012)]
fn unprocessable() -> Response {
    problem_response(
        StatusCode::UNPROCESSABLE_ENTITY,
        problem(422, "about:blank", "Unprocessable Entity"),
    )
}

/// Calcule la prochaine échéance strictement postérieure à `after`, pour l'ancre et le cycle donnés.
#[utoipa::path(
    post,
    path = "/schedule/next-due",
    operation_id = "computeNextDue",
    extensions(("x-requirements" = json!(["REQ-SUB-012"]))),
    request_body = NextDueRequest,
    responses(
        (status = 200, description = "Prochaine échéance", body = NextDueResponse, content_type = "application/json"),
        (status = 401, description = "Non authentifié", body = wallos_proto::Problem, content_type = "application/problem+json"),
        (status = 422, description = "Entrée invalide", body = wallos_proto::Problem, content_type = "application/problem+json")
    )
)]
#[requirement(REQ-SUB-012)]
pub async fn compute_next_due(
    // Auth requise ; le calcul est sans état (pas de portée foyer).
    AuthActor(_actor): AuthActor,
    Json(req): Json<NextDueRequest>,
) -> Response {
    let (Ok(anchor), Ok(after)) = (
        NaiveDate::parse_from_str(&req.first_payment, "%Y-%m-%d"),
        NaiveDate::parse_from_str(&req.after, "%Y-%m-%d"),
    ) else {
        return unprocessable();
    };
    let Ok(unit) = BillingUnit::parse(&req.cycle.unit) else {
        return unprocessable();
    };
    let Ok(cycle) = BillingCycle::from_parts(unit, req.cycle.interval) else {
        return unprocessable();
    };
    match next_due(anchor, cycle, after) {
        Some(date) => Json(NextDueResponse {
            next_payment: date.to_string(),
        })
        .into_response(),
        // Débordement de plage (dates astronomiques) : entrée hors domaine raisonnable.
        None => unprocessable(),
    }
}

/// Échéancier des prochains paiements sur une fenêtre de `days` jours (REQ-STA-005).
///
/// Énumère, pour chaque abonnement **actif** du foyer (oracle Wallos `inactive = 0`, REQ-SUB-008),
/// **toutes** ses occurrences de paiement dans `[from, from + days]` (bornes incluses), en respectant
/// sa date de fin (REQ-SUB-009) **et** sa fin d'essai gratuit (REQ-SUB-010 : aucun paiement dû pendant
/// l'essai — exclusion transverse REQ-STA-003). `from` par défaut = date du jour (horloge serveur). Le
/// résultat est trié par date, puis nom, puis id (ordre déterministe).
#[utoipa::path(
    get,
    path = "/schedule/upcoming",
    operation_id = "getUpcomingPayments",
    extensions(("x-requirements" = json!(["REQ-STA-005"]))),
    params(UpcomingPaymentsQuery),
    responses(
        (status = 200, description = "Échéancier des prochains paiements", body = UpcomingPaymentsResponse, content_type = "application/json"),
        (status = 401, description = "Non authentifié", body = wallos_proto::Problem, content_type = "application/problem+json"),
        (status = 422, description = "Fenêtre invalide", body = wallos_proto::Problem, content_type = "application/problem+json"),
        (status = 500, description = "Erreur interne", body = wallos_proto::Problem, content_type = "application/problem+json")
    )
)]
#[requirement(REQ-STA-005)]
#[requirement(REQ-STA-003)]
pub async fn get_upcoming_payments(
    AuthActor(actor): AuthActor,
    State(db): State<Db>,
    Query(q): Query<UpcomingPaymentsQuery>,
) -> Response {
    // Fenêtre bornée : ni nulle, ni démesurée (garde-fou anti-énumération).
    if q.days == 0 || q.days > MAX_WINDOW_DAYS {
        return unprocessable();
    }
    let from = match q.from.as_deref() {
        Some(raw) => match NaiveDate::parse_from_str(raw, "%Y-%m-%d") {
            Ok(date) => date,
            Err(_) => return unprocessable(),
        },
        None => Utc::now().date_naive(),
    };
    let Some(to) = from.checked_add_days(Days::new(u64::from(q.days))) else {
        return unprocessable();
    };

    // Abonnements **actifs** du foyer (isolation §9 via `&actor`). Les inactifs (REQ-SUB-008) sont
    // exclus par le filtre ; les abonnements terminés (REQ-SUB-009) ne produisent aucune occurrence
    // grâce à la borne haute `end_date`, et les abonnements en essai gratuit (REQ-SUB-010) aucune
    // occurrence avant `trial_end_date` (borne basse) — exclusion transverse REQ-STA-003.
    let filter = SubscriptionFilter {
        active: Some(true),
        ..SubscriptionFilter::default()
    };
    let Ok(rows) = SubscriptionRepository::new(db.pool())
        .list(&actor, &filter)
        .await
    else {
        return problem_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            problem(500, "about:blank", "Internal Server Error"),
        );
    };

    let mut payments = Vec::new();
    for row in rows {
        // Un cycle illisible (donnée corrompue) est ignoré plutôt que de faire échouer tout l'échéancier.
        let Ok(unit) = BillingUnit::parse(&row.cycle_unit) else {
            continue;
        };
        let Ok(interval) = u32::try_from(row.cycle_interval) else {
            continue;
        };
        let Ok(cycle) = BillingCycle::from_parts(unit, interval) else {
            continue;
        };
        for date in occurrences_in_range(
            row.first_payment,
            cycle,
            from,
            to,
            row.end_date,
            row.trial_end_date,
        ) {
            payments.push(UpcomingPayment {
                date: date.to_string(),
                subscription_id: row.id.to_string(),
                name: row.name.clone(),
                amount: row.amount.to_string(),
                currency: row.currency.clone(),
            });
        }
    }
    // Ordre déterministe (REQ-CAT-005 esprit) : date, puis nom, puis id.
    payments.sort_by(|a, b| {
        a.date
            .cmp(&b.date)
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| a.subscription_id.cmp(&b.subscription_id))
    });

    Json(UpcomingPaymentsResponse {
        from: from.to_string(),
        to: to.to_string(),
        payments,
    })
    .into_response()
}
