use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;
use axum::body::{Body, HttpBody};
use axum::response::Response;
use chrono::{Months, Timelike, Utc};
use http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::json;
use tower::ServiceExt;
use crate::dto::{Code, ExternalUser, SavedUser, Service, ServiceType};
use crate::repo::test::mocks::{mock_repositories, ServicesMock, CtorWithData, UsersMock, ExternalId};
use crate::{repo, rest};
use crate::repo::users::UserId;

struct UserServiceClient {
    router: axum::Router,
}

impl Default for UserServiceClient {
    fn default() -> Self {
        Self::new(mock_repositories())
    }
}

impl UserServiceClient {
    fn new(repos: repo::Repositories) -> Self {
        Self {
            router: rest::router(Arc::new(repos)),
        }
    }
}

impl UserServiceClient {
    async fn get_user(&self, uid: UserId) -> anyhow::Result<Response> {
        let app = self.router.clone();
        let path = match uid {
            UserId::Internal(internal_id) => format!("/{internal_id}"),
            UserId::External(external_id) => format!("/external/{external_id}"),
        };
        let response = app.oneshot(
            Request::builder()
                .method(http::Method::GET)
                .uri(path)
                .body(Body::empty())?
        ).await?;
        Ok(response)
    }

    async fn create_user(&self, user: &ExternalUser, service: &Service) -> anyhow::Result<Response> {
        let app = self.router.clone();
        let response = app.oneshot(
            Request::builder()
                .method(http::Method::PUT)
                .uri("/external")
                .header(http::header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "user": {
                            "external_id": user.external_id,
                            "name": user.name
                        },
                        "service": {
                            "name": service.name,
                            "type": service.service_type
                        }
                    }))?
                ))?
        ).await?;
        Ok(response)
    }

    async fn update_user_language(&self, user_id: i64, code: Code) -> anyhow::Result<Response> {
        let app = self.router.clone();
        let response = app.oneshot(
            Request::builder()
                .method(http::Method::POST)
                .uri(format!("/{user_id}/language/{code}"))
                .body(Body::empty())?
        ).await?;
        Ok(response)
    }

    async fn update_user_location(&self, user_id: i64,
                                  latitude: f64, longitude: f64) -> anyhow::Result<Response> {
        let app = self.router.clone();
        let response = app.oneshot(
            Request::builder()
                .method(http::Method::POST)
                .uri(format!("/{user_id}/location/?latitude={latitude}&longitude={longitude}"))
                .body(Body::empty())?
        ).await?;
        Ok(response)
    }

    async fn activate_user_premium(&self, user_id: i64, variant: &str) -> anyhow::Result<Response> {
        let app = self.router.clone();
        let response = app.oneshot(
            Request::builder()
                .method(http::Method::POST)
                .uri(format!("/{user_id}/premium/activate/{variant}"))
                .body(Body::empty())?
        ).await?;
        Ok(response)
    }
}

#[tokio::test]
async fn test_get_and_create() -> anyhow::Result<()> {
    pretty_env_logger::init();

    let client = UserServiceClient::default();
    let external_user = ExternalUser {
        external_id: 1234567890,
        name: Some("SadBot".to_owned()),
    };
    let service = Service {
        name: "SadFavBot".to_string(),
        service_type: ServiceType::TelegramBot,
    };

    log::info!("ensure nobody is in the database");
    let response = client.get_user(UserId::Internal(1)).await?;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let response = client.get_user(UserId::External(external_user.external_id)).await?;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    log::info!("create the first user");
    let response = client.create_user(&external_user, &service).await?;
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = to_json_value(response).await?;
    assert_eq!(body, json!({
        "status": "created",
        "id": 1
    }));

    log::info!("try to create the same user again");
    let response = client.create_user(&external_user, &service).await?;
    assert_eq!(response.status(), StatusCode::FOUND);
    let body = to_json_value(response).await?;
    assert_eq!(body, json!({
        "status": "already_present",
        "id": 1
    }));

    log::info!("test the output of the GET method");
    let response = client.get_user(UserId::Internal(1)).await?;
    assert_eq!(response.status(), StatusCode::OK);
    let response = client.get_user(UserId::External(external_user.external_id)).await?;
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_json_value(response).await?;
    assert_eq!(body, json!({
        "id": 1,
        "name": external_user.name.unwrap(),
        "options": {
            "language_code": null,
            "location": null
        },
        "is_premium": false
    }));

    Ok(())
}

#[tokio::test]
async fn test_updates() -> anyhow::Result<()> {
    let client = UserServiceClient::new(build_repos_with_test_user());
    let username = build_external_user().name.unwrap();
    let (latitude, longitude) = (12.345, 67.89);

    let response = client.update_user_language(1, "ru".try_into()?).await?;
    ensure_success(response).await?;

    let response = client.update_user_location(1, latitude, longitude).await?;
    ensure_success(response).await?;

    let response = client.activate_user_premium(1, "week").await?;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let response = client.activate_user_premium(1, "month").await?;
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_json_value(response).await?;
    let date_part_str_range = 0..10;
    let date_in_month = &Utc::now()
        .checked_add_months(Months::new(1))
        .unwrap()
        .with_nanosecond(0)
        .unwrap()
        .to_string()
        [date_part_str_range.clone()];
    assert_eq!(body["success"], true);
    let active_till = body["active_till"].as_str()
        .map(|s| &s[date_part_str_range])
        .expect("active_till must be present here");
    assert_eq!(active_till, date_in_month);

    let response = client.get_user(UserId::Internal(1)).await?;
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_json_value(response).await?;
    assert_eq!(body, json!({
        "id": 1,
        "name": username,
        "options": {
            "language_code": "ru",
            "location": {
                "latitude": latitude,
                "longitude": longitude
            }
        },
        "is_premium": true
    }));

    Ok(())
}

async fn to_json_value<T>(response: http::Response<T>) -> anyhow::Result<serde_json::Value>
where
    T: HttpBody,
    <T as HttpBody>::Error: Error + Send + Sync + 'static
{
    let body = response.into_body().collect().await?.to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&body)?;
    Ok(body)
}

async fn ensure_success<T>(response: http::Response<T>) -> anyhow::Result<()>
    where
        T: HttpBody,
        <T as HttpBody>::Error: Error + Send + Sync + 'static
{
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_json_value(response).await?;
    assert_eq!(body, json!({"success": true}));
    Ok(())
}

fn build_repos_with_test_user() -> repo::Repositories {
    let usr = build_external_user();
    let external_id = usr.external_id as ExternalId;
    let usr = SavedUser {
        id: 1,
        name: usr.name,
        language_code: None,
        location: None,
        premium_till: None,
    };

    let services = ServicesMock::with_data(HashMap::from([(1, build_service())]));
    let users = UsersMock::with_data(HashMap::from([(external_id, usr)]));
    repo::Repositories {
        services: Box::new(services),
        users: Box::new(users),
    }
}

fn build_external_user() -> ExternalUser {
    ExternalUser {
        external_id: 1234567890,
        name: Some("SadBot".to_owned()),
    }
}

fn build_service() -> Service {
    Service {
        name: "SadFavBot".to_string(),
        service_type: ServiceType::TelegramBot,
    }
}
