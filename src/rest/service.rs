use std::sync::Arc;
use axum::{Extension, Json};
use axum::extract::{Path, Query};
use axum::routing::{get, post, put};
use axum_route_error::RouteError;
use crate::dto::{Code, RegistrationResponse, RegistrationStatus};
use crate::dto::error::CodeStringLengthError;
use crate::repo;
use crate::repo::error::{DatabaseError, RepoError};
use crate::repo::users::UserId;
use crate::rest::{RegistrationRequest, Success, UserView};

pub fn router(repos: Arc<repo::Repositories>) -> axum::Router {
    axum::Router::new()
        .route("/:id", get(get_user))
        .route("/external/:external_id", get(get_external_user))
        .route("/external", put(register_user))
        .route("/:id/language/:code", post(update_language))
        .route("/:id/location/", post(update_location))
        .route("/:id/premium/activate", post(activate_premium))
        .layer(Extension(repos))
}

async fn get_user(
    Extension(repos): Extension<Arc<repo::Repositories>>,
    Path(id): Path<i64>,
) -> Result<Json<UserView>, RouteError> {
    get_user_impl(repos, UserId::Internal(id)).await
}

async fn get_external_user(
    Extension(repos): Extension<Arc<repo::Repositories>>,
    Path(id): Path<i64>,
) -> Result<Json<UserView>, RouteError> {
    get_user_impl(repos, UserId::External(id)).await
}

async fn get_user_impl(
    repos: Arc<repo::Repositories>,
    id: UserId,
) -> Result<Json<UserView>, RouteError> {
    let user = repos.users.get(id)
        .await?
        .ok_or(RouteError::new_not_found())?
        .into();
    Ok(Json(user))
}

async fn register_user(
    Extension(repos): Extension<Arc<repo::Repositories>>,
    Json(req): Json<RegistrationRequest>,
) -> Result<Json<RegistrationResponse>, DatabaseError> {
    let service_id = match repos.services.get_id(&req.service).await? {
        Some(id) => id,
        None => repos.services.create(req.service.service_type, &req.service.name).await?
    };

    let user_id = repos.users.get_user_id(service_id, req.user.external_id).await?;
    let status = match user_id {
        Some(id) => RegistrationStatus::AlreadyPresent.with_id(id),
        None => {
            let id = repos.users.register(req.user, service_id).await?;
            RegistrationStatus::Created.with_id(id)
        }
    };
    Ok(Json(status))
}

async fn update_language(
    Extension(repos): Extension<Arc<repo::Repositories>>,
    Path((id, code)): Path<(i64, String)>,
) -> Result<Success, Json<RepoError<CodeStringLengthError>>> {
    let lang_code: Code = code.try_into()
        .map_err(|e| RepoError::Other(e))?;
    repos.users.update_value(id, lang_code.into())
        .await
        .map_err(|e| RepoError::Database(e.into()))?;
    Ok(Success)
}

async fn update_location(
    Extension(repos): Extension<Arc<repo::Repositories>>,
    Path(id): Path<i64>,
    Query(location): Query<(f64, f64)>,
) -> Result<Success, Json<RepoError<CodeStringLengthError>>> {
    repos.users.update_value(id, location.into())
        .await
        .map_err(|e| RepoError::Database(e.into()))?;
    Ok(Success)
}

async fn activate_premium(
    Extension(repos): Extension<Arc<repo::Repositories>>,
    Path(id): Path<i64>,
    Query(till): Query<chrono::DateTime<chrono::Utc>>,
) -> Result<Success, Json<RepoError<CodeStringLengthError>>> {
    repos.users.update_value(id, till.into())
        .await
        .map_err(|e| RepoError::Database(e.into()))?;
    Ok(Success)
}
