use std::net::SocketAddr;
use std::sync::Arc;
use std::time::SystemTime;
use anyhow::anyhow;
use chrono::{DateTime, Months, Timelike, Utc};
use opentelemetry::trace::SpanId;
use tracing::subscriber::set_default;
use serde_json::json;
use tokio::net::TcpListener;
use tonic::Code;
use tonic::transport::{Channel, Server};
use crate::grpc::generated::{ActivatePremiumRequest, ExternalUser, GetUserRequest, Location, PremiumVariant, RegistrationRequest, RegistrationStatus, Service, ServiceType, UpdateUserRequest};
use crate::grpc::generated::update_user_request::Target;
use crate::grpc::generated::user_service_client::UserServiceClient;
use crate::grpc::generated::user_service_server::UserServiceServer;
use crate::grpc::server::GrpcServer;
use crate::repo;
use crate::repo::test::mocks::mock_repositories;
use crate::repo::test::otel::setup_otel_test;
use crate::repo::users::Users;
use crate::repo::services::Services;

#[tokio::test]
async fn test_all() -> anyhow::Result<()> {
    let addr = start_test_server(mock_repositories()).await?;
    let mut client = UserServiceClient::connect(format!("http://{}", addr)).await?;

    let (ext_id, username, service_name) = (12345, "SadBot".to_owned(), "SadFavBot".to_owned());
    let get_req_by_internal_id = GetUserRequest {
        id: 1,
        by_external_id: false,
    };
    let get_req_by_external_id = GetUserRequest {
        id: ext_id,
        by_external_id: true,
    };
    test_get_not_found(&mut client, get_req_by_internal_id.clone()).await;
    test_get_not_found(&mut client, get_req_by_external_id.clone()).await;

    let registration_req = RegistrationRequest {
        user: Some(ExternalUser {
            external_id: ext_id,
            name: Some(username.clone()),
        }),
        service: Some(Service {
            name: service_name.clone(),
            kind: ServiceType::TelegramBot.into(),
        }),
        consent_info: Some(serde_json::from_value(json!({"test": true}))?),
    };
    test_registration(&mut client, registration_req.clone(), RegistrationStatus::Created).await?;
    test_registration(&mut client, registration_req, RegistrationStatus::AlreadyPresent).await?;

    let user = client.get(get_req_by_internal_id.clone()).await?.into_inner();
    assert!(!user.is_premium);
    let opts = user.options.unwrap();
    assert_eq!(opts.language_code, None);
    assert_eq!(opts.location, None);

    let (lang, latitude, longitude) = ("ru".to_owned(), 12.345, 67.890);
    client.update(UpdateUserRequest {
        id: 1,
        target: Some(Target::Language(lang.clone())),
    }).await?;
    client.update(UpdateUserRequest {
        id: 1,
        target: Some(Target::Location(Location { latitude, longitude })),
    }).await?;

    let month_later = Utc::now().with_nanosecond(0).unwrap()
        .checked_add_months(Months::new(1)).unwrap();
    let resp = client.activate_premium(ActivatePremiumRequest {
        id: 1,
        variant: PremiumVariant::Month as i32,
    }).await?.into_inner();
    let till: SystemTime = resp.active_till
        .ok_or(anyhow!("active_till must be present in the response"))?
        .try_into()?;
    let till: DateTime<Utc> = till.into();
    let till = till.with_nanosecond(0).unwrap();
    assert_eq!(till, month_later);

    let user = client.get(get_req_by_internal_id).await?.into_inner();
    assert_eq!(user.id, 1);
    assert_eq!(user.name, Some(username));
    assert!(user.is_premium);
    let opts = &user.options.unwrap();
    assert_eq!(opts.language_code, Some(lang));
    assert_eq!(opts.location, Some(Location { latitude, longitude }));

    Ok(())
}

#[tokio::test]
async fn invalid_arguments() -> anyhow::Result<()> {
    let addr = start_test_server(mock_repositories()).await?;
    let mut client = UserServiceClient::connect(format!("http://{}", addr)).await?;

    let resp = client.update(UpdateUserRequest {
        id: 1,
        target: None,
    }).await;
    assert_eq!(resp.err().map(|status| status.code()), Some(Code::InvalidArgument));

    let resp = client.activate_premium(ActivatePremiumRequest {
        id: 1,
        variant: 0,
    }).await;
    assert_eq!(resp.err().map(|status| status.code()), Some(Code::InvalidArgument));

    Ok(())
}

async fn start_test_server<U, S>(repos: repo::Repositories<U, S>) -> anyhow::Result<SocketAddr>
where
    U: Users + Send + Sync + 'static,
    S: Services + Send + Sync + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;

    tokio::spawn(async move {
        Server::builder()
            .add_service(UserServiceServer::new(GrpcServer::new(Arc::new(repos))))
            .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener))
            .await
            .expect("couldn't start a gRPC server");
    });

    Ok(addr)
}

async fn test_get_not_found(client: &mut UserServiceClient<Channel>, request: GetUserRequest) {
    let resp = client.get(request).await;
    assert!(resp.is_err());
    assert_eq!(resp.unwrap_err().code(), Code::NotFound);
}

async fn test_registration(client: &mut UserServiceClient<Channel>, request: RegistrationRequest, status: RegistrationStatus) -> anyhow::Result<()> {
    let resp = client.register(request).await?.into_inner();
    assert_eq!(resp.id, 1);
    assert_eq!(resp.status, status as i32);
    Ok(())
}

#[tokio::test]
async fn test_span_hierarchy() -> anyhow::Result<()> {
    let (exporter, provider, subscriber) = setup_otel_test();
    let _guard = set_default(subscriber);

    let addr = start_test_server(mock_repositories()).await?;
    let mut client = UserServiceClient::connect(format!("http://{}", addr)).await?;

    // Call get — expect NotFound, but spans should still be created
    let _ = client.get(GetUserRequest { id: 1, by_external_id: false }).await;

    let _ = provider.force_flush();
    let spans = exporter.get_finished_spans().expect("Failed to get finished spans");

    println!("=== gRPC spans ({}) ===", spans.len());
    for span in &spans {
        println!(
            "  name={:?} trace_id={:?} span_id={:?} parent_span_id={:?}",
            span.name,
            span.span_context.trace_id(),
            span.span_context.span_id(),
            span.parent_span_id
        );
    }

    // We expect at least 2 spans: the handler "get" and the repo "get"
    assert!(spans.len() >= 2, "Expected at least 2 spans, got {}", spans.len());

    let handler_span = spans.iter()
        .find(|s| s.name == "get" && s.parent_span_id == SpanId::INVALID)
        .expect("Should have a root handler 'get' span");
    let repo_span = spans.iter()
        .find(|s| s.name == "get" && s.span_context.span_id() != handler_span.span_context.span_id())
        .expect("Should have a repo 'get' span");

    // Same trace
    assert_eq!(
        handler_span.span_context.trace_id(),
        repo_span.span_context.trace_id(),
        "Handler and repo spans must share the same trace_id"
    );

    // Repo span is a child of the handler span
    assert_eq!(
        repo_span.parent_span_id,
        handler_span.span_context.span_id(),
        "Repo span's parent_span_id must equal handler span's span_id"
    );

    Ok(())
}
