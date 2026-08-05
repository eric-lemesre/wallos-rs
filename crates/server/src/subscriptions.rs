//! Création d'abonnements (REQ-SUB-002).
//!
//! Intègre le modèle (SUB-001), le cycle (SUB-003) et le calcul d'échéance (SUB-012/013) : à la
//! création, l'abonnement est rattaché au **foyer** de l'appelant (§9) et sa **prochaine échéance
//! est calculée immédiatement** (dérivée, `next_due`). Validation **par champ** (critère #2).

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use chrono::{NaiveDate, Utc};
use rust_decimal::Decimal;
use uuid::Uuid;
use wallos_core::actor::Actor;
use wallos_core::billing::{BillingCycle, BillingUnit};
use wallos_core::money::{CurrencyCode, Money};
use wallos_core::requirement;
use wallos_core::{RateTable, aggregate_converted, next_due};
use wallos_proto::{
    BillingCycleDto, ConvertedTotalResponse, CreateSubscriptionRequest, FieldError,
    SubscriptionDto, SubscriptionListQuery, SubscriptionListResponse, problem,
};
use wallos_storage::{
    CreateOutcome, Db, ExchangeRateRepository, SettingsRepository, SubscriptionFilter,
    SubscriptionRepository, SubscriptionRow,
};

use crate::auth::AuthActor;
use crate::exchange::load_rate_table;
use crate::idempotency::{self, IdempotencyKey, Outcome};
use crate::problem_response;

/// `500` générique (défaut interne non divulgué).
#[requirement(REQ-SUB-002)]
fn internal_error() -> Response {
    problem_response(
        StatusCode::INTERNAL_SERVER_ERROR,
        problem(500, "about:blank", "Internal Server Error"),
    )
}

/// `409` : l'`id` fourni par le client est **déjà pris** (collision de clé primaire, REQ-SYN-001,
/// revue F2). Ne divulgue pas le foyer propriétaire (§9).
#[requirement(REQ-SYN-001)]
fn duplicate_id() -> Response {
    problem_response(
        StatusCode::CONFLICT,
        problem(409, "about:blank", "Conflict").with_detail("id: identifiant déjà utilisé"),
    )
}

/// `422` identifiant le champ fautif (RFC 9457 `detail`), REQ-SUB-002 critère #2.
#[requirement(REQ-SUB-002)]
fn field_error(err: &FieldError) -> Response {
    problem_response(
        StatusCode::UNPROCESSABLE_ENTITY,
        problem(422, "about:blank", "Unprocessable Entity")
            .with_detail(format!("{}: {}", err.field, err.message)),
    )
}

/// `404` générique pour un abonnement inconnu ou hors du foyer — ne divulgue rien (§9).
#[requirement(REQ-SUB-004)]
fn subscription_not_found() -> Response {
    problem_response(
        StatusCode::NOT_FOUND,
        problem(404, "about:blank", "Not Found"),
    )
}

/// Crée un abonnement dans le foyer de l'appelant ; renvoie l'abonnement et sa prochaine échéance.
/// **Idempotent** via `Idempotency-Key` (REQ-SYN-006).
#[utoipa::path(
    post,
    path = "/subscriptions",
    operation_id = "createSubscription",
    extensions(("x-requirements" = json!(["REQ-SUB-002", "REQ-SYN-006"]))),
    params(("Idempotency-Key" = Option<String>, Header, description = "Clé d'idempotence (REQ-SYN-006)")),
    request_body = CreateSubscriptionRequest,
    responses(
        (status = 201, description = "Abonnement créé (avec prochaine échéance)", body = SubscriptionDto, content_type = "application/json"),
        (status = 401, description = "Non authentifié", body = wallos_proto::Problem, content_type = "application/problem+json"),
        (status = 409, description = "Conflit : identifiant déjà utilisé, ou clé d'idempotence réutilisée avec un corps différent", body = wallos_proto::Problem, content_type = "application/problem+json"),
        (status = 422, description = "Validation par champ (dont identifiant)", body = wallos_proto::Problem, content_type = "application/problem+json")
    )
)]
#[requirement(REQ-SUB-002)]
#[requirement(REQ-SYN-006)]
pub async fn create_subscription(
    AuthActor(actor): AuthActor,
    State(db): State<Db>,
    IdempotencyKey(idem): IdempotencyKey,
    Json(req): Json<CreateSubscriptionRequest>,
) -> Response {
    if let Outcome::Short(response) = idempotency::begin(&db, &actor, idem.as_deref(), &req).await {
        return response;
    }
    match build_subscription(&db, &actor, req).await {
        Ok((status, body)) => {
            idempotency::finish(&db, &actor, idem.as_deref(), status, &body).await;
            (status, [(header::CONTENT_TYPE, "application/json")], body).into_response()
        }
        Err(response) => {
            idempotency::abort(&db, &actor, idem.as_deref()).await;
            response
        }
    }
}

