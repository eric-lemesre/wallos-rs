//! Export et import des données d'un foyer (REQ-SUB-016).
//!
//! Deux opérations symétriques, isolées par foyer (§9) :
//! - `GET /export` sérialise **toutes** les entités possédées du foyer (catégories, moyens de paiement,
//!   abonnements) plus sa devise de référence, dans une enveloppe [`DataBundle`] qui réutilise les
//!   requêtes de création — l'`id` de chaque entité est **préservé** (REQ-SYN-001).
//! - `POST /import` recrée ces entités dans le foyer appelant. L'import est **tolérant** : chaque ligne
//!   invalide est **rejetée avec sa raison** (jamais un échec global silencieux), et les lignes valides
//!   sont créées. Un export réimporté dans un compte vierge reconstruit l'état **à l'identique**
//!   (mêmes identifiants ⇒ mêmes liaisons ; les échéances, dérivées, sont recalculées à la lecture).
//!
//! Les **devises** ne sont pas des entités (référentiel figé, REQ-CUR-002) : elles sont seulement
//! **validées** à l'import (code hors référentiel ⇒ ligne rejetée). Voir ADR 0027.

use std::collections::HashMap;

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use uuid::Uuid;
use wallos_core::requirement;
use wallos_core::{Category, PaymentMethod};
use wallos_proto::{
    BillingCycleDto, CreateCategoryRequest, CreatePaymentMethodRequest, CreateSubscriptionRequest,
    DATA_BUNDLE_VERSION, DataBundle, FieldError, ImportCounts, ImportReport, RejectedRow, problem,
};
use wallos_storage::{
    CategoryRepository, CreateOutcome, Db, PaymentMethodRepository, SettingsRepository,
    SubscriptionFilter, SubscriptionRepository, SubscriptionRow,
};

use crate::auth::AuthActor;
use crate::problem_response;

/// `422` identifiant le champ fautif (RFC 9457 `detail`).
#[requirement(REQ-SUB-016)]
fn field_error(field: &str, message: &str) -> Response {
    problem_response(
        StatusCode::UNPROCESSABLE_ENTITY,
        problem(422, "about:blank", "Unprocessable Entity")
            .with_detail(format!("{field}: {message}")),
    )
}

/// `500` générique (défaut interne non divulgué).
#[requirement(REQ-SUB-016)]
fn internal_error() -> Response {
    problem_response(
        StatusCode::INTERNAL_SERVER_ERROR,
        problem(500, "about:blank", "Internal Server Error"),
    )
}

/// Traduit une erreur de validation par champ en ligne rejetée (champ fautif + message).
fn reject_field(kind: &str, reference: &str, err: &FieldError) -> RejectedRow {
    RejectedRow {
        kind: kind.to_string(),
        reference: reference.to_string(),
        reason: format!("{}: {}", err.field, err.message),
    }
}

/// Projette une ligne d'abonnement stockée vers une requête de création (champs persistables, `id`
/// préservé) — la face **export** du round-trip.
#[requirement(REQ-SUB-016)]
fn row_to_create_request(row: SubscriptionRow) -> CreateSubscriptionRequest {
    CreateSubscriptionRequest {
        id: Some(row.id.to_string()),
        name: row.name,
        amount: row.amount.to_string(),
        currency: row.currency,
        cycle: BillingCycleDto {
            unit: row.cycle_unit,
            // `cycle_interval` est un `integer` SQL toujours > 0 ; le repli n'est jamais atteint.
            interval: u32::try_from(row.cycle_interval).unwrap_or(1),
        },
        first_payment: row.first_payment.format("%Y-%m-%d").to_string(),
        category: row.category_id.map(|u| u.to_string()),
        payment_method: row.payment_method_id.map(|u| u.to_string()),
        payer: row.payer_id.map(|u| u.to_string()),
        logo: row.logo,
        url: row.url,
        notes: row.notes,
        active: row.active,
        end_date: row.end_date.map(|d| d.format("%Y-%m-%d").to_string()),
    }
}

