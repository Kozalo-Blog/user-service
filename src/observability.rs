use opentelemetry::global;
use opentelemetry::trace::TracerProvider;
use opentelemetry_sdk::Resource;
use opentelemetry_otlp::{SpanExporter, WithExportConfig};
use opentelemetry_sdk::trace::SdkTracerProvider;
use tracing_subscriber::{Layer, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Service name from Cargo.toml
const SERVICE_NAME: &str = env!("CARGO_PKG_NAME");

/// Initialize tracing subscriber, optionally with OpenTelemetry OTLP export.
///
/// Configuration via environment variables:
/// - `OTEL_EXPORTER_OTLP_ENDPOINT`: OTLP endpoint. If unset, OTLP export is
///   disabled and only console output is produced (useful for local development).
/// - `RUST_LOG`: console log level filter
pub fn init_tracing() -> Result<SdkTracerProvider, Box<dyn std::error::Error>> {
    let provider = build_provider()?;
    global::set_tracer_provider(provider.clone());

    // Per-layer filtering so RUST_LOG controls console verbosity
    // without disabling OTel spans needed for trace context propagation
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_thread_ids(true)
        .with_line_number(true)
        .with_filter(EnvFilter::from_default_env());
    // Allow all user-service spans but suppress noisy h2/hyper connection internals
    let otel_filter = EnvFilter::new("trace,h2=off,hyper=off,tower=off");
    tracing_subscriber::registry()
        .with(tracing_opentelemetry::layer()
            .with_tracer(provider.tracer(SERVICE_NAME))
            .with_filter(otel_filter))
        .with(fmt_layer)
        .try_init()?;

    tracing::info!(service_name = %SERVICE_NAME, "Tracing initialized");
    Ok(provider)
}

fn build_provider() -> Result<SdkTracerProvider, Box<dyn std::error::Error>> {
    let Some(endpoint) = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").ok() else {
        tracing::warn!("OTEL_EXPORTER_OTLP_ENDPOINT is not set — OTLP export disabled");
        return Ok(SdkTracerProvider::builder().build());
    };

    let otlp_exporter = SpanExporter::builder()
        .with_tonic()
        .with_endpoint(endpoint)
        .build()?;
    let resource = Resource::builder()
        .with_service_name(SERVICE_NAME.to_owned())
        .build();
    Ok(SdkTracerProvider::builder()
        .with_batch_exporter(otlp_exporter)
        .with_resource(resource)
        .build())
}
