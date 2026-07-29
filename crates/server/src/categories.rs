//! Gestion des catégories (REQ-CAT-001).
//!
//! CRUD **isolé par foyer** : chaque handler passe l'`Actor` au `CategoryRepository`, qui filtre par
//! `household_id`. Une catégorie d'un autre foyer est traitée comme inexistante (`404`, jamais `403`,
//! §9). Une catégorie créée est immédiatement disponible dans la liste (source du formulaire d'abonnement).

use axum::Json;
use axum::extract::{Path, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use uuid::Uuid;
use wallos_core::Category;
use wallos_core::actor::Actor;
use wallos_core::requirement;
use wallos_proto::{CategoryDto, CreateCategoryRequest, RenameCategoryRequest, problem};
use wallos_storage::{CategoryRepository, Db, RenameOutcome};

use crate::auth::AuthActor;
use crate::idempotency::{self, IdempotencyKey, Outcome};
use crate::problem_response;

/// `500` générique (défaut interne non divulgué).
#[requirement(REQ-CAT-001)]
fn internal_error() -> Response {
    problem_response(
        StatusCode::INTERNAL_SERVER_ERROR,
        problem(500, "about:blank", "Internal Server Error"),
    )
}

/// `404` générique pour une catégorie inconnue ou hors du foyer — ne divulgue rien (§9).
#[requirement(REQ-CAT-001)]
fn category_not_found() -> Response {
    problem_response(
        StatusCode::NOT_FOUND,
        problem(404, "about:blank", "Not Found"),
    )
}

/// `422` pour un nom de catégorie invalide (vide).
#[requirement(REQ-CAT-001)]
fn invalid_category() -> Response {
    problem_response(
        StatusCode::UNPROCESSABLE_ENTITY,
        problem(422, "about:blank", "Unprocessable Entity"),
    )
}

/// `422` pour un nom de catégorie déjà utilisé dans le foyer (unicité REQ-CAT-004).
#[requirement(REQ-CAT-004)]
fn duplicate_category() -> Response {
    problem_response(
        StatusCode::UNPROCESSABLE_ENTITY,
        problem(422, "about:blank", "Unprocessable Entity")
            .with_detail("name: déjà utilisé dans ce foyer"),
    )
}

/// Crée une catégorie dans le foyer de l'appelant. **Idempotent** via l'en-tête `Idempotency-Key`
/// (REQ-SYN-006) : un rejeu à clé + corps identiques renvoie la réponse initiale sans nouvelle création.
#[utoipa::path(
    post,
    path = "/categories",
    operation_id = "createCategory",
    extensions(("x-requirements" = json!(["REQ-CAT-001", "REQ-SYN-006"]))),
    params(("Idempotency-Key" = Option<String>, Header, description = "Clé d'idempotence (REQ-SYN-006)")),
    request_body = CreateCategoryRequest,
    responses(
        (status = 201, description = "Catégorie créée", body = CategoryDto, content_type = "application/json"),
        (status = 401, description = "Non authentifié", body = wallos_proto::Problem, content_type = "application/problem+json"),
        (status = 409, description = "Clé d'idempotence réutilisée avec un corps différent", body = wallos_proto::Problem, content_type = "application/problem+json"),
        (status = 422, description = "Nom invalide", body = wallos_proto::Problem, content_type = "application/problem+json")
    )
)]
#[requirement(REQ-CAT-001)]
#[requirement(REQ-SYN-006)]
pub async fn create_category(
    AuthActor(actor): AuthActor,
    State(db): State<Db>,
    IdempotencyKey(idem): IdempotencyKey,
    Json(req): Json<CreateCategoryRequest>,
) -> Response {
    if let Outcome::Short(response) = idempotency::begin(&db, &actor, idem.as_deref(), &req).await {
        return response;
    }
    match build_category(&db, &actor, req).await {
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

/// Exécute la création (validation + persistance) et renvoie la réponse **réussie** sous forme
/// `(statut, corps JSON)` mémorisable, ou une réponse d'erreur (non mémorisée).
#[requirement(REQ-CAT-001)]
async fn build_category(
    db: &Db,
    actor: &Actor,
    req: CreateCategoryRequest,
) -> Result<(StatusCode, String), Response> {
    // L'identifiant peut être **fourni par le client** (offline-first, REQ-SYN-001) : conservé tel
    // quel s'il est présent et valide, sinon généré par le serveur.
    let Ok(id) = req.resolve_id() else {
        return Err(invalid_category());
    };
    let Ok(category) = Category::new(id, req.name) else {
        return Err(invalid_category());
    };
    match CategoryRepository::new(db.pool())
        .create(actor, category.id(), category.name())
        .await
    {
        Ok(true) => {
            let dto = CategoryDto {
                id: category.id().to_string(),
                name: category.name().to_string(),
            };
            Ok((
                StatusCode::CREATED,
                serde_json::to_string(&dto).unwrap_or_default(),
            ))
        }
        // Nom déjà utilisé dans le foyer (unicité CAT-004) : erreur de validation par champ.
        Ok(false) => Err(duplicate_category()),
        _ => Err(internal_error()),
    }
}

/// Liste les catégories du foyer de l'appelant (disponibles immédiatement pour le formulaire).
#[utoipa::path(
    get,
    path = "/categories",
    operation_id = "listCategories",
    extensions(("x-requirements" = json!(["REQ-CAT-001"]))),
    responses(
        (status = 200, description = "Catégories du foyer", body = Vec<CategoryDto>, content_type = "application/json"),
        (status = 401, description = "Non authentifié", body = wallos_proto::Problem, content_type = "application/problem+json")
    )
)]
#[requirement(REQ-CAT-001)]
pub async fn list_categories(AuthActor(actor): AuthActor, State(db): State<Db>) -> Response {
    match CategoryRepository::new(db.pool()).list(&actor).await {
        Ok(rows) => {
            let categories: Vec<CategoryDto> = rows
                .into_iter()
                .map(|row| CategoryDto {
                    id: row.id.to_string(),
                    name: row.name,
                })
                .collect();
            Json(categories).into_response()
        }
        _ => problem_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            problem(500, "about:blank", "Internal Server Error"),
        ),
    }
}

