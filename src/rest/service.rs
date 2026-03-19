use std::str::FromStr;
use std::sync::Arc;
use axum::{Extension, Json};
use axum::extract::{Path, Query};
use axum::routing::{get, post, put};
use axum_route_error::RouteError;
use axum::http::StatusCode;
use crate::dto::{Code, Location, RegistrationResponse, RegistrationStatus};
use crate::rest::error::RestErrorExt;
use crate::repo;
use crate::repo::users::{UpdateTarget, UserId, Users};
use crate::repo::services::Services;
use crate::rest::{PremiumActivationResult, PremiumVariantRest, RegistrationRequest, RestError, Success, UserView};

pub fn router<U, S>(repos: Arc<repo::Repositories<U, S>>) -> axum::Router
where
    U: Users + Send + Sync + 'static,
    S: Services + Send + Sync + 'static,
{
    axum::Router::new()
        .route("/{id}", get(get_user::<U, S>))
        .route("/external/{external_id}", get(get_external_user::<U, S>))
        .route("/external", put(register_user::<U, S>))
        .route("/{id}/language/{code}", post(update_language::<U, S>))
        .route("/{id}/location/", post(update_location::<U, S>))
        .route("/{id}/premium/activate/{variant}", post(activate_premium::<U, S>))
        .layer(Extension(repos))
}

#[tracing::instrument(skip(repos), fields(user_id = %id))]
async fn get_user<U, S>(
    Extension(repos): Extension<Arc<repo::Repositories<U, S>>>,
    Path(id): Path<i64>,
) -> Result<Json<UserView>, RouteError>
where
    U: Users,
    S: Services,
{
    get_user_impl(repos, UserId::Internal(id)).await
}

#[tracing::instrument(skip(repos), fields(external_id = %id))]
async fn get_external_user<U, S>(
    Extension(repos): Extension<Arc<repo::Repositories<U, S>>>,
    Path(id): Path<i64>,
) -> Result<Json<UserView>, RouteError>
where
    U: Users,
    S: Services,
{
    get_user_impl(repos, UserId::External(id)).await
}

async fn get_user_impl<U, S>(
    repos: Arc<repo::Repositories<U, S>>,
    id: UserId,
) -> Result<Json<UserView>, RouteError>
where
    U: Users,
    S: Services,
{
    let user = repos.users.get(id)
        .await?
        .ok_or(RouteError::new_not_found())?
        .into();
    Ok(Json(user))
}

#[tracing::instrument(skip(repos, req), fields(external_id = %req.user.external_id, service_type = ?req.service.service_type))]
async fn register_user<U, S>(
    Extension(repos): Extension<Arc<repo::Repositories<U, S>>>,
    Json(req): Json<RegistrationRequest>,
) -> Result<(StatusCode, Json<RegistrationResponse>), RouteError<RestError>>
where
    U: Users,
    S: Services,
{
    let maybe_service = repos.services.get_id(&req.service).await
        .log_route_error("Failed to get service ID")?;
    let service_id = match maybe_service {
        Some(id) => id,
        None => repos.services.create(req.service.service_type, &req.service.name).await?
    };

    let user_id = repos.users.get_user_id(service_id, req.user.external_id).await
        .log_route_error("Failed to get user ID")?;
    let status = match user_id {
        Some(id) => {
            tracing::info!(user_id = %id, "User already registered");
            (StatusCode::FOUND, RegistrationStatus::AlreadyPresent.with_id(id))
        }
        None => {
            let id = repos.users.register(req.user, service_id, req.consent_info).await
                .log_route_error("Failed to register user")?;
            tracing::info!(user_id = %id, "User registered successfully");
            (StatusCode::CREATED, RegistrationStatus::Created.with_id(id))
        }
    };
    Ok((status.0, Json(status.1)))
}

#[tracing::instrument(skip(repos), fields(user_id = %id, language_code = %code))]
async fn update_language<U, S>(
    Extension(repos): Extension<Arc<repo::Repositories<U, S>>>,
    Path((id, code)): Path<(i64, String)>,
) -> Result<Success, RouteError<RestError>>
where
    U: Users,
    S: Services,
{
    let lang_code: Code = code.try_into()
        .log_route_warn("Invalid language code format")?;
    update_impl(repos, id, lang_code.into()).await
}

#[tracing::instrument(skip(repos), fields(user_id = %id, lat = %location.latitude, lon = %location.longitude))]
async fn update_location<U, S>(
    Extension(repos): Extension<Arc<repo::Repositories<U, S>>>,
    Path(id): Path<i64>,
    Query(location): Query<Location>,
) -> Result<Success, RouteError<RestError>>
where
    U: Users,
    S: Services,
{
    location.validate()
        .log_route_warn("Invalid location coordinates")?;
    update_impl(repos, id, location.into()).await
}

async fn update_impl<U, S>(repos: Arc<repo::Repositories<U, S>>, id: i64, target: UpdateTarget) -> Result<Success, RouteError<RestError>>
where
    U: Users,
    S: Services,
{
    repos.users.update_value(id, target).await
        .log_route_error("Failed to update user")?;
    Ok(Success)
}

#[tracing::instrument(skip(repos), fields(user_id = %id, variant = %till))]
async fn activate_premium<U, S>(
    Extension(repos): Extension<Arc<repo::Repositories<U, S>>>,
    Path((id, till)): Path<(i64, String)>,
) -> Result<Json<PremiumActivationResult>, RouteError<RestError>>
where
    U: Users,
    S: Services,
{
    let variant = PremiumVariantRest::from_str(&till)
        .log_route_warn("Invalid premium variant")?;
    let activation_result = repos.users.activate_premium(id, variant.into()).await
        .log_route_error("Failed to activate premium")?;
    tracing::info!(?activation_result, "Premium activation completed");
    Ok(Json(PremiumActivationResult::from(activation_result)))
}
