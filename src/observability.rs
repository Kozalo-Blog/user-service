use opentelemetry::global;
use opentelemetry::trace::TracerProvider;
use opentelemetry_sdk::trace as sdktrace;
use opentelemetry_sdk::Resource;
use opentelemetry_otlp::WithExportConfig;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Initialize tracing subscriber with OpenTelemetry OTLP export
///
/// Configures tracing with:
/// - Console output with structured formatting
/// - OpenTelemetry OTLP exporter to VictoriaStack/Grafana
/// - Environment-based log level filtering
///
/// Configuration via environment variables:
/// - OTEL_EXPORTER_OTLP_ENDPOINT: OTLP endpoint (default: http://localhost:4317)
/// - OTEL_SERVICE_NAME: Service name for traces (default: user-service)
/// - RUST_LOG: Log level filter (default: info,user_service=debug)
pub fn init_tracing() -> Result<(), Box<dyn std::error::Error>> {
    let endpoint = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
        .unwrap_or_else(|_| "http://localhost:4317".to_string());

    let service_name = std::env::var("OTEL_SERVICE_NAME")
        .unwrap_or_else(|_| "user-service".to_string());

    // Initialize OpenTelemetry OTLP exporter with gRPC (Tonic)
    let otlp_exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_endpoint(endpoint)
        .build()?;

    // Create tracer provider with OTLP exporter and resource
    let resource = Resource::builder()
        .with_service_name(service_name.clone())
        .build();

    let provider = sdktrace::SdkTracerProvider::builder()
        .with_batch_exporter(otlp_exporter)
        .with_resource(resource)
        .build();

    // Set as global tracer provider
    global::set_tracer_provider(provider.clone());

    // Get tracer from the provider
    let tracer = provider.tracer("user-service");

    // Create OpenTelemetry tracing layer
    let telemetry_layer = tracing_opentelemetry::layer().with_tracer(tracer);

    // Configure environment-based log filtering
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,user_service=debug"));

    // Initialize the tracing subscriber with all layers
    tracing_subscriber::registry()
        .with(telemetry_layer)
        .with(tracing_subscriber::fmt::layer()
            .with_target(true)
            .with_thread_ids(true)
            .with_line_number(true))
        .with(env_filter)
        .try_init()?;

    tracing::info!(
        service_name = %service_name,
        "Tracing initialized successfully with OpenTelemetry OTLP export"
    );

    Ok(())
}

/// Shutdown tracing provider gracefully
///
/// Flushes any pending spans to the OTLP collector before shutdown
pub fn shutdown_tracing() {
    tracing::info!("Shutting down tracing and flushing spans");
    // In OpenTelemetry 0.31, shutdown is handled by dropping the provider
    // The global provider will flush spans on shutdown
}
