use axum::{
    extract::State,
    routing::{delete, get, post},
    Json, Router,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

mod auth;
mod alerts;
mod config;
mod error;
mod handlers;
mod metrics;
mod models;
mod qdrant;
mod request_id;
mod stats;

use auth::{auth_middleware, AuthState};
use alerts::WebhookAlerts;
use config::Config;
use error::AppError;
use metrics::MetricsState;
use qdrant::QdrantService;
use stats::StatsTracker;

/// OpenAPI documentation
#[derive(OpenApi)]
#[openapi(
    info(
        title = "Synaplan Qdrant Microservice",
        version = env!("CARGO_PKG_VERSION"),
        description = "High-performance vector storage and search service for Synaplan memories.\n\n\
        **Key Features:**\n\
        - ✅ Vector storage with Qdrant (1024-dim BGE-M3 embeddings)\n\
        - ✅ Semantic search with cosine similarity\n\
        - ✅ Batch operations (up to 100 points)\n\
        - ✅ User-scoped and category filtering\n\
        - ✅ Prometheus metrics + Generic webhook alerts\n\
        - ⚠️ Embedding removed - backend must send pre-computed vectors\n\n\
        **Authentication:** Protected endpoints require `X-API-Key` header.\n\n\
        **Performance:** 2-5ms search for 10k points, ~50ms for 100k points."
    ),
    paths(
        handlers::get_capabilities,
        handlers::upsert_memory,
        handlers::get_memory,
        handlers::delete_memory,
        handlers::search_memories,
        handlers::get_collection_info,
        handlers::scroll_memories,
        handlers::batch_upsert_memories,
        handlers::get_service_info,
        // Document endpoints
        handlers::upsert_document,
        handlers::batch_upsert_documents,
        handlers::search_documents,
        handlers::get_document,
        handlers::delete_document,
        handlers::delete_by_file,
        handlers::delete_by_group_key,
        handlers::delete_all_for_user,
        handlers::update_group_key,
        handlers::get_document_stats,
        handlers::get_group_keys,
    ),
    components(schemas(
        models::ServiceCapabilities,
        models::EmbeddingCapabilities,
        models::MemoryPayload,
        models::UpsertMemoryRequest,
        models::BatchUpsertRequest,
        models::MemoryResponse,
        models::SearchMemoriesRequest,
        models::SearchResult,
        models::SearchMemoriesResponse,
        models::ScrollMemoriesRequest,
        models::ScrollMemoriesResponse,
        models::CollectionInfo,
        models::BatchOperationResponse,
        models::BatchError,
        // Document schemas
        models::DocumentPayload,
        models::UpsertDocumentRequest,
        models::BatchUpsertDocumentsRequest,
        models::BatchUpsertResponse,
        models::SearchDocumentsRequest,
        models::DocumentSearchResult,
        models::DeleteByFileRequest,
        models::DeleteByGroupKeyRequest,
        models::UpdateGroupKeyRequest,
        models::DocumentStatsResponse,
    )),
    tags(
        (name = "Service Info", description = "Service capabilities, version, and statistics"),
        (name = "Memories", description = "CRUD operations for memory storage and search"),
        (name = "documents", description = "CRUD operations for document chunk storage and search")
    )
)]
struct ApiDoc;

#[derive(Clone)]
pub struct AppState {
    qdrant: Arc<QdrantService>,
    config: Arc<Config>,
    metrics: MetricsState,
    alerts: WebhookAlerts,
    stats: StatsTracker,
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
    if config.tls_cert_path.is_some() || config.tls_key_path.is_some() {
        info!(
            "TLS paths configured (cert: {:?}, key: {:?})",
            config.tls_cert_path,
            config.tls_key_path
        );
    }

    // Initialize Prometheus metrics exporter
    let builder = metrics_exporter_prometheus::PrometheusBuilder::new();
    let prometheus_handle = builder
        .install_recorder()
        .expect("Failed to install Prometheus recorder");
    info!("Prometheus metrics exporter initialized");

    // Initialize metrics state
    let metrics = MetricsState::new();

    // Initialize stats tracker
    let stats = StatsTracker::new();

    // Initialize webhook alerts (filter out empty strings)
    let alerts = WebhookAlerts::new(
        config.webhook_url.clone().filter(|url| !url.is_empty())
    );

