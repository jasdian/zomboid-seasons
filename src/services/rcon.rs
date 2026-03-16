use crate::config::ZomboidConfig;
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone)]
pub struct RconConfig {
    pub url: String,
    pub password: String,
}

impl RconConfig {
    pub fn from_zomboid_config(cfg: &ZomboidConfig) -> AppResult<Self> {
        let password = std::fs::read_to_string(&cfg.rcon_pw_file)
            .map_err(|e| {
                AppError::Config(format!(
                    "failed to read rcon password file {}: {e}",
                    cfg.rcon_pw_file
                ))
            })?
            .trim()
            .to_string();

        let url = format!("{}:{}", cfg.rcon_host, cfg.rcon_port);
        tracing::info!(url = %url, "rcon config loaded");
        Ok(Self { url, password })
    }
}

pub async fn rcon_command(config: &RconConfig, command: &str) -> AppResult<String> {
    let url = config.url.clone();
    let password = config.password.clone();
    let command = command.to_string();

    let result = tokio::task::spawn_blocking(move || {
        use rcon_client::{AuthRequest, RCONClient, RCONConfig, RCONRequest};

        let config = RCONConfig {
            url: url.clone(),
            read_timeout: Some(10),
            write_timeout: Some(10),
        };

        let mut client = RCONClient::new(config).map_err(|e| {
            tracing::debug!(error = %e, "rcon_connection_failed");
            AppError::Rcon(e.to_string())
        })?;

        let auth = AuthRequest::new(password);
        client.auth(auth).map_err(|e| {
            tracing::debug!(error = %e, "rcon_auth_failed");
            AppError::Rcon(e.to_string())
        })?;

        tracing::debug!("rcon_connected");

        let request = RCONRequest::new(command.clone());
        let response = client.execute(request).map_err(|e| {
            tracing::error!(error = %e, command = %command, "rcon_execute_failed");
            AppError::Rcon(e.to_string())
        })?;

        tracing::debug!(command = %command, "rcon_sent");
        Ok::<String, AppError>(response.body)
    })
    .await
    .map_err(|e| AppError::Rcon(format!("spawn_blocking join error: {e}")))?;

    result
}

/// Reload all ZomboidSeasons Lua modules via RCON.
/// PZ's `reloadlua` takes a single filename, so we reload Main.lua
/// which `require`s all submodules (Config, KillTracker, SupplyDrops, StarterKit).
/// Kahlua2's `require` re-executes on reloadlua, so all modules reload transitively.
pub async fn reload_lua(config: &RconConfig) -> AppResult<String> {
    rcon_command(config, r#"reloadlua "ZomboidSeasons/Main.lua""#).await
}

pub async fn rcon_command_best_effort(config: &RconConfig, command: &str) {
    match rcon_command(config, command).await {
        Ok(_) => {}
        Err(e) => {
            tracing::warn!(error = %e, command = %command, "rcon_command_failed");
        }
    }
}
