use opentelemetry::global;
use opentelemetry::trace::TracerProvider;
use opentelemetry_sdk::Resource;
use opentelemetry_otlp::{SpanExporter, WithExportConfig};
use opentelemetry_sdk::trace::SdkTracerProvider;
use tracing_subscriber::{Layer, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use crate::env::get_value_or_default;

/// Service name from Cargo.toml
const SERVICE_NAME: &str = env!("CARGO_PKG_NAME");

/// Initialize tracing subscriber with OpenTelemetry OTLP export
///
/// Configures tracing with:
/// - Console output with structured formatting
/// - OpenTelemetry OTLP exporter to VictoriaStack/Grafana
/// - Environment-based log level filtering
///
/// Configuration via environment variables:
/// - OTEL_EXPORTER_OTLP_ENDPOINT: OTLP endpoint (default: http://localhost:4317)
pub fn init_tracing() -> Result<SdkTracerProvider, Box<dyn std::error::Error>> {
    let endpoint = get_value_or_default(
        "OTEL_EXPORTER_OTLP_ENDPOINT",
        "http://localhost:4317".to_string(),
    );

    // Initialize OpenTelemetry OTLP exporter with gRPC (Tonic)
    let otlp_exporter = SpanExporter::builder()
        .with_tonic()
        .with_endpoint(endpoint)
        .build()?;

    // Create tracer provider with OTLP exporter and resource
    let resource = Resource::builder()
        .with_service_name(SERVICE_NAME.to_owned())
        .build();
    let provider = SdkTracerProvider::builder()
        .with_batch_exporter(otlp_exporter)
        .with_resource(resource)
        .build();

    // Set as global tracer provider
    global::set_tracer_provider(provider.clone());

    // Initialize the tracing subscriber with all layers
    let telemetry_layer = tracing_opentelemetry::layer()
        .with_tracer(provider.tracer(SERVICE_NAME));
    // Per-layer filtering: EnvFilter only applies to console output,
    // so OpenTelemetry spans (including axum-tracing-opentelemetry) are never disabled
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_thread_ids(true)
        .with_line_number(true)
        .with_filter(EnvFilter::from_default_env());
    tracing_subscriber::registry()
        .with(telemetry_layer)
        .with(fmt_layer)
        .try_init()?;

    tracing::info!(
        service_name = %SERVICE_NAME,
        "Tracing initialized successfully with OpenTelemetry OTLP export"
    );
    Ok(provider)
}