    // Initialize Qdrant service
    let qdrant = match QdrantService::new(&config).await {
        Ok(service) => service,
        Err(e) => {
            let error_msg = format!("Failed to connect to Qdrant: {}", e);
            error!("{}", error_msg);
            alerts.alert_qdrant_connection_failed(&error_msg).await;
            return Err(e);
        }
    };
    info!("Connected to Qdrant at {}", config.qdrant_url);

    // Ensure collection exists
    qdrant.ensure_collection_exists().await?;
    info!("Collections ready");

    // Send startup notification
    alerts
        .alert_service_started(env!("CARGO_PKG_VERSION"))
        .await;

    // Create app state
    let state = AppState {
        qdrant: Arc::new(qdrant),
        config: Arc::new(config.clone()),
        metrics: metrics.clone(),
        alerts: alerts.clone(),
        stats: stats.clone(),
    };

    // Create auth state
    let auth_state = Arc::new(AuthState::new(config.service_api_key.clone()));
    if auth_state.is_enabled() {
        info!("API key authentication enabled");
    } else {
        info!("API key authentication disabled");
    }

    // Build protected routes (require API key if configured)
    let protected_routes = Router::new()
        .route("/memories", post(handlers::upsert_memory))
        .route("/memories/batch", post(handlers::batch_upsert_memories))
        .route("/memories/:point_id", get(handlers::get_memory))
        .route("/memories/:point_id", delete(handlers::delete_memory))
        .route("/memories/search", post(handlers::search_memories))
        .route("/memories/scroll", post(handlers::scroll_memories))
        .route("/collection/info", get(handlers::get_collection_info))
        .route("/service/info", get(handlers::get_service_info))
        // Document routes
        .route("/documents", post(handlers::upsert_document))
        .route("/documents/batch", post(handlers::batch_upsert_documents))
        .route("/documents/search", post(handlers::search_documents))
        .route("/documents/:point_id", get(handlers::get_document))
        .route("/documents/:point_id", delete(handlers::delete_document))
        .route("/documents/delete-by-file", post(handlers::delete_by_file))
        .route("/documents/delete-by-group", post(handlers::delete_by_group_key))
        .route("/documents/user/:user_id", delete(handlers::delete_all_for_user))
        .route("/documents/update-group-key", post(handlers::update_group_key))
        .route("/documents/stats/:user_id", get(handlers::get_document_stats))
        .route("/documents/groups/:user_id", get(handlers::get_group_keys))
        .route("/documents/files-by-group", post(handlers::get_files_by_group))
        .route_layer(axum::middleware::from_fn_with_state(
            auth_state.clone(),
            auth_middleware,
        ))
        .with_state(state.clone());

    // Build public routes (no auth required)
    let public_routes = Router::new()
        .route("/health", get(health_check))
        .route("/capabilities", get(handlers::get_capabilities))
        .route("/metrics", get(move || async move {
            prometheus_handle.render()
        }))
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .with_state(state.clone());

    // Combine routes with Request ID tracking + Metrics + Tracing + CORS
    let app = Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .layer(axum::middleware::from_fn(request_id::request_id_middleware))
        .layer(axum::middleware::from_fn_with_state(
            metrics.clone(),
            metrics::track_metrics,
        ))
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

    // Start server with graceful shutdown
    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    info!("Starting server on {}", addr);
    info!("📖 Swagger UI available at http://{}/swagger-ui", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;

    // Spawn daily stats task if enabled
    if config.enable_daily_stats {
        let stats_task_state = state.clone();
        tokio::spawn(async move {
            daily_stats_task(stats_task_state).await;
        });
        info!(
            "📊 Daily stats reporting enabled (every {} hours)",
            config.stats_interval_hours
        );
    }

    // Check if TLS is enabled
    #[cfg(feature = "tls")]
    if config.tls_enabled {
        info!("TLS enabled - starting HTTPS server");
        
        let tls_config = load_tls_config(&config)?;
        
        axum_server::bind_rustls(addr, tls_config)
            .serve(app.into_make_service())
            .with_graceful_shutdown(shutdown_signal())
            .await?;
    } else {
        info!("TLS disabled - starting HTTP server");
        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown_signal())
            .await?;
    }

    #[cfg(not(feature = "tls"))]
    {
        if config.tls_enabled {
            tracing::warn!(
                "TLS requested but not compiled with TLS support. Starting HTTP server instead. (TLS_CERT_PATH={:?}, TLS_KEY_PATH={:?})",
                config.tls_cert_path,
                config.tls_key_path
            );
        }
        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown_signal())
            .await?;
    }

    Ok(())
}

