//! Création d'abonnements (REQ-SUB-002).
//!
//! Intègre le modèle (SUB-001), le cycle (SUB-003) et le calcul d'échéance (SUB-012/013) : à la
//! création, l'abonnement est rattaché au **foyer** de l'appelant (§9) et sa **prochaine échéance
//! est calculée immédiatement** (dérivée, `next_due`). Validation **par champ** (critère #2).

use axum::Json;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use chrono::{NaiveDate, Utc};
use uuid::Uuid;
use wallos_core::billing::{BillingCycle, BillingUnit};
use wallos_core::money::{CurrencyCode, Money};
use wallos_core::requirement;
use wallos_core::{aggregate_converted, next_due};
use wallos_proto::{
    BillingCycleDto, ConvertedTotalResponse, CreateSubscriptionRequest, FieldError,
    SubscriptionDto, SubscriptionListQuery, SubscriptionListResponse, problem,
};
use wallos_storage::{
    Db, ExchangeRateRepository, SubscriptionFilter, SubscriptionRepository, SubscriptionRow,
};

use crate::auth::AuthActor;
use crate::exchange::load_rate_table;
use crate::problem_response;

/// `422` identifiant le champ fautif (RFC 9457 `detail`), REQ-SUB-002 critère #2.
#[requirement(REQ-SUB-002)]
fn field_error(err: &FieldError) -> Response {
    problem_response(
        StatusCode::UNPROCESSABLE_ENTITY,
        problem(422, "about:blank", "Unprocessable Entity")
            .with_detail(format!("{}: {}", err.field, err.message)),
    )
}

/// Crée un abonnement dans le foyer de l'appelant ; renvoie l'abonnement et sa prochaine échéance.
#[utoipa::path(
    post,
    path = "/subscriptions",
    operation_id = "createSubscription",
    extensions(("x-requirements" = json!(["REQ-SUB-002"]))),
    request_body = CreateSubscriptionRequest,
    responses(
        (status = 201, description = "Abonnement créé (avec prochaine échéance)", body = SubscriptionDto, content_type = "application/json"),
        (status = 401, description = "Non authentifié", body = wallos_proto::Problem, content_type = "application/problem+json"),
        (status = 422, description = "Validation par champ", body = wallos_proto::Problem, content_type = "application/problem+json")
    )
)]
#[requirement(REQ-SUB-002)]
pub async fn create_subscription(
    AuthActor(actor): AuthActor,
    State(db): State<Db>,
    Json(req): Json<CreateSubscriptionRequest>,
) -> Response {
    let subscription = match req.into_core(Uuid::new_v4()) {
        Ok(sub) => sub,
        Err(err) => return field_error(&err),
    };

    // Prochaine échéance : première occurrence à partir d'aujourd'hui (inclus). Horloge serveur —
    // le domaine reste pur (l'instant est injecté ici).
    let today = Utc::now().date_naive();
    let reference = today.pred_opt().unwrap_or(today);
    let Some(next) = next_due(
        subscription.first_payment(),
        subscription.cycle(),
        reference,
    ) else {
        return field_error(&FieldError::new("first_payment", "échéance hors plage"));
    };

    match SubscriptionRepository::new(db.pool())
        .create(&actor, &subscription)
        .await
    {
        Ok(()) => (
            StatusCode::CREATED,
            Json(SubscriptionDto::from_core_with_next_payment(
                &subscription,
                next,
            )),
        )
            .into_response(),
        _ => problem_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            problem(500, "about:blank", "Internal Server Error"),
        ),
    }
}

/// Reconstruit le cycle stocké (jamais silencieusement altéré : un intervalle non stockable est écarté).
#[requirement(REQ-SUB-006)]
fn row_cycle(row: &SubscriptionRow) -> Option<BillingCycle> {
    let unit = BillingUnit::parse(&row.cycle_unit).ok()?;
    let interval = u32::try_from(row.cycle_interval).ok()?;
    BillingCycle::from_parts(unit, interval).ok()
}

