use axum::{
    extract::State,
    middleware,
    routing::{delete, get, post},
    Json, Router,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod auth;
mod config;
mod error;
mod handlers;
mod models;
mod qdrant;

use auth::{auth_middleware, AuthState};
use config::Config;
use error::AppError;
use qdrant::QdrantService;

#[derive(Clone)]
pub struct AppState {
    qdrant: Arc<QdrantService>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "synaplan_qdrant_service=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load configuration
    dotenvy::dotenv().ok();
    let config = Config::from_env()?;
    info!("Configuration loaded: {:?}", config);

    // Initialize Qdrant service
    let qdrant = QdrantService::new(&config).await?;
    info!("Connected to Qdrant at {}", config.qdrant_url);

    // Ensure collection exists
    qdrant.ensure_collection_exists().await?;
    info!("Collection '{}' is ready", config.collection_name);

    // Create app state
    let state = AppState {
        qdrant: Arc::new(qdrant),
    };

    // Create auth state
    let auth_state = Arc::new(AuthState {
        api_key: config.service_api_key.clone(),
    });

    // Build protected routes (require API key if configured)
    let protected_routes = Router::new()
        .route("/memories", post(handlers::upsert_memory))
        .route("/memories/:point_id", get(handlers::get_memory))
        .route("/memories/:point_id", delete(handlers::delete_memory))
        .route("/memories/search", post(handlers::search_memories))
        .route("/memories/scroll", post(handlers::scroll_memories))
        .route("/collection/info", get(handlers::get_collection_info))
        .route_layer(middleware::from_fn_with_state(
            auth_state.clone(),
            auth_middleware,
        ))
        .with_state(state.clone());

    // Build public routes (no auth required)
    let public_routes = Router::new()
        .route("/health", get(health_check))
        .with_state(state);

    // Combine routes
    let app = Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .layer(
            tower_http::trace::TraceLayer::new_for_http()
                .make_span_with(tower_http::trace::DefaultMakeSpan::new())
                .on_response(tower_http::trace::DefaultOnResponse::new()),
        )
        .layer(
            tower_http::cors::CorsLayer::new()
                .allow_origin(tower_http::cors::Any)
                .allow_methods(tower_http::cors::Any)
                .allow_headers(tower_http::cors::Any),
        );

    // Start server
    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    info!("Starting server on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn health_check(State(state): State<AppState>) -> Result<Json<serde_json::Value>, AppError> {
    let healthy = state.qdrant.health_check().await?;

    Ok(Json(serde_json::json!({
        "status": if healthy { "healthy" } else { "unhealthy" },
        "service": "synaplan-qdrant-service",
        "qdrant": if healthy { "connected" } else { "disconnected" }
    })))
}

