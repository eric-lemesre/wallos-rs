//! Séries statistiques historiques (REQ-STA-006).
//!
//! Face publique de l'évolution du coût mensuel : le handler résout la devise cible (référence du
//! foyer par défaut, REQ-CUR-001), liste les abonnements **actifs** du foyer (§9), normalise chaque
//! coût mensuel (REQ-STA-001) et le **convertit** dans la devise cible (REQ-CUR-003), puis délègue le
//! **fenêtrage temporel** au domaine pur (`core::monthly_cost_evolution`). Horloge serveur seulement
//! pour ancrer « le mois courant » ; le calcul lui-même reste pur (REQ-STA-008).

use axum::Json;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use chrono::Utc;
use wallos_core::billing::{BillingCycle, BillingUnit};
use wallos_core::money::{CurrencyCode, Money};
use wallos_core::requirement;
use wallos_core::stats::monthly_cost;
use wallos_core::{CostSpan, RateTable, convert, monthly_cost_evolution};
use wallos_proto::{
    CostEvolutionQuery, CostEvolutionResponse, FieldError, MonthlyCostPointDto, problem,
};
use wallos_storage::{
    Db, ExchangeRateRepository, SettingsRepository, SubscriptionFilter, SubscriptionRepository,
    SubscriptionRow,
};

use crate::auth::AuthActor;
use crate::exchange::load_rate_table;
use crate::problem_response;

/// Nombre de mois par défaut de la série (une année glissante).
const DEFAULT_MONTHS: u32 = 12;
/// Borne haute du nombre de mois (garde-fou : 5 ans). Au-delà → 422.
const MAX_MONTHS: u32 = 60;

/// `422` identifiant le champ fautif (RFC 9457 `detail`).
#[requirement(REQ-STA-006)]
fn field_error(err: &FieldError) -> Response {
    problem_response(
        StatusCode::UNPROCESSABLE_ENTITY,
        problem(422, "about:blank", "Unprocessable Entity")
            .with_detail(format!("{}: {}", err.field, err.message)),
    )
}

/// `500` générique (défaut interne non divulgué).
#[requirement(REQ-STA-006)]
fn internal_error() -> Response {
    problem_response(
        StatusCode::INTERNAL_SERVER_ERROR,
        problem(500, "about:blank", "Internal Server Error"),
    )
}

/// Coût mensuel normalisé (REQ-STA-001) d'une ligne, **converti dans la devise cible** (REQ-CUR-003),
/// en **précision pleine** (aucun arrondi ici — l'arrondi d'affichage n'intervient qu'une fois, sur la
/// **somme mensuelle**, revue kimi STA-006 F2 ; conforme à REQ-CUR-004).
///
/// `None` si la devise, le cycle ou le montant sont illisibles, ou si aucun taux n'est connu pour la
/// paire — l'abonnement est alors **exclu de la série** (jamais assimilé à un coût nul, revue STA-001 F2),
/// exactement comme les agrégats multi-devises excluent un montant non convertible ; l'appelant signale
/// cette partialité via `complete = false` (revue kimi STA-006 F3).
#[requirement(REQ-STA-006)]
fn monthly_in_target(
    row: &SubscriptionRow,
    target: CurrencyCode,
    table: &RateTable,
) -> Option<rust_decimal::Decimal> {
    let currency = CurrencyCode::new(&row.currency).ok()?;
    let price = Money::new(row.amount, currency).ok()?;
    let unit = BillingUnit::parse(&row.cycle_unit).ok()?;
    let interval = u32::try_from(row.cycle_interval).ok()?;
    let cycle = BillingCycle::from_parts(unit, interval).ok()?;
    let monthly = monthly_cost(price, cycle);
    convert(&monthly, target, table).map(|m| m.amount())
}

