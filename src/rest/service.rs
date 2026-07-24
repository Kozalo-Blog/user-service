use std::str::FromStr;
use std::sync::Arc;
use axum::{Extension, Json};
use axum::extract::{Path, Query};
use axum::routing::{get, patch, post};
use axum_route_error::RouteError;
use axum::http::StatusCode;
use crate::dto::{Code, Location, RegistrationResponse, RegistrationStatus};
use crate::rest::error::RestErrorExt;
use crate::repo;
use crate::repo::users::{UpdateTarget, UserId, Users};
use crate::repo::services::Services;
use crate::rest::{SearchParams, PremiumActivationResult, PremiumVariantRest, RegistrationRequest, RestError, Success, UserView};

/// JSON response group names for `GET /search` — users looked up by internal vs external id.
const BY_ID_GROUP: &str = "users_by_id";
const BY_EXTERNAL_ID_GROUP: &str = "users_by_external_id";

pub fn router<U, S>(repos: Arc<repo::Repositories<U, S>>) -> axum::Router
where
    U: Users + Send + Sync + 'static,
    S: Services + Send + Sync + 'static,
{
    axum::Router::new()
        .route("/{id}", get(get_user::<U, S>))
        .route("/external/{external_id}", get(get_external_user::<U, S>))
        .route("/search", get(search_users::<U, S>))
        .route("/external", post(register_user::<U, S>))
        .route("/{id}/language/{code}", patch(update_language::<U, S>))
        .route("/{id}/location/", patch(update_location::<U, S>))
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

/// A user serialized to JSON, tagged with the [`UserId`] it was requested under.
type KeyedUser = (UserId, serde_json::Value);

/// Parses a comma-separated id list (from a query parameter) into numeric ids, ignoring blanks.
fn parse_ids(raw: &Option<String>) -> Result<Vec<i64>, std::num::ParseIntError> {
    raw.as_deref().unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::parse::<i64>)
        .collect()
}

#[tracing::instrument(skip(repos, params), fields(id = ?params.id, external_id = ?params.external_id))]
async fn search_users<U, S>(
    Extension(repos): Extension<Arc<repo::Repositories<U, S>>>,
    Query(params): Query<SearchParams>,
) -> Result<Json<serde_json::Value>, RouteError<RestError>>
where
    U: Users,
    S: Services,
{
    let internal_ids = parse_ids(&params.id)
        .log_route_warn("Invalid 'id' query parameter")?;
    let external_ids = parse_ids(&params.external_id)
        .log_route_warn("Invalid 'external_id' query parameter")?;
    let ids: Vec<UserId> = internal_ids.into_iter().map(UserId::Internal)
        .chain(external_ids.into_iter().map(UserId::External))
        .collect();

    let paths: Option<Vec<String>> = params.fields
        .filter(|f| !f.trim().is_empty())
        .map(|f| f.split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect());

    let users = repos.users.get_many(ids).await
        .log_route_error("Failed to fetch users")?;

    // Split the results into two id-keyed objects: { "internal": {..}, "external": {..} }.
    let (internal, external): (Vec<KeyedUser>, Vec<KeyedUser>) = users.into_iter()
        .map(|(key, user)| {
            let mut value = serde_json::to_value(UserView::from(user))?;
            if let Some(paths) = &paths {
                project_value(&mut value, paths);
            }
            Ok((key, value))
        })
        .collect::<Result<Vec<_>, serde_json::Error>>()
        .log_route_error("Failed to serialize users")?
        .into_iter()
        .partition(|(key, _)| matches!(key, UserId::Internal(_)));

    let to_object = |entries: Vec<KeyedUser>| serde_json::Value::Object(
        entries.into_iter()
            .map(|(key, value)| (key.value().to_string(), value))
            .collect()
    );
    // Omit a group entirely when it has no results, rather than emitting an empty object.
    let response: serde_json::Map<String, serde_json::Value> = [(BY_ID_GROUP, internal), (BY_EXTERNAL_ID_GROUP, external)]
        .into_iter()
        .filter(|(_, entries)| !entries.is_empty())
        .map(|(name, entries)| (name.to_owned(), to_object(entries)))
        .collect();
    Ok(Json(serde_json::Value::Object(response)))
}

/// Prunes a JSON value in place, keeping only object keys named in `paths` (and, for keys with
/// nested selections like `options.language_code`, recursing into the selected sub-paths). Mirrors
/// the gRPC-side `project_user` semantics for `FieldMask` projections.
fn project_value(value: &mut serde_json::Value, paths: &[String]) {
    let serde_json::Value::Object(map) = value else { return };
    map.retain(|key, child| {
        if paths.iter().any(|p| p == key) {
            return true;
        }
        let prefix = format!("{key}.");
        let sub_paths: Vec<String> = paths.iter()
            .filter_map(|p| p.strip_prefix(&prefix).map(String::from))
            .collect();
        if sub_paths.is_empty() {
            false
        } else {
            project_value(child, &sub_paths);
            true
        }
    });
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
