mod repo;
mod dto;
mod grpc;
mod rest;

use std::sync::Arc;
use axum::routing::get;
use axum_prometheus::PrometheusMetricLayer;
use prometheus::{Encoder, TextEncoder};
use tokio::join;
use tonic::transport::Server;
use crate::grpc::generated::user_service_server::UserServiceServer;
use crate::grpc::server::GrpcServer;

const AXUM_PORT: u16 = 8080;
const TONIC_PORT: u16 = 8090;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    pretty_env_logger::init();
    #[cfg(debug_assertions)] dotenvy::dotenv()?;
    autometrics::prometheus_exporter::init();

    let db_config = repo::DatabaseConfig::from_env()?;
    let db = repo::establish_database_connection(&db_config).await?;
    let rest_repos = Arc::new(repo::Repositories::new(db));
    let grpc_repos = rest_repos.clone();

    let rest_srv_handle = tokio::spawn(async move {
        run_rest_server(rest_repos).await
    });
    let grpc_srv_handle = tokio::spawn(async move {
        run_grpc_server(grpc_repos).await
    });

    let (rest_res, grpc_res) = join!(rest_srv_handle, grpc_srv_handle);
    (rest_res??, grpc_res??);
    Ok(())
}

async fn run_rest_server(repos: Arc<repo::Repositories>) -> anyhow::Result<()> {
    let prometheus = prometheus::Registry::new();
    let (prometheus_layer, metric_handle) = PrometheusMetricLayer::pair();

    let app = axum::Router::new()
        .nest("/api/rest/v1/user", rest::router(repos))
        .route("/metrics", get(|| async move {
            let auto_metrics = autometrics::prometheus_exporter::encode_to_string().unwrap();

            let mut buffer = vec![];
            let metrics = prometheus.gather();
            TextEncoder::new().encode(&metrics, &mut buffer).unwrap();
            let custom_metrics = String::from_utf8(buffer).unwrap();

            metric_handle.render() + &auto_metrics + &custom_metrics
        }))
        .layer(prometheus_layer);

    let listener = tokio::net::TcpListener::bind(("0.0.0.0", AXUM_PORT)).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn run_grpc_server(repos: Arc<repo::Repositories>) -> anyhow::Result<()> {
    Server::builder()
        .add_service(UserServiceServer::new(GrpcServer::new(repos)))
        .serve(([0,0,0,0], TONIC_PORT).into()).await?;
    Ok(())
}