/// Exporte toutes les données du foyer appelant dans une enveloppe réimportable (REQ-SUB-016).
#[utoipa::path(
    get,
    path = "/export",
    operation_id = "exportData",
    extensions(("x-requirements" = json!(["REQ-SUB-016"]))),
    responses(
        (status = 200, description = "Enveloppe complète des données du foyer", body = DataBundle, content_type = "application/json"),
        (status = 401, description = "Non authentifié", body = wallos_proto::Problem, content_type = "application/problem+json"),
        (status = 500, description = "Erreur interne", body = wallos_proto::Problem, content_type = "application/problem+json")
    )
)]
#[requirement(REQ-SUB-016)]
pub async fn export_data(AuthActor(actor): AuthActor, State(db): State<Db>) -> Response {
    let pool = db.pool();

    let categories = match CategoryRepository::new(pool).list(&actor).await {
        Ok(rows) => rows
            .into_iter()
            .map(|c| CreateCategoryRequest {
                id: Some(c.id.to_string()),
                name: c.name,
            })
            .collect(),
        Err(_) => return internal_error(),
    };

    let payment_methods = match PaymentMethodRepository::new(pool).list(&actor).await {
        Ok(rows) => rows
            .into_iter()
            .map(|p| CreatePaymentMethodRequest {
                id: Some(p.id.to_string()),
                name: p.name,
            })
            .collect(),
        Err(_) => return internal_error(),
    };

    let subscriptions = match SubscriptionRepository::new(pool)
        .list(&actor, &SubscriptionFilter::default())
        .await
    {
        Ok(rows) => rows.into_iter().map(row_to_create_request).collect(),
        Err(_) => return internal_error(),
    };

    // Devise de référence du foyer (REQ-CUR-001) : incluse pour un round-trip complet.
    let reference_currency = SettingsRepository::new(pool)
        .reference_currency(&actor)
        .await
        .ok();

    Json(DataBundle {
        version: DATA_BUNDLE_VERSION,
        reference_currency,
        categories,
        payment_methods,
        subscriptions,
    })
    .into_response()
}

