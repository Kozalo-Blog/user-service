mod repo;
mod dto;

use std::sync::Arc;
use axum::routing::{get, post, put};
use axum::{Extension, Json};
use axum::extract::{Path, Query};
use axum_prometheus::PrometheusMetricLayer;
use axum_route_error::RouteError;
use prometheus::{Encoder, TextEncoder};
use crate::dto::{UserView, error::CodeStringLengthError, RegistrationRequest, RegistrationResponse, RegistrationStatus, Success, Code};
use crate::repo::users::UserId;
use crate::repo::error::{DatabaseError, RepoError};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(debug_assertions)]
    dotenvy::dotenv()?;

    pretty_env_logger::init();

    let db_config = repo::DatabaseConfig::from_env()?;
    let db = repo::establish_database_connection(&db_config).await?;
    let repos = Arc::new(repo::Repositories::new(db));

    let prometheus = prometheus::Registry::new();
    let (prometheus_layer, metric_handle) = PrometheusMetricLayer::pair();

    let app = axum::Router::new()
        .route("/api/v1/user/:id", get(get_user))
        .route("/api/v1/user/external/:external_id", get(get_external_user))
        .route("/api/v1/user/external", put(register_user))
        .route("/api/v1/user/:id/language/:code", post(update_language))
        .route("/api/v1/user/:id/location/", post(update_location))
        .route("/api/v1/user/:id/premium/activate", post(activate_premium))
        .layer(Extension(repos))
        .route("/metrics", get(|| async move {
            let mut buffer = vec![];
            let metrics = prometheus.gather();
            TextEncoder::new().encode(&metrics, &mut buffer).unwrap();
            let custom_metrics = String::from_utf8(buffer).unwrap();

            metric_handle.render() + custom_metrics.as_str()
        }))
        .layer(prometheus_layer);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;
    axum::serve(listener, app).await?;
    Ok(())
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
