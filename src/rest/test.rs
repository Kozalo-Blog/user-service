use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;
use axum::body::{Body, HttpBody};
use axum::response::Response;
use axum_tracing_opentelemetry::middleware::{OtelAxumLayer, OtelInResponseLayer};
use chrono::{Months, Timelike, Utc};
use http::{Request, StatusCode};
use http_body_util::BodyExt;
use opentelemetry::trace::SpanId;
use tracing::subscriber::set_default;
use serde_json::json;
use tower::ServiceExt;
use crate::dto::{Code, ExternalUser, SavedUser, Service, ServiceType};
use crate::repo::test::mocks::{mock_repositories, ServicesMock, CtorWithData, UsersMock, ExternalId, MockRepositories};
use crate::repo::test::otel::setup_otel_test;
use crate::{repo, rest};
use crate::repo::users::{UserId, Users};
use crate::repo::services::Services;

struct UserServiceClient {
    router: axum::Router,
}

impl Default for UserServiceClient {
    fn default() -> Self {
        Self::new(mock_repositories())
    }
}

impl UserServiceClient {
    fn new<U, S>(repos: repo::Repositories<U, S>) -> Self
    where
        U: Users + Send + Sync + 'static,
        S: Services + Send + Sync + 'static,
    {
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
                .method(http::Method::POST)
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
                        },
                        "consent_info": {"test": true}
                    }))?
                ))?
        ).await?;
        Ok(response)
    }

    async fn update_user_language(&self, user_id: i64, code: Code) -> anyhow::Result<Response> {
        let app = self.router.clone();
        let response = app.oneshot(
            Request::builder()
                .method(http::Method::PATCH)
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
                .method(http::Method::PATCH)
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
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();

    let client = UserServiceClient::default();
    let external_user = ExternalUser {
        external_id: 1234567890,
        name: Some("SadBot".to_owned()),
    };
    let service = Service {
        name: "SadFavBot".to_string(),
        service_type: ServiceType::TelegramBot,
    };

    tracing::info!("ensure nobody is in the database");
    let response = client.get_user(UserId::Internal(1)).await?;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let response = client.get_user(UserId::External(external_user.external_id)).await?;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    tracing::info!("create the first user");
    let response = client.create_user(&external_user, &service).await?;
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = to_json_value(response).await?;
    assert_eq!(body, json!({
        "status": "created",
        "id": 1
    }));

    tracing::info!("try to create the same user again");
    let response = client.create_user(&external_user, &service).await?;
    assert_eq!(response.status(), StatusCode::FOUND);
    let body = to_json_value(response).await?;
    assert_eq!(body, json!({
        "status": "already_present",
        "id": 1
    }));

    tracing::info!("test the output of the GET method");
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

fn build_repos_with_test_user() -> MockRepositories {
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
    repo::Repositories::new(users, services)
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

#[tokio::test]
async fn test_span_hierarchy() -> anyhow::Result<()> {
    let (exporter, provider, subscriber) = setup_otel_test();
    let _guard = set_default(subscriber);

    let repos = Arc::new(mock_repositories());
    let app = rest::router(repos)
        .layer(OtelInResponseLayer)
        .layer(OtelAxumLayer::default());

    let response = app.oneshot(
        Request::builder()
            .method(http::Method::GET)
            .uri("/1")
            .body(Body::empty())?
    ).await?;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let _ = provider.force_flush();
    let spans = exporter.get_finished_spans().expect("Failed to get finished spans");

    println!("=== REST spans ({}) ===", spans.len());
    for span in &spans {
        println!(
            "  name={:?} trace_id={:?} span_id={:?} parent_span_id={:?}",
            span.name,
            span.span_context.trace_id(),
            span.span_context.span_id(),
            span.parent_span_id
        );
    }

    // We expect at least 2 spans: OTel middleware + handler get_user (+ possibly repo get)
    assert!(spans.len() >= 2, "Expected at least 2 spans, got {}", spans.len());

    // All spans should share the same trace_id
    let trace_id = spans[0].span_context.trace_id();
    for span in &spans {
        assert_eq!(
            span.span_context.trace_id(), trace_id,
            "All spans must share the same trace_id, but span {:?} has a different one",
            span.name
        );
    }

    // Find the root span (OTel middleware)
    let root_span = spans.iter()
        .find(|s| s.parent_span_id == SpanId::INVALID)
        .expect("Should have a root span (OTel middleware)");
    println!("Root span: {:?}", root_span.name);

    // Find the handler span — it should be a child of the root
    let handler_span = spans.iter()
        .find(|s| s.name == "get_user")
        .expect("Should have a handler 'get_user' span");
    assert_eq!(
        handler_span.parent_span_id,
        root_span.span_context.span_id(),
        "Handler span must be a child of the OTel middleware root span"
    );

    // If repo span exists, it should be a child of the handler span
    if let Some(repo_span) = spans.iter().find(|s| s.name == "get") {
        assert_eq!(
            repo_span.parent_span_id,
            handler_span.span_context.span_id(),
            "Repo span's parent_span_id must equal handler span's span_id"
        );
    }

    Ok(())
}
