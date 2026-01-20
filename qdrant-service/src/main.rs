use axum::{
    extract::State,
    routing::{delete, get, post},
    Json, Router,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod auth;
mod config;
mod discord;
mod embedding;
#[cfg(feature = "native_onnx")]
mod embedding_onnx;
mod error;
mod handlers;
mod metrics;
mod metrics_middleware;
mod models;
mod qdrant;

use auth::{auth_middleware, AuthState};
use config::Config;
use discord::DiscordAlerts;
use embedding::{Embedder, OllamaEmbedder};
#[cfg(feature = "native_onnx")]
use embedding_onnx::OnnxRuntimeEmbedder;
use error::AppError;
use metrics::MetricsState;
use qdrant::QdrantService;

#[derive(Clone)]
pub struct AppState {
    qdrant: Arc<QdrantService>,
    config: Arc<Config>,
    embedder: Option<Arc<dyn Embedder>>,
    metrics: MetricsState,
    discord: DiscordAlerts,
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

    // Initialize Prometheus metrics exporter
    let builder = metrics_exporter_prometheus::PrometheusBuilder::new();
    let prometheus_handle = builder
        .install_recorder()
        .expect("Failed to install Prometheus recorder");
    info!("Prometheus metrics exporter initialized");

    // Initialize metrics state
    let metrics = MetricsState::new();

    // Initialize Discord alerts
    let discord = DiscordAlerts::new(config.discord_webhook_url.clone());

    // Initialize Qdrant service
    let qdrant = match QdrantService::new(&config).await {
        Ok(service) => service,
        Err(e) => {
            let error_msg = format!("Failed to connect to Qdrant: {}", e);
            error!("{}", error_msg);
            discord.alert_qdrant_connection_failed(&error_msg).await;
            return Err(e);
        }
    };
    info!("Connected to Qdrant at {}", config.qdrant_url);

    // Ensure collection exists
    qdrant.ensure_collection_exists().await?;
    info!("Collection '{}' is ready", config.collection_name);

    // Initialize embedding backend (optional).
    // NOTE: This is the first step to move vectorization out of PHP. We keep vector-based routes as fallback.
    let embedder: Option<Arc<dyn Embedder>> = match config.embedding_backend.as_str() {
        "none" => None,
        "ollama" => {
            let base_url = config.ollama_base_url.clone().ok_or_else(|| {
                anyhow::anyhow!("EMBEDDING_BACKEND=ollama requires OLLAMA_BASE_URL")
            })?;
            let model = config
                .embedding_model
                .clone()
                .unwrap_or_else(|| "bge-m3".to_string());
            info!("Embedding backend enabled: ollama (model={})", model);
            Some(Arc::new(OllamaEmbedder::new(base_url, model)))
        }
        "onnxruntime" => {
            #[cfg(feature = "native_onnx")]
            {
                let model = config
                    .embedding_model
                    .clone()
                    .unwrap_or_else(|| "bge-m3".to_string());
                let device = config.embedding_device.clone();
                info!("Embedding backend enabled: onnxruntime (model={}, device={})", model, device);

                match OnnxRuntimeEmbedder::try_new(&config, model, device) {
                    Ok(e) => Some(Arc::new(e)),
                    Err(e) => {
                        tracing::error!("Failed to init onnxruntime embedder: {}", e);
                        None
                    }
                }
            }
            #[cfg(not(feature = "native_onnx"))]
            {
                tracing::warn!(
                    "EMBEDDING_BACKEND=onnxruntime requested but binary was built without feature 'native_onnx'. Embeddings disabled (fallback expected)."
                );
                None
            }
        }
        other => {
            tracing::warn!(
                "Unknown or unsupported EMBEDDING_BACKEND='{}'. Embeddings disabled (fallback expected).",
                other
            );
            None
        }
    };

    // Send startup notification
    discord
        .alert_service_started(env!("CARGO_PKG_VERSION"))
        .await;

    // Create app state
    let state = AppState {
        qdrant: Arc::new(qdrant),
        config: Arc::new(config.clone()),
        embedder,
        metrics: metrics.clone(),
        discord: discord.clone(),
    };

    // Create auth state
    let auth_state = Arc::new(AuthState {
        api_key: config.service_api_key.clone(),
    });

    // Build protected routes (require API key if configured)
    let protected_routes = Router::new()
        .route("/memories", post(handlers::upsert_memory))
        .route("/memories/text", post(handlers::upsert_memory_text))
        .route("/memories/:point_id", get(handlers::get_memory))
        .route("/memories/:point_id", delete(handlers::delete_memory))
        .route("/memories/search", post(handlers::search_memories))
        .route("/memories/search-text", post(handlers::search_memories_text))
        .route("/memories/scroll", post(handlers::scroll_memories))
        .route("/collection/info", get(handlers::get_collection_info))
        .route("/info", get(handlers::get_service_info))
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
        .with_state(state);

    // Combine routes
    let app = Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .layer(axum::middleware::from_fn_with_state(
            metrics.clone(),
            metrics_middleware::metrics_middleware,
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

    // Start server
    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    info!("Starting server on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;

    // Check if TLS is enabled
    #[cfg(feature = "tls")]
    if config.tls_enabled {
        info!("TLS enabled - starting HTTPS server");
        
        let tls_config = load_tls_config(&config)?;
        
        axum_server::bind_rustls(addr, tls_config)
            .serve(app.into_make_service())
            .await?;
    } else {
        info!("TLS disabled - starting HTTP server");
        axum::serve(listener, app).await?;
    }

    #[cfg(not(feature = "tls"))]
    {
        if config.tls_enabled {
            tracing::warn!("TLS requested but not compiled with TLS support. Starting HTTP server instead.");
        }
    axum::serve(listener, app).await?;
    }

    Ok(())
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
    let (coll_status, points_count, vectors_count, _) = state.qdrant.get_collection_info().await.unwrap_or((
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
            let discord = state.discord.clone();
            async move {
                discord
                    .alert_high_error_rate(error_rate, requests_failed, requests_total)
                    .await;
            }
        });
    }

    // Check for high collection usage (warning at 100k points)
    if points_count > 100_000 && points_count % 10_000 == 0 {
        tokio::spawn({
            let discord = state.discord.clone();
            async move {
                discord.alert_collection_high_usage(points_count).await;
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

