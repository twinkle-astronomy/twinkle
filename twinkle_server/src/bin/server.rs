use axum::Router;

use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;

use twinkle_server::AppState;

#[tokio::main]
async fn main() {
    // initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_span_events(
            tracing_subscriber::fmt::format::FmtSpan::NEW
                | tracing_subscriber::fmt::format::FmtSpan::CLOSE,
        )
        .init();
    let state = AppState::default();

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([axum::http::Method::GET, axum::http::Method::POST])
        .allow_headers(Any)
        .expose_headers(Any);

    // build our application with a route
    let app = twinkle_server::indi::routes(Router::new())
        .with_state(state)
        .layer(cors)
        .fallback_service(ServeDir::new("assets"));

    // run our app with hyper
    let listener = tokio::net::TcpListener::bind("0.0.0.0:4000").await.unwrap();
    tracing::debug!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}