/// Importe une enveloppe dans le foyer appelant et renvoie un rapport (REQ-SUB-016).
///
/// Ordre de traitement : devise de référence, puis catégories et moyens de paiement (référencés par
/// les abonnements), puis abonnements. Chaque ligne invalide est **rejetée avec sa raison** ; les
/// lignes valides sont créées.
#[utoipa::path(
    post,
    path = "/import",
    operation_id = "importData",
    extensions(("x-requirements" = json!(["REQ-SUB-016"]))),
    request_body = DataBundle,
    responses(
        (status = 200, description = "Rapport d'import (créées + rejetées)", body = ImportReport, content_type = "application/json"),
        (status = 401, description = "Non authentifié", body = wallos_proto::Problem, content_type = "application/problem+json"),
        (status = 422, description = "Version de format non prise en charge", body = wallos_proto::Problem, content_type = "application/problem+json"),
        (status = 500, description = "Erreur interne", body = wallos_proto::Problem, content_type = "application/problem+json")
    )
)]
#[requirement(REQ-SUB-016)]
pub async fn import_data(
    AuthActor(actor): AuthActor,
    State(db): State<Db>,
    Json(bundle): Json<DataBundle>,
) -> Response {
    if bundle.version != DATA_BUNDLE_VERSION {
        return field_error("version", "version de format non prise en charge");
    }
    let pool = db.pool();
    let mut counts = ImportCounts::default();
    let mut rejected: Vec<RejectedRow> = Vec::new();

    // Devise de référence : validée contre le référentiel figé (REQ-CUR-002) ; inconnue ⇒ rejetée.
    if let Some(code) = bundle.reference_currency {
        if wallos_core::currencies::is_supported(&code) {
            if SettingsRepository::new(pool)
                .set_reference_currency(&actor, &code)
                .await
                .is_err()
            {
                return internal_error();
            }
        } else {
            rejected.push(RejectedRow {
                kind: "reference_currency".to_string(),
                reference: code,
                reason: "devise hors référentiel".to_string(),
            });
        }
    }

    // Catégories : **fusion par nom** (insensible à la casse). Un compte neuf porte déjà les catégories
    // par défaut (REQ-CAT-002), avec des identifiants **propres au foyer** ; on ne les recrée donc pas —
    // on réutilise l'existante et on **remappe** les abonnements qui la référencent, pour préserver les
    // liaisons sans empiler de doublons. Seules les catégories réellement nouvelles sont créées (id
    // préservé, REQ-SYN-001). `cat_map` traduit l'id d'origine (bundle) vers l'id résolu dans ce foyer.
    let cat_repo = CategoryRepository::new(pool);
    let mut existing: HashMap<String, Uuid> = match cat_repo.list(&actor).await {
        Ok(rows) => rows
            .into_iter()
            .map(|c| (c.name.trim().to_lowercase(), c.id))
            .collect(),
        Err(_) => return internal_error(),
    };
    let mut cat_map: HashMap<String, Uuid> = HashMap::new();
    for req in bundle.categories {
        let reference = req.name.clone();
        let bundle_id = req.id.clone();
        let key = req.name.trim().to_lowercase();
        if let Some(&resolved) = existing.get(&key) {
            if let Some(bid) = bundle_id {
                cat_map.insert(bid, resolved);
            }
            continue; // Déjà présente : réutilisée, ni créée ni rejetée.
        }
        let id = match req.resolve_id() {
            Ok(id) => id,
            Err(e) => {
                rejected.push(reject_field("category", &reference, &e));
                continue;
            }
        };
        let category = match Category::new(id, req.name) {
            Ok(c) => c,
            Err(_) => {
                rejected.push(RejectedRow {
                    kind: "category".to_string(),
                    reference,
                    reason: "nom de catégorie invalide".to_string(),
                });
                continue;
            }
        };
        match cat_repo
            .create(&actor, category.id(), category.name())
            .await
        {
            Ok(CreateOutcome::Created) => {
                counts.categories += 1;
                existing.insert(key, id);
                if let Some(bid) = bundle_id {
                    cat_map.insert(bid, id);
                }
            }
            // Course improbable (créée entre-temps) : réutiliser par nom au lieu de rejeter.
            Ok(CreateOutcome::DuplicateName) => {
                if let Some(bid) = bundle_id {
                    cat_map.insert(bid, id);
                }
            }
            // Id déjà pris par une entité d'un **autre** foyer (clé primaire globale) : rejeté.
            Ok(CreateOutcome::DuplicateId) => rejected.push(RejectedRow {
                kind: "category".to_string(),
                reference,
                reason: "identifiant déjà présent".to_string(),
            }),
            Err(_) => return internal_error(),
        }
    }

    let pm_repo = PaymentMethodRepository::new(pool);
    for req in bundle.payment_methods {
        let reference = req.name.clone();
        let id = match req.resolve_id() {
            Ok(id) => id,
            Err(e) => {
                rejected.push(reject_field("payment_method", &reference, &e));
                continue;
            }
        };
        let pm = match PaymentMethod::new(id, req.name) {
            Ok(p) => p,
            Err(_) => {
                rejected.push(RejectedRow {
                    kind: "payment_method".to_string(),
                    reference,
                    reason: "nom de moyen de paiement invalide".to_string(),
                });
                continue;
            }
        };
        match pm_repo.create(&actor, pm.id(), pm.name()).await {
            Ok(CreateOutcome::Created) => counts.payment_methods += 1,
            // Aucune unicité de nom sur les moyens de paiement : DuplicateName ne survient pas.
            Ok(CreateOutcome::DuplicateId) => rejected.push(RejectedRow {
                kind: "payment_method".to_string(),
                reference,
                reason: "identifiant déjà présent".to_string(),
            }),
            Ok(CreateOutcome::DuplicateName) | Err(_) => return internal_error(),
        }
    }

    let sub_repo = SubscriptionRepository::new(pool);
    for mut req in bundle.subscriptions {
        let reference = req.name.clone();
        // Remappe la catégorie référencée vers l'id résolu dans ce foyer (fusion ci-dessus). Absente
        // de `cat_map` (ex. catégorie non incluse dans l'enveloppe) ⇒ conservée telle quelle.
        if let Some(cid) = req.category.take() {
            req.category = Some(cat_map.get(&cid).map_or(cid, ToString::to_string));
        }
        let id = match req.resolve_id() {
            Ok(id) => id,
            Err(e) => {
                rejected.push(reject_field("subscription", &reference, &e));
                continue;
            }
        };
        let sub = match req.into_core(id) {
            Ok(s) => s,
            Err(e) => {
                rejected.push(reject_field("subscription", &reference, &e));
                continue;
            }
        };
        match sub_repo.create(&actor, &sub).await {
            Ok(CreateOutcome::Created) => counts.subscriptions += 1,
            Ok(CreateOutcome::DuplicateId) => rejected.push(RejectedRow {
                kind: "subscription".to_string(),
                reference,
                reason: "identifiant déjà présent".to_string(),
            }),
            // Aucune unicité de nom sur les abonnements : DuplicateName ne survient pas.
            Ok(CreateOutcome::DuplicateName) | Err(_) => return internal_error(),
        }
    }

    Json(ImportReport {
        imported: counts,
        rejected,
    })
    .into_response()
}
