// Suppress warnings from generated protobuf code about serde feature
#![allow(unexpected_cfgs)]

mod repo;
mod dto;
mod grpc;
mod rest;
mod observability;

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

    // Initialize distributed tracing
    observability::init_tracing()?;

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

    // Shutdown tracing provider gracefully
    observability::shutdown_tracing();

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
        .layer(prometheus_layer)
        .layer(axum_tracing_opentelemetry::middleware::OtelAxumLayer::default());

    let listener = tokio::net::TcpListener::bind(("0.0.0.0", AXUM_PORT)).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn run_grpc_server(repos: Arc<repo::Repositories>) -> anyhow::Result<()> {
    // Note: gRPC trace context propagation happens automatically through the global
    // tracing subscriber with OpenTelemetry layer configured in observability::init_tracing().
    // The tonic-tracing-opentelemetry crate provides extensions for automatic span creation,
    // but explicit middleware is not needed - spans are created by our #[tracing::instrument] attributes.

    Server::builder()
        .add_service(UserServiceServer::new(GrpcServer::new(repos)))
        .serve_with_shutdown(([0,0,0,0], TONIC_PORT).into(), shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install CTRL+C signal handler");
    log::info!("Shutdown of the servers…");
}