/// Renomme une catégorie du foyer de l'appelant.
#[utoipa::path(
    put,
    path = "/categories/{id}",
    operation_id = "renameCategory",
    params(("id" = String, Path, description = "Identifiant (UUID) de la catégorie")),
    extensions(("x-requirements" = json!(["REQ-CAT-001"]))),
    request_body = RenameCategoryRequest,
    responses(
        (status = 200, description = "Catégorie renommée", body = CategoryDto, content_type = "application/json"),
        (status = 401, description = "Non authentifié", body = wallos_proto::Problem, content_type = "application/problem+json"),
        (status = 404, description = "Catégorie inconnue ou hors du foyer", body = wallos_proto::Problem, content_type = "application/problem+json"),
        (status = 422, description = "Nom invalide", body = wallos_proto::Problem, content_type = "application/problem+json")
    )
)]
#[requirement(REQ-CAT-001)]
pub async fn rename_category(
    AuthActor(actor): AuthActor,
    State(db): State<Db>,
    Path(id): Path<String>,
    Json(req): Json<RenameCategoryRequest>,
) -> Response {
    let Ok(category_id) = Uuid::parse_str(&id) else {
        return category_not_found();
    };
    // Valide le nom via le modèle de domaine (non vide).
    let Ok(category) = Category::new(category_id, req.name) else {
        return invalid_category();
    };
    match CategoryRepository::new(db.pool())
        .rename(&actor, category_id, category.name())
        .await
    {
        Ok(RenameOutcome::Renamed) => Json(CategoryDto {
            id: category_id.to_string(),
            name: category.name().to_string(),
        })
        .into_response(),
        Ok(RenameOutcome::NotFound) => category_not_found(),
        // Renommage vers un nom déjà pris dans le foyer (unicité CAT-004).
        Ok(RenameOutcome::Duplicate) => duplicate_category(),
        _ => problem_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            problem(500, "about:blank", "Internal Server Error"),
        ),
    }
}

/// Supprime une catégorie du foyer de l'appelant.
#[utoipa::path(
    delete,
    path = "/categories/{id}",
    operation_id = "deleteCategory",
    params(("id" = String, Path, description = "Identifiant (UUID) de la catégorie")),
    extensions(("x-requirements" = json!(["REQ-CAT-001"]))),
    responses(
        (status = 204, description = "Catégorie supprimée"),
        (status = 401, description = "Non authentifié", body = wallos_proto::Problem, content_type = "application/problem+json"),
        (status = 404, description = "Catégorie inconnue ou hors du foyer", body = wallos_proto::Problem, content_type = "application/problem+json")
    )
)]
#[requirement(REQ-CAT-001)]
pub async fn delete_category(
    AuthActor(actor): AuthActor,
    State(db): State<Db>,
    Path(id): Path<String>,
) -> Response {
    let Ok(category_id) = Uuid::parse_str(&id) else {
        return category_not_found();
    };
    match CategoryRepository::new(db.pool())
        .delete(&actor, category_id)
        .await
    {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => category_not_found(),
        _ => problem_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            problem(500, "about:blank", "Internal Server Error"),
        ),
    }
}