/// Exécute la création (validation + calcul d'échéance + persistance) et renvoie `(statut, corps JSON)`
/// mémorisable, ou une réponse d'erreur (non mémorisée).
#[requirement(REQ-SUB-002)]
async fn build_subscription(
    db: &Db,
    actor: &Actor,
    req: CreateSubscriptionRequest,
) -> Result<(StatusCode, String), Response> {
    // L'identifiant peut être **fourni par le client** (offline-first, REQ-SYN-001) : conservé tel
    // quel s'il est présent et valide, sinon généré par le serveur.
    let id = req.resolve_id().map_err(|err| field_error(&err))?;
    let subscription = req.into_core(id).map_err(|err| field_error(&err))?;

    // Prochaine échéance : première occurrence à partir d'aujourd'hui (inclus), bornée par la date de
    // fin (REQ-SUB-009). Horloge serveur — le domaine reste pur (l'instant est injecté ici).
    let today = Utc::now().date_naive();
    let reference = today.pred_opt().unwrap_or(today);
    let next = subscription.next_due_after(reference);
    let ended = subscription.has_ended(today);
    // Un abonnement déjà terminé n'a pas de prochaine échéance (légitime) ; sinon l'absence d'échéance
    // signale une date hors plage.
    if next.is_none() && !ended {
        return Err(field_error(&FieldError::new(
            "first_payment",
            "échéance hors plage",
        )));
    }

    match SubscriptionRepository::new(db.pool())
        .create(actor, &subscription)
        .await
    {
        Ok(CreateOutcome::Created) => {
            let dto = SubscriptionDto::from_core_derived(&subscription, next, ended);
            Ok((
                StatusCode::CREATED,
                serde_json::to_string(&dto).unwrap_or_default(),
            ))
        }
        // Id client déjà pris (REQ-SYN-001, revue F2) : conflit au lieu d'un 500.
        Ok(CreateOutcome::DuplicateId) => Err(duplicate_id()),
        // Aucune unicité de nom sur les abonnements : DuplicateName ne peut pas survenir.
        Ok(CreateOutcome::DuplicateName) | Err(_) => Err(internal_error()),
    }
}

/// Modifie un abonnement du foyer de l'appelant ; recalcule la prochaine échéance (REQ-SUB-004).
///
/// L'échéance est **ré-ancrée sur `first_payment`** avec le nouveau cycle (jamais dérivée de
/// l'ancienne). La résolution des modifications concurrentes (horodatage antérieur) relève de
/// REQ-SYN-005, hors de ce vertical : ici, la dernière écriture reçue fait foi.
#[utoipa::path(
    put,
    path = "/subscriptions/{id}",
    operation_id = "updateSubscription",
    params(("id" = String, Path, description = "Identifiant (UUID) de l'abonnement")),
    extensions(("x-requirements" = json!(["REQ-SUB-004"]))),
    request_body = CreateSubscriptionRequest,
    responses(
        (status = 200, description = "Abonnement modifié (échéance recalculée)", body = SubscriptionDto, content_type = "application/json"),
        (status = 401, description = "Non authentifié", body = wallos_proto::Problem, content_type = "application/problem+json"),
        (status = 404, description = "Abonnement inconnu ou hors du foyer", body = wallos_proto::Problem, content_type = "application/problem+json"),
        (status = 422, description = "Validation par champ", body = wallos_proto::Problem, content_type = "application/problem+json")
    )
)]
#[requirement(REQ-SUB-004)]
pub async fn update_subscription(
    AuthActor(actor): AuthActor,
    State(db): State<Db>,
    Path(id): Path<String>,
    Json(req): Json<CreateSubscriptionRequest>,
) -> Response {
    let Ok(uuid) = Uuid::parse_str(&id) else {
        return subscription_not_found();
    };
    let subscription = match req.into_core(uuid) {
        Ok(sub) => sub,
        Err(err) => return field_error(&err),
    };

    let today = Utc::now().date_naive();
    let reference = today.pred_opt().unwrap_or(today);
    let next = subscription.next_due_after(reference);
    let ended = subscription.has_ended(today);
    if next.is_none() && !ended {
        return field_error(&FieldError::new("first_payment", "échéance hors plage"));
    }

    match SubscriptionRepository::new(db.pool())
        .update(&actor, &subscription)
        .await
    {
        Ok(true) => Json(SubscriptionDto::from_core_derived(
            &subscription,
            next,
            ended,
        ))
        .into_response(),
        Ok(false) => subscription_not_found(),
        _ => problem_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            problem(500, "about:blank", "Internal Server Error"),
        ),
    }
}