/// Série d'évolution du coût mensuel sur `months` mois (REQ-STA-006), du plus ancien au plus récent.
///
/// Chaque point somme les coûts mensuels des abonnements **actifs à ce mois-là** (fenêtre `[first_payment,
/// end_date]`), convertis dans la devise cible. Les abonnements **désactivés** (REQ-SUB-008) sont exclus
/// de toute la série ; les abonnements **terminés** (REQ-SUB-009) ne contribuent qu'aux mois antérieurs à
/// leur date de fin (portés par la fenêtre du domaine). La série reflète ainsi l'historique, pas l'état
/// courant projeté.
#[utoipa::path(
    get,
    path = "/statistics/cost-evolution",
    operation_id = "getCostEvolution",
    extensions(("x-requirements" = json!(["REQ-STA-006"]))),
    params(CostEvolutionQuery),
    responses(
        (status = 200, description = "Série d'évolution du coût mensuel", body = CostEvolutionResponse, content_type = "application/json"),
        (status = 401, description = "Non authentifié", body = wallos_proto::Problem, content_type = "application/problem+json"),
        (status = 422, description = "Paramètres invalides", body = wallos_proto::Problem, content_type = "application/problem+json"),
        (status = 500, description = "Erreur interne", body = wallos_proto::Problem, content_type = "application/problem+json")
    )
)]
#[requirement(REQ-STA-006)]
pub async fn get_cost_evolution(
    AuthActor(actor): AuthActor,
    State(db): State<Db>,
    Query(q): Query<CostEvolutionQuery>,
) -> Response {
    // Fenêtre bornée : au moins un mois, au plus MAX_MONTHS (garde-fou).
    let months = q.months.unwrap_or(DEFAULT_MONTHS);
    if months == 0 || months > MAX_MONTHS {
        return field_error(&FieldError::new("months", "hors bornes"));
    }

    // Devise cible : override explicite, sinon devise de référence du foyer (REQ-CUR-001).
    let target_code = match q.currency.filter(|s| !s.is_empty()) {
        Some(explicit) => explicit,
        None => match SettingsRepository::new(db.pool())
            .reference_currency(&actor)
            .await
        {
            Ok(code) => code,
            Err(_) => return internal_error(),
        },
    };
    let Ok(target) = CurrencyCode::new(&target_code) else {
        return field_error(&FieldError::new("currency", "devise hors référentiel"));
    };

    // Abonnements **actifs** du foyer (§9). Les inactifs (REQ-SUB-008) sont exclus par le filtre ; la
    // fenêtre temporelle (`end_date`) borne les terminés (REQ-SUB-009) mois par mois.
    let filter = SubscriptionFilter {
        active: Some(true),
        ..SubscriptionFilter::default()
    };
    let Ok(rows) = SubscriptionRepository::new(db.pool())
        .list(&actor, &filter)
        .await
    else {
        return internal_error();
    };

    // Table des taux : chargée dès qu'un abonnement doit être converti. Aucune ligne → série de zéros
    // sans aller-retour base.
    let table = if rows.is_empty() {
        RateTable::new(Vec::new())
    } else {
        match load_rate_table(&ExchangeRateRepository::new(db.pool())).await {
            Ok(t) => t,
            Err(_) => return internal_error(),
        }
    };

    // Projection en fenêtres d'activité + coût mensuel converti ; un abonnement non convertible est exclu.
    let spans: Vec<CostSpan> = rows
        .iter()
        .filter_map(|row| {
            monthly_in_target(row, target, &table).map(|monthly| CostSpan {
                start: row.first_payment,
                end: row.end_date,
                monthly,
            })
        })
        .collect();
    // Partialité (REQ-CUR-003) : au moins un abonnement actif a été exclu (donnée illisible ou taux
    // manquant). La série est alors incomplète — signalé au client, jamais présenté comme un total nul
    // silencieux (revue kimi STA-006 F3, cohérent avec `ConvertedTotalResponse.complete`).
    let complete = spans.len() == rows.len();

    let today = Utc::now().date_naive();
    let points = monthly_cost_evolution(&spans, today, months);

    // Les coûts par abonnement sont sommés en **précision pleine** (F2) ; on n'arrondit qu'ici, **une
    // seule fois**, la somme mensuelle, puis on la **formate au nombre de décimales de la devise cible**
    // (REQ-CUR-005/007). Le formatage à précision fixe garantit un axe cohérent au sein de la série
    // (p. ex. « 0.00 »/« 42.50 » pour EUR, « 0 »/« 4200 » pour JPY à 0 décimale). Le total étant toujours
    // ≥ 0, `Money::new` ne peut échouer.
    let decimals = wallos_core::currencies::find(target.as_str()).map_or(2, |c| c.decimals);
    let point_dtos: Vec<MonthlyCostPointDto> = points
        .iter()
        .map(|p| {
            let rounded =
                Money::new(p.total, target).map_or(p.total, |m| m.round_to(decimals).amount());
            MonthlyCostPointDto {
                month: p.month.format("%Y-%m").to_string(),
                total: format!("{rounded:.*}", decimals as usize),
            }
        })
        .collect();
    // `points` a exactement `months >= 1` éléments : `first`/`last` existent toujours.
    let from = point_dtos
        .first()
        .map_or_else(String::new, |p| p.month.clone());
    let to = point_dtos
        .last()
        .map_or_else(String::new, |p| p.month.clone());

    Json(CostEvolutionResponse {
        currency: target.as_str().to_string(),
        from,
        to,
        complete,
        points: point_dtos,
    })
    .into_response()
}