/// Projette une ligne stockée vers le DTO, en calculant la prochaine échéance (vue par défaut).
#[requirement(REQ-SUB-006)]
fn row_to_dto(row: SubscriptionRow, today: NaiveDate) -> SubscriptionDto {
    let reference = today.pred_opt().unwrap_or(today);
    let next_payment = row_cycle(&row)
        .and_then(|cycle| next_due(row.first_payment, cycle, reference))
        .map(|d| d.to_string());
    SubscriptionDto {
        id: row.id.to_string(),
        name: row.name,
        amount: row.amount.to_string(),
        currency: row.currency,
        cycle: BillingCycleDto {
            unit: row.cycle_unit,
            interval: u32::try_from(row.cycle_interval).unwrap_or_default(),
        },
        first_payment: row.first_payment.to_string(),
        category: row.category_id.map(|u| u.to_string()),
        payment_method: row.payment_method_id.map(|u| u.to_string()),
        payer: row.payer_id.map(|u| u.to_string()),
        logo: row.logo,
        url: row.url,
        notes: row.notes,
        active: row.active,
        next_payment,
    }
}

/// Parse un identifiant de filtre optionnel (UUID) ; `Err` nomme le champ fautif (→ 422).
#[requirement(REQ-SUB-006)]
fn filter_id(raw: Option<String>, field: &'static str) -> Result<Option<Uuid>, FieldError> {
    raw.filter(|s| !s.is_empty())
        .map(|s| Uuid::parse_str(&s).map_err(|_| FieldError::new(field, "identifiant invalide")))
        .transpose()
}

/// Liste les abonnements du foyer, filtrés (conjonctifs), avec le total agrégé du sous-ensemble actif.
#[utoipa::path(
    get,
    path = "/subscriptions",
    operation_id = "listSubscriptions",
    extensions(("x-requirements" = json!(["REQ-SUB-006"]))),
    params(SubscriptionListQuery),
    responses(
        (status = 200, description = "Abonnements filtrés et total agrégé", body = SubscriptionListResponse, content_type = "application/json"),
        (status = 401, description = "Non authentifié", body = wallos_proto::Problem, content_type = "application/problem+json"),
        (status = 422, description = "Filtre invalide", body = wallos_proto::Problem, content_type = "application/problem+json")
    )
)]
#[requirement(REQ-SUB-006)]
pub async fn list_subscriptions(
    AuthActor(actor): AuthActor,
    State(db): State<Db>,
    Query(q): Query<SubscriptionListQuery>,
) -> Response {
    let category = match filter_id(q.category, "category") {
        Ok(c) => c,
        Err(err) => return field_error(&err),
    };
    let payer = match filter_id(q.payer, "payer") {
        Ok(p) => p,
        Err(err) => return field_error(&err),
    };
    // Devise cible du total : défaut EUR en attendant la devise de référence (REQ-CUR-001).
    let target_code = q
        .currency
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "EUR".to_string());
    let Ok(target) = CurrencyCode::new(&target_code) else {
        return field_error(&FieldError::new("currency", "devise hors référentiel"));
    };

    let filter = SubscriptionFilter {
        category,
        payer,
        active: q.active,
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

    // Total = somme convertie des abonnements **actifs** du sous-ensemble filtré (REQ-SUB-008 : un
    // abonnement désactivé figure dans la liste mais est exclu de l'agrégat).
    let amounts: Vec<Money> = rows
        .iter()
        .filter(|r| r.active)
        .filter_map(|r| {
            CurrencyCode::new(&r.currency)
                .ok()
                .and_then(|c| Money::new(r.amount, c).ok())
        })
        .collect();
    let table = match load_rate_table(&ExchangeRateRepository::new(db.pool())).await {
        Ok(t) => t,
        Err(_) => {
            return problem_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                problem(500, "about:blank", "Internal Server Error"),
            );
        }
    };
    let agg = aggregate_converted(&amounts, target, &table);

    let today = Utc::now().date_naive();
    let subscriptions: Vec<SubscriptionDto> =
        rows.into_iter().map(|r| row_to_dto(r, today)).collect();
    Json(SubscriptionListResponse {
        subscriptions,
        total: ConvertedTotalResponse {
            total: agg.total().amount().to_string(),
            currency: target.as_str().to_string(),
            converted: agg.converted() as u32,
            excluded: agg.excluded() as u32,
            complete: agg.is_complete(),
            as_of: agg.as_of().map(|d| d.to_string()),
        },
    })
    .into_response()
}
