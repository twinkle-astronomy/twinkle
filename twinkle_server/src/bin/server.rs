use axum::Router;

use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;

use tracing::level_filters::LevelFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use tracing_subscriber::Layer;
use twinkle_server::{flats, tracing_broadcast, AppState};

use opentelemetry::trace::TracerProvider;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::trace as sdktrace;
use opentelemetry_sdk::Resource;
use opentelemetry_semantic_conventions::resource::SERVICE_NAME;


#[tokio::main]
async fn main() {
    // Initialize OpenTelemetry OTLP exporter
    let otlp_endpoint = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
        .unwrap_or_else(|_| "http://tempo:4317".to_string());

    let tracer_provider = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(
            opentelemetry_otlp::new_exporter()
                .tonic()
                .with_endpoint(otlp_endpoint)
        )
        .with_trace_config(
            sdktrace::Config::default()
                .with_resource(Resource::new(vec![
                    opentelemetry::KeyValue::new(SERVICE_NAME, "twinkle_server"),
                ]))
        )
        .install_batch(opentelemetry_sdk::runtime::Tokio)
        .expect("Failed to initialize tracer");

    let tracer = tracer_provider.tracer("twinkle_server");
    let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);

    // initialize tracing
    let fmt = tracing_subscriber::fmt::layer()
        // .with_filter(LevelFilter::DEBUG)
        .with_span_events(
            tracing_subscriber::fmt::format::FmtSpan::NEW
                | tracing_subscriber::fmt::format::FmtSpan::CLOSE,
        )
        .with_filter(LevelFilter::DEBUG);

    tracing_subscriber::Registry::default()
        .with(telemetry)
        .with(fmt)
        .with(tracing_broadcast::TracingBroadcast::new(
            "twinkle_server::flats",
            flats::TRACE_CHANNEL.clone(),
        ))
        .init();
    let state = AppState::new().await.expect("Loading AppState");

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::DELETE,
        ])
        .allow_headers(Any)
        .expose_headers(Any);

    // build our application with a route
    let app = Router::new()
        .merge(twinkle_server::flats::routes())
        .merge(twinkle_server::indi::routes())
        .merge(twinkle_server::settings::routes())
        .merge(twinkle_server::capture::routes())
        .with_state(state)
        .layer(cors)
        .fallback_service(ServeDir::new("assets"));

    // run our app with hyper
    let listener = tokio::net::TcpListener::bind("0.0.0.0:4000").await.unwrap();
    tracing::debug!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();

    // Shutdown the global tracer provider to ensure all spans are exported
    opentelemetry::global::shutdown_tracer_provider();
}
