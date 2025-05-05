use axum::Router;

use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;

use tracing::level_filters::LevelFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use tracing_subscriber::Layer;
use twinkle_server::{flats, tracing_broadcast, AppState};

#[tokio::main]
async fn main() {
    // initialize tracing
    let fmt = tracing_subscriber::fmt::layer()
    // .with_filter(LevelFilter::DEBUG)
        .with_span_events(
            tracing_subscriber::fmt::format::FmtSpan::NEW
                | tracing_subscriber::fmt::format::FmtSpan::CLOSE,
        ).with_filter(LevelFilter::DEBUG);

    tracing_subscriber::Registry::default()
        .with(fmt)
        .with(tracing_broadcast::TracingBroadcast::new("twinkle_server::flats", flats::TRACE_CHANNEL.clone()))
        .init();
    let state = AppState::new().await.expect("Loading AppState");

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([axum::http::Method::GET, axum::http::Method::POST, axum::http::Method::DELETE])
        .allow_headers(Any)
        .expose_headers(Any);

        
    // build our application with a route
    let app = Router::new()
        .merge(twinkle_server::flats::routes())
        .merge(twinkle_server::counts::routes())
        .merge(twinkle_server::indi::routes())
        .merge(twinkle_server::settings::routes())
        .with_state(state)
        .layer(cors)
        .fallback_service(ServeDir::new("assets"));

    // run our app with hyper
    let listener = tokio::net::TcpListener::bind("0.0.0.0:4000").await.unwrap();
    tracing::debug!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}
