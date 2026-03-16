use std::path::PathBuf;
use std::sync::Arc;

use axum::http::StatusCode;
use sqlx::sqlite::SqlitePoolOptions;
use tokio::sync::RwLock;
use tracing_subscriber::EnvFilter;

use zomboid_seasons::config::{AppConfig, LoggingConfig};
use zomboid_seasons::crypto::Cipher;
use zomboid_seasons::db::{SeasonRepo, SqliteRepo};
use zomboid_seasons::error;
use zomboid_seasons::services::gametime::GameTimeCache;
use zomboid_seasons::services::rcon::RconConfig;
use zomboid_seasons::{AppState, LeaderboardCache};

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
        .unwrap_or_else(|| "config.toml".to_string());

    let config = AppConfig::from_file(config_path.as_ref())?;

    init_tracing(&config.logging);

    tracing::info!(config_path = %config_path, "loaded configuration");

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&config.database.url)
        .await
        .map_err(|e| error::AppError::Config(format!("database connection failed: {e}")))?;

    sqlx::query("PRAGMA journal_mode = WAL")
        .execute(&pool)
        .await?;

    let cipher = Cipher::from_base64_key(&config.database.encryption_key)?;
    let repo = SqliteRepo::new(pool, cipher);
    repo.run_migrations().await?;
    tracing::info!("database migrations complete");

    // Encrypt any existing plaintext passwords
    let migrated = repo.encrypt_plaintext_passwords().await?;
    if migrated > 0 {
        tracing::info!(count = migrated, "encrypted plaintext passwords");
    }

    let rcon_config = RconConfig::from_zomboid_config(&config.zomboid)?;

    let season = repo.get_active_season().await?;
    tracing::info!(
        season_id = season.id.as_i64(),
        ends_at = %season.ends_at,
        "active season verified"
    );

    let leaderboard_cache: LeaderboardCache = Arc::new(RwLock::new(None));
    let gametime_cache: GameTimeCache = Arc::new(RwLock::new(None));

    let state = AppState {
        repo,
        archive_dir: PathBuf::from(&config.zomboid.archive_dir),
        deposit_address: config.eth.deposit_address.clone(),
        base_fee_wei: config.eth.base_fee_wei.clone(),
        admin_token: Arc::from(config.admin.token.as_str()),
        static_dir: PathBuf::from(&config.server.static_dir),
        config: Arc::new(config.clone()),
        rcon_config: Arc::new(rcon_config),
        leaderboard_cache: leaderboard_cache.clone(),
        gametime_cache: gametime_cache.clone(),
    };

    zomboid_seasons::services::scheduler::spawn_scheduler(
        state.repo.clone(),
        state.config.clone(),
        state.rcon_config.clone(),
    );

    zomboid_seasons::services::payment::spawn_payment_poller(
        state.repo.clone(),
        state.rcon_config.clone(),
        state.config.clone(),
        &config.eth.rpc_url,
        &state.deposit_address,
    );

    zomboid_seasons::services::payment::spawn_expiry_cleanup(
        state.repo.clone(),
        config.eth.payment_expiry_hours,
    );

    zomboid_seasons::services::leaderboard::spawn_leaderboard_poller(
        state.repo.clone(),
        config.leaderboard.clone(),
        leaderboard_cache,
    );

    zomboid_seasons::services::gametime::spawn_gametime_poller(
        config.leaderboard.data_dir.clone(),
        gametime_cache,
    );

    zomboid_seasons::services::events::spawn_event_poller(
        state.repo.clone(),
        config.events.clone(),
    );

    if config.supply_drops.enabled {
        zomboid_seasons::services::supply_drops::spawn_drop_scheduler(
            state.repo.clone(),
            state.config.clone(),
            state.rcon_config.clone(),
        );
        zomboid_seasons::services::supply_drops::spawn_claim_poller(
            state.repo.clone(),
            state.config.clone(),
            state.rcon_config.clone(),
        );
    }

    // Write mod config for Lua mod (includes steam_id_map for precision fix)
    if let Err(e) =
        zomboid_seasons::services::leaderboard::write_mod_config(&config.leaderboard.data_dir, &state.repo).await
    {
        tracing::warn!(error = %e, "mod_config_write_failed");
    }

    // Update server name in .ini for current season
    if let Err(e) = zomboid_seasons::services::rotation::update_server_ini(&config, season.id).await
    {
        tracing::warn!(error = %e, "server_ini_update_failed_on_startup");
    }

    // Sync whitelist in background (retries if PZ server isn't ready yet)
    {
        let repo = state.repo.clone();
        let rcon_config = state.rcon_config.clone();
        let zomboid_config = config.zomboid.clone();
        tokio::spawn(async move {
            if let Err(e) = zomboid_seasons::services::whitelist::sync_whitelist_on_startup(
                &repo,
                &rcon_config,
                &zomboid_config,
            )
            .await
            {
                tracing::warn!(error = %e, "whitelist_sync_failed_on_startup");
            }
        });
    }

    let rate_limited_api = zomboid_seasons::api::build_api_router()
        .with_state(state.clone())
        .layer(tower_governor::GovernorLayer::new(
            tower_governor::governor::GovernorConfigBuilder::default()
                .key_extractor(zomboid_seasons::rate_limit::RealIpKeyExtractor)
                .per_second(50)
                .burst_size(100)
                .finish()
                .expect("valid governor config"),
        ));

    let app = axum::Router::new()
        .route("/health", axum::routing::get(health))
        .merge(rate_limited_api)
        .fallback_service(
            tower_http::services::ServeDir::new(&state.static_dir).not_found_service(
                tower_http::services::ServeFile::new(state.static_dir.join("index.html")),
            ),
        );

    let listener = tokio::net::TcpListener::bind(&config.server.bind).await?;
    tracing::info!(bind = %config.server.bind, "server listening");

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await?;

    Ok(())
}
