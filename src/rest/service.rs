use std::str::FromStr;
use std::sync::Arc;
use axum::{Extension, Json};
use axum::extract::{Path, Query};
use axum::routing::{get, post, put};
use axum_route_error::RouteError;
use axum::http::StatusCode;
use crate::dto::{Code, Location, RegistrationResponse, RegistrationStatus};
use crate::dto::error::CodeStringLengthError;
use crate::repo;
use crate::repo::users::{UpdateTarget, UserId};
use crate::rest::{PremiumActivationResult, PremiumVariantRest, RegistrationRequest, RestError, Success, UserView};

pub fn router(repos: Arc<repo::Repositories>) -> axum::Router {
    axum::Router::new()
        .route("/:id", get(get_user))
        .route("/external/:external_id", get(get_external_user))
        .route("/external", put(register_user))
        .route("/:id/language/:code", post(update_language))
        .route("/:id/location/", post(update_location))
        .route("/:id/premium/activate/:variant", post(activate_premium))
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
) -> Result<(StatusCode, Json<RegistrationResponse>), RouteError<RestError>> {
    let maybe_service = repos.services.get_id(&req.service).await
        .map_err(|e| RouteError::new_internal_server().set_error_data(e.into()))?;
    let service_id = match maybe_service {
        Some(id) => id,
        None => repos.services.create(req.service.service_type, &req.service.name).await?
    };

    let user_id = repos.users.get_user_id(service_id, req.user.external_id).await
        .map_err(|e| RouteError::new_internal_server().set_error_data(e.into()))?;
    let status = match user_id {
        Some(id) => (StatusCode::FOUND, RegistrationStatus::AlreadyPresent.with_id(id)),
        None => {
            let id = repos.users.register(req.user, service_id, req.consent_info).await
                .map_err(|e| RouteError::new_internal_server().set_error_data(e.into()))?;
            (StatusCode::CREATED, RegistrationStatus::Created.with_id(id))
        }
    };
    Ok((status.0, Json(status.1)))
}

async fn update_language(
    Extension(repos): Extension<Arc<repo::Repositories>>,
    Path((id, code)): Path<(i64, String)>,
) -> Result<Success, RouteError<RestError>> {
    let lang_code: Code = code.try_into()
        .map_err(|e: CodeStringLengthError| RouteError::new_bad_request().set_error_data(e.into()))?;
    update_impl(repos, id, lang_code.into()).await
}

async fn update_location(
    Extension(repos): Extension<Arc<repo::Repositories>>,
    Path(id): Path<i64>,
    Query(location): Query<Location>,
) -> Result<Success, RouteError<RestError>> {
    update_impl(repos, id, location.into()).await
}

async fn update_impl(repos: Arc<repo::Repositories>, id: i64, target: UpdateTarget) -> Result<Success, RouteError<RestError>> {
    repos.users.update_value(id, target).await
        .map_err(|e| RouteError::new_internal_server().set_error_data(e.into()))?;
    Ok(Success)
}

async fn activate_premium(
    Extension(repos): Extension<Arc<repo::Repositories>>,
    Path((id, till)): Path<(i64, String)>,
) -> Result<Json<PremiumActivationResult>, RouteError<RestError>> {
    let variant = PremiumVariantRest::from_str(&till)
        .map_err(|e| RouteError::new_bad_request().set_error_data(e.into()))?;
    let activation_result = repos.users.activate_premium(id, variant.into()).await
        .map_err(|e| RouteError::new_internal_server().set_error_data(e.into()))?;
    Ok(Json(PremiumActivationResult::from(activation_result)))
}