/// Daily statistics reporting task
///
/// Runs in the background and sends stats to webhook at configured intervals.
/// Automatically resets stats after each report.
async fn daily_stats_task(state: AppState) {
    let interval_hours = state.config.stats_interval_hours;
    let interval = tokio::time::Duration::from_secs(interval_hours * 3600);

    loop {
        tokio::time::sleep(interval).await;

        // Get stats snapshot
        let snapshot = state.stats.get_snapshot();

        // Send to webhook
        state
            .alerts
            .send_daily_stats(&snapshot, &state.config.collection_name)
            .await;

        // Reset stats for next period
        state.stats.reset();

        info!(
            "Daily stats sent: {} upserts, {} searches, {} deletes",
            snapshot.upserts, snapshot.searches, snapshot.deletes
        );
    }
}

/// Graceful shutdown signal handler
///
/// **Purpose:** Waits for SIGTERM (Docker stop) or SIGINT (Ctrl+C) and closes connections cleanly.
/// Prevents: "Connection reset by peer" errors during deployment restarts.
async fn shutdown_signal() {
    use tokio::signal;

    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("Received Ctrl+C, shutting down gracefully...");
        },
        _ = terminate => {
            info!("Received SIGTERM, shutting down gracefully...");
        },
    }
}

#[cfg(feature = "tls")]
fn load_tls_config(config: &Config) -> anyhow::Result<axum_server::tls_rustls::RustlsConfig> {
    use std::fs::File;
    use std::io::BufReader;
    use rustls::{ServerConfig, pki_types::{CertificateDer, PrivateKeyDer}};
    use rustls_pemfile::{certs, pkcs8_private_keys};

    let cert_path = config.tls_cert_path.as_ref()
        .ok_or_else(|| anyhow::anyhow!("TLS_CERT_PATH not set"))?;
    let key_path = config.tls_key_path.as_ref()
        .ok_or_else(|| anyhow::anyhow!("TLS_KEY_PATH not set"))?;

    // Load certificates
    let cert_file = File::open(cert_path)?;
    let mut cert_reader = BufReader::new(cert_file);
    let certs: Vec<CertificateDer> = certs(&mut cert_reader)
        .collect::<Result<_, _>>()?;

    // Load private key
    let key_file = File::open(key_path)?;
    let mut key_reader = BufReader::new(key_file);
    let mut keys = pkcs8_private_keys(&mut key_reader)
        .collect::<Result<Vec<_>, _>>()?;

    if keys.is_empty() {
        return Err(anyhow::anyhow!("No private key found"));
    }

    let key = PrivateKeyDer::Pkcs8(keys.remove(0));

    let server_config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)?;

    Ok(axum_server::tls_rustls::RustlsConfig::from_config(Arc::new(server_config)))
}

async fn health_check(State(state): State<AppState>) -> Result<Json<serde_json::Value>, AppError> {
    let healthy = state.qdrant.health_check().await?;
    
    // Get Qdrant stats for metrics
    let (coll_status, points_count, vectors_count, _) = state.qdrant.get_collection_info(None).await.unwrap_or((
        "unknown".to_string(),
        0,
        0,
        0,
    ));
    
    // Update Prometheus metrics
    state.metrics.update_qdrant_stats(points_count, vectors_count);

    // Calculate metrics
    let requests_total = state.metrics.get_requests_total();
    let requests_failed = state.metrics.get_requests_failed();
    let success_rate = if requests_total > 0 {
        ((requests_total - requests_failed) as f64 / requests_total as f64) * 100.0
    } else {
        100.0
    };

    // Check for high error rate and alert
    if requests_total > 100 && success_rate < 95.0 {
        let error_rate = 100.0 - success_rate;
        tokio::spawn({
            let alerts = state.alerts.clone();
            async move {
                alerts
                    .alert_high_error_rate(error_rate, requests_failed, requests_total)
                    .await;
            }
        });
    }

    Ok(Json(serde_json::json!({
        "status": if healthy { "healthy" } else { "unhealthy" },
        "service": "synaplan-qdrant-service",
        "version": env!("CARGO_PKG_VERSION"),
        "uptime_seconds": state.metrics.uptime_seconds(),
        "qdrant": {
            "status": if healthy { "connected" } else { "disconnected" },
            "collection_status": coll_status,
            "points_count": points_count,
            "vectors_count": vectors_count,
        },
        "metrics": {
            "requests_total": requests_total,
            "requests_failed": requests_failed,
            "requests_success": requests_total - requests_failed,
            "success_rate_percent": format!("{:.2}", success_rate),
        }
    })))
}

