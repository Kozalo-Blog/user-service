use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Initialize tracing subscriber with structured logging
///
/// This sets up tracing with environment-based filtering and formatted output.
/// To add OpenTelemetry OTLP export, upgrade dependencies:
/// - tonic to 0.11+ (currently 0.10.2 conflicts with newer opentelemetry crates)
/// - Add opentelemetry, opentelemetry_sdk, opentelemetry-otlp dependencies
/// - Add axum-tracing-opentelemetry and tonic-tracing-opentelemetry for automatic context propagation
pub fn init_tracing() -> Result<(), Box<dyn std::error::Error>> {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,user_service=debug"));

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer()
            .with_target(true)
            .with_thread_ids(true)
            .with_line_number(true))
        .with(env_filter)
        .try_init()?;

    tracing::info!("Tracing initialized successfully");
    Ok(())
}

/// Shutdown tracing provider gracefully
pub fn shutdown_tracing() {
    tracing::info!("Shutting down tracing");
}
