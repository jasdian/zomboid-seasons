use std::path::PathBuf;
use std::sync::Arc;

use axum::http::StatusCode;
use sqlx::sqlite::SqlitePoolOptions;
use tracing_subscriber::EnvFilter;

use zomboid_status::config::{LoggingConfig, StatusConfig};
use zomboid_status::db::{SqliteRepo, StatusRepo};
use zomboid_status::error;
use zomboid_status::AppState;

fn init_tracing(config: &LoggingConfig) {
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&config.level));

    let subscriber = tracing_subscriber::fmt().with_env_filter(env_filter);

    match config.format.as_str() {
        "json" => subscriber.json().init(),
        _ => subscriber.init(),
    }
}

async fn health() -> StatusCode {
    StatusCode::OK
}

#[tokio::main]
async fn main() -> error::AppResult<()> {
    let config_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "config.toml".into());
    let config = StatusConfig::from_file(&config_path)?;
    init_tracing(&config.logging);
    tracing::info!(config_path = %config_path, "loaded configuration");

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&config.database.url)
        .await
        .map_err(|e| error::AppError::Config(format!("database connection failed: {e}")))?;

    sqlx::query("PRAGMA journal_mode = WAL")
        .execute(&pool)
        .await
        .map_err(error::AppError::Database)?;

    let repo = SqliteRepo::new(pool);
    repo.run_migrations().await?;
    tracing::info!("database migrations complete");

    // Upsert component groups from config
    for group in &config.component_groups {
        repo.upsert_component_group(group).await?;
    }
    tracing::info!(
        count = config.component_groups.len(),
        "component groups synced from config"
    );

    // Upsert components from config
    for comp in &config.components {
        repo.upsert_component(comp).await?;
    }
    tracing::info!(
        count = config.components.len(),
        "components synced from config"
    );

    // Spawn background tasks: probes, uptime aggregator, probe cleanup, maintenance watcher
    let reqwest_client = reqwest::Client::builder()
        .user_agent("zomboid-status-probe/0.1")
        .build()
        .map_err(|e| error::AppError::Config(format!("reqwest client: {e}")))?;

    let notification_sender = zomboid_status::notifications::spawn(
        config_path.clone(),
        repo.clone(),
        reqwest_client.clone(),
        256,
    );

    let state = AppState {
        repo,
        admin_token: Arc::from(config.admin.token.as_str()),
        static_dir: PathBuf::from(&config.server.static_dir),
        config: Arc::new(config.clone()),
        notification_sender: notification_sender.clone(),
    };

    let probe_handles = zomboid_status::probes::spawn_probe_tasks(
        &config.components,
        state.repo.clone(),
        reqwest_client,
        notification_sender.clone(),
    );
    tracing::info!(count = probe_handles.len(), "probe tasks spawned");

    let _uptime_handle = zomboid_status::probes::spawn_uptime_aggregator(state.repo.clone());
    let _cleanup_handle = zomboid_status::probes::spawn_probe_cleanup(state.repo.clone());
    let _maint_handle = zomboid_status::probes::spawn_maintenance_watcher(
        state.repo.clone(),
        notification_sender,
    );

    let bind_addr = state.config.server.bind.clone();

    let governor_config = std::sync::Arc::new(
        tower_governor::governor::GovernorConfigBuilder::default()
            .key_extractor(zomboid_status::rate_limit::RealIpKeyExtractor)
            .per_second(50)
            .burst_size(100)
            .finish()
            .ok_or_else(|| error::AppError::Config("invalid rate limiter config".into()))?,
    );

    let rate_limited_api = zomboid_status::api::build_api_router()
        .with_state(state.clone())
        .layer(tower_governor::GovernorLayer::new(governor_config));

    let internal_api = zomboid_status::api::build_internal_router().with_state(state.clone());

    let app = axum::Router::new()
        .route("/health", axum::routing::get(health))
        .merge(internal_api)
        .merge(rate_limited_api)
        .fallback_service(
            tower_http::services::ServeDir::new(&state.static_dir)
                .append_index_html_on_directories(false)
                .fallback(tower_http::services::ServeFile::new(
                    state.static_dir.join("status.html"),
                )),
        );

    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .map_err(|e| error::AppError::Config(format!("bind failed: {e}")))?;
    tracing::info!(bind = %bind_addr, "server listening");
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await
    .map_err(|e| error::AppError::Config(format!("server error: {e}")))?;
    Ok(())
}