/// Supprime un abonnement du foyer de l'appelant (REQ-SUB-005).
///
/// **Suppression traçable** (REQ-SYN-002) : une pierre tombale horodatée est créée dans la même
/// transaction, si bien qu'un autre appareil applique la suppression au lieu de réintroduire
/// l'abonnement. L'abonnement disparaît de **toutes** les vues (liste, agrégats). Isolation §9 : un
/// abonnement inconnu ou d'un autre foyer se comporte comme inexistant (`404`, jamais `403`).
#[utoipa::path(
    delete,
    path = "/subscriptions/{id}",
    operation_id = "deleteSubscription",
    params(("id" = String, Path, description = "Identifiant (UUID) de l'abonnement")),
    extensions(("x-requirements" = json!(["REQ-SUB-005"]))),
    responses(
        (status = 204, description = "Abonnement supprimé (pierre tombale créée)"),
        (status = 401, description = "Non authentifié", body = wallos_proto::Problem, content_type = "application/problem+json"),
        (status = 404, description = "Abonnement inconnu ou hors du foyer", body = wallos_proto::Problem, content_type = "application/problem+json")
    )
)]
#[requirement(REQ-SUB-005)]
pub async fn delete_subscription(
    AuthActor(actor): AuthActor,
    State(db): State<Db>,
    Path(id): Path<String>,
) -> Response {
    let Ok(uuid) = Uuid::parse_str(&id) else {
        return subscription_not_found();
    };
    match SubscriptionRepository::new(db.pool())
        .delete(&actor, uuid)
        .await
    {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => subscription_not_found(),
        Err(_) => internal_error(),
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
    // Cycle dérivé une seule fois : `next_payment` et l'intervalle affiché restent cohérents (revue
    // SUB-006 #3 — jamais un intervalle=0 exposé pendant que le calcul d'échéance est abandonné).
    let cycle = row_cycle(&row);
    // Prochaine échéance bornée par la date de fin (REQ-SUB-009) : aucune échéance au-delà de `end_date`.
    let next_payment = cycle
        .and_then(|c| next_due(row.first_payment, c, reference))
        .filter(|d| row.end_date.is_none_or(|end| *d <= end))
        .map(|d| d.to_string());
    // Terminé si la date de fin est strictement dépassée à la date du jour (REQ-SUB-009).
    let ended = row.end_date.is_some_and(|end| today > end);
    // Coûts normalisés (mensuel REQ-STA-001, annuel REQ-STA-002), dans la devise de l'abonnement. Une
    // ligne à la devise, au cycle ou au montant illisible (négatif) retombe sur le montant brut
    // (jamais un zéro silencieux — revue STA-001 F2).
    let raw = || (row.amount.to_string(), row.amount.to_string());
    let (monthly, yearly) = match (CurrencyCode::new(&row.currency), cycle) {
        (Ok(currency), Some(cycle)) => match Money::new(row.amount, currency) {
            Ok(price) => (
                wallos_core::stats::monthly_cost_rounded(price, cycle)
                    .amount()
                    .to_string(),
                wallos_core::stats::yearly_cost_rounded(price, cycle)
                    .amount()
                    .to_string(),
            ),
            // Montant corrompu (négatif) : on expose le brut, jamais un « 0 » mensonger.
            Err(_) => raw(),
        },
        _ => raw(),
    };
    SubscriptionDto {
        id: row.id.to_string(),
        name: row.name,
        amount: row.amount.to_string(),
        currency: row.currency,
        cycle: BillingCycleDto {
            unit: row.cycle_unit,
            interval: cycle.map_or(0, |c| c.interval()),
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
        end_date: row.end_date.map(|d| d.to_string()),
        ended,
        monthly_cost: monthly,
        yearly_cost: yearly,
    }
}

/// Montants entrant dans le total d'un lot à la date `today` (REQ-SUB-008 / REQ-SUB-009) : un abonnement
/// **désactivé** ou **terminé** (date de fin dépassée) est exclu de l'agrégat (conservé dans la liste,
/// mais ne pèse sur aucun total). Une ligne à la devise/au montant illisible est ignorée du total
/// (jamais traitée comme zéro silencieux dans le calcul).
#[requirement(REQ-SUB-008)]
#[requirement(REQ-SUB-009)]
fn active_amounts(rows: &[SubscriptionRow], today: NaiveDate) -> Vec<Money> {
    rows.iter()
        .filter(|r| r.active && r.end_date.is_none_or(|end| today <= end))
        .filter_map(|r| {
            CurrencyCode::new(&r.currency)
                .ok()
                .and_then(|c| Money::new(r.amount, c).ok())
        })
        .collect()
}

/// Parse un identifiant de filtre optionnel (UUID) ; `Err` nomme le champ fautif (→ 422).
#[requirement(REQ-SUB-006)]
fn filter_id(raw: Option<String>, field: &'static str) -> Result<Option<Uuid>, FieldError> {
    raw.filter(|s| !s.is_empty())
        .map(|s| Uuid::parse_str(&s).map_err(|_| FieldError::new(field, "identifiant invalide")))
        .transpose()
}

/// Critère de tri de la liste (REQ-SUB-007). Défaut **tolérant** : une valeur absente ou inconnue
/// retombe sur `Name` (jamais une 422 — le tri est un confort d'affichage, non une contrainte).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SortCriterion {
    /// Par nom, ordre alphabétique replié (casse+diacritiques ignorées), croissant.
    Name,
    /// Par **coût mensuel normalisé** (REQ-STA-001) **converti en devise de référence**, décroissant.
    Amount,
    /// Par prochaine échéance, croissant (les abonnements sans échéance à venir en fin de liste).
    NextDue,
}

impl SortCriterion {
    /// Interprète le paramètre `sort` ; toute valeur non reconnue (ou absente) donne `Name`.
    #[requirement(REQ-SUB-007)]
    fn parse(raw: Option<&str>) -> Self {
        match raw {
            Some("amount") => Self::Amount,
            Some("next_due") => Self::NextDue,
            _ => Self::Name,
        }
    }
}

/// Clé de tri pré-calculée pour un abonnement, portée avec son DTO afin de trier **une seule fois**
/// (le coût normalisé et l'échéance ne sont pas recalculés à chaque comparaison).
struct Sortable {
    /// Nom replié (casse+diacritiques ignorées) — clé primaire du tri par nom et départage stable.
    folded_name: String,
    /// Coût mensuel converti en devise de référence (`None` si devise/cycle illisible ou taux manquant).
    amount_ref: Option<Decimal>,
    /// Prochaine échéance (`None` si aucune : terminé ou hors plage).
    next_due: Option<NaiveDate>,
    /// Identifiant (chaîne UUID) — départage final déterministe.
    id: String,
    /// DTO projeté, restitué après tri.
    dto: SubscriptionDto,
}

/// Coût mensuel normalisé (REQ-STA-001) d'une ligne, **converti en devise de référence** (REQ-CUR-003).
///
/// `None` si la devise, le cycle ou le montant sont illisibles, ou si aucun taux n'est connu pour la
/// paire — l'abonnement est alors trié en fin de liste, jamais assimilé à un coût nul (revue STA-001 F2).
#[requirement(REQ-SUB-007)]
fn monthly_in_reference(
    row: &SubscriptionRow,
    target: CurrencyCode,
    table: &RateTable,
) -> Option<Decimal> {
    let currency = CurrencyCode::new(&row.currency).ok()?;
    let price = Money::new(row.amount, currency).ok()?;
    let cycle = row_cycle(row)?;
    let monthly = wallos_core::stats::monthly_cost_rounded(price, cycle);
    wallos_core::convert(&monthly, target, table).map(|m| m.amount())
}

/// Compare deux `Option` en plaçant `None` **en dernier** (ascendant sur les `Some`).
#[requirement(REQ-SUB-007)]
fn none_last<T: Ord>(a: &Option<T>, b: &Option<T>) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    match (a, b) {
        (Some(x), Some(y)) => x.cmp(y),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

/// Trie les abonnements selon le critère, avec départage **stable et déterministe** : nom replié puis
/// identifiant. Le tri par montant est décroissant (les plus coûteux d'abord) ; nom et échéance sont
/// croissants (REQ-SUB-007).
#[requirement(REQ-SUB-007)]
fn sort_subscriptions(items: &mut [Sortable], criterion: SortCriterion) {
    items.sort_by(|a, b| {
        let tiebreak = a
            .folded_name
            .cmp(&b.folded_name)
            .then_with(|| a.id.cmp(&b.id));
        use std::cmp::Ordering;
        let primary = match criterion {
            SortCriterion::Name => Ordering::Equal,
            // Décroissant sur les `Some` (plus coûteux d'abord), `None` toujours en dernier.
            SortCriterion::Amount => match (&a.amount_ref, &b.amount_ref) {
                (Some(x), Some(y)) => y.cmp(x),
                (Some(_), None) => Ordering::Less,
                (None, Some(_)) => Ordering::Greater,
                (None, None) => Ordering::Equal,
            },
            SortCriterion::NextDue => none_last(&a.next_due, &b.next_due),
        };
        primary.then(tiebreak)
    });
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
    // Devise cible du total : la **devise de référence du foyer** (REQ-CUR-001) par défaut ; un
    // paramètre `currency` explicite reste un override ponctuel. Les agrégats s'expriment ainsi dans
    // la devise choisie par le foyer, sans jamais altérer les montants saisis.
    let target_code = match q.currency.filter(|s| !s.is_empty()) {
        Some(explicit) => explicit,
        None => match SettingsRepository::new(db.pool())
            .reference_currency(&actor)
            .await
        {
            Ok(code) => code,
            Err(_) => {
                return problem_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    problem(500, "about:blank", "Internal Server Error"),
                );
            }
        },
    };
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

    // Recherche plein-texte (REQ-SUB-007) : filtrage **insensible casse+diacritiques** sur le nom
    // **ET** les notes, appliqué avant l'agrégat et le tri — le total reflète ainsi exactement la vue
    // affichée. Une requête vide/absente n'exclut rien.
    let search = q.search.unwrap_or_default();
    let rows: Vec<SubscriptionRow> = if search.trim().is_empty() {
        rows
    } else {
        rows.into_iter()
            .filter(|r| {
                wallos_core::matches_search(&r.name, &search)
                    || wallos_core::matches_search(r.notes.as_deref().unwrap_or(""), &search)
            })
            .collect()
    };

    // Total = somme convertie des abonnements **actifs et non terminés** du sous-ensemble filtré
    // (REQ-SUB-008/009). Horloge serveur pour décider « terminé » et la prochaine échéance.
    let today = Utc::now().date_naive();
    let amounts = active_amounts(&rows, today);
    let criterion = SortCriterion::parse(q.sort.as_deref());
    // Taux nécessaires si un montant actif doit être agrégé **ou** si le tri par montant normalisé est
    // demandé (il convertit chaque coût mensuel en devise de référence). Sinon, on évite l'aller-retour
    // base (revue SUB-006 #8).
    let table = if amounts.is_empty() && criterion != SortCriterion::Amount {
        RateTable::new(Vec::new())
    } else {
        match load_rate_table(&ExchangeRateRepository::new(db.pool())).await {
            Ok(t) => t,
            Err(_) => {
                return problem_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    problem(500, "about:blank", "Internal Server Error"),
                );
            }
        }
    };
    let agg = aggregate_converted(&amounts, target, &table);

    // Projection + clés de tri en une passe, puis tri déterministe (REQ-SUB-007).
    let mut items: Vec<Sortable> = rows
        .into_iter()
        .map(|r| {
            let amount_ref = if criterion == SortCriterion::Amount {
                monthly_in_reference(&r, target, &table)
            } else {
                None
            };
            let dto = row_to_dto(r, today);
            let next_due = dto
                .next_payment
                .as_deref()
                .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok());
            Sortable {
                folded_name: wallos_core::fold_for_search(&dto.name),
                amount_ref,
                next_due,
                id: dto.id.clone(),
                dto,
            }
        })
        .collect();
    sort_subscriptions(&mut items, criterion);
    let subscriptions: Vec<SubscriptionDto> = items.into_iter().map(|s| s.dto).collect();
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
