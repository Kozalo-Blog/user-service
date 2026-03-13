use opentelemetry::trace::TracerProvider;
use opentelemetry_sdk::trace::{InMemorySpanExporter, SdkTracerProvider, SimpleSpanProcessor};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::Registry;

const TEST_TRACER_NAME: &str = "test-tracer";

pub fn setup_otel_test() -> (InMemorySpanExporter, SdkTracerProvider, impl tracing::Subscriber) {
    let exporter = InMemorySpanExporter::default();
    let provider = SdkTracerProvider::builder()
        .with_span_processor(SimpleSpanProcessor::new(exporter.clone()))
        .build();
    let tracer = provider.tracer(TEST_TRACER_NAME);
    let telemetry_layer = tracing_opentelemetry::layer().with_tracer(tracer);
    let subscriber = Registry::default().with(telemetry_layer);
    (exporter, provider, subscriber)
}
