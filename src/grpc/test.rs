use std::net::SocketAddr;
use std::sync::Arc;
use std::time::SystemTime;
use anyhow::anyhow;
use chrono::{DateTime, Months, Timelike, Utc};
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
    assert_eq!(user.is_premium, false);
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
    assert_eq!(user.is_premium, true);
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

async fn start_test_server(repos: repo::Repositories) -> anyhow::Result<SocketAddr> {
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
