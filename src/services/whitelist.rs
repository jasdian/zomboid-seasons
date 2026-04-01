use std::sync::Arc;

use crate::config::ZomboidConfig;
use crate::db::{RotationRepo, SeasonRepo, SqliteRepo};
use crate::domain::SeasonId;
use crate::error::AppResult;
use crate::services::rcon::{rcon_command_best_effort, RconConfig};

/// Whitelist a player on the PZ server via RCON.
///
/// `adduser` creates the in-game account (always required).
/// `addsteamid` is an optional extra layer that gates connection by Steam ID.
/// Since Build 42.14, `adduser` alone is sufficient for Open=false servers.
pub async fn whitelist_player(
    rcon_config: &RconConfig,
    _zomboid_config: &ZomboidConfig,
    username: &str,
    password: &str,
    steam_id: Option<&str>,
) {
    if let Some(sid) = steam_id {
        let cmd = format!("addsteamid {}", sid);
        rcon_command_best_effort(rcon_config, &cmd).await;
    }

    let cmd = format!("adduser {} {}", username, password);
    rcon_command_best_effort(rcon_config, &cmd).await;
}

/// Remove a player from the PZ whitelist via RCON.
pub async fn remove_player(
    rcon_config: &RconConfig,
    _zomboid_config: &ZomboidConfig,
    username: &str,
    steam_id: Option<&str>,
) {
    let cmd = format!("removeuserfromwhitelist {}", username);
    rcon_command_best_effort(rcon_config, &cmd).await;

    if let Some(sid) = steam_id {
        let cmd = format!("removesteamid {}", sid);
        rcon_command_best_effort(rcon_config, &cmd).await;
    }
}

/// Whitelist a single player after registration confirmation.
pub async fn apply_whitelist_for_registration(
    repo: &SqliteRepo,
    rcon_config: &RconConfig,
    zomboid_config: &ZomboidConfig,
    username: &str,
    password: &str,
    steam_id: Option<&str>,
    season_id: SeasonId,
) -> AppResult<()> {
    let season = repo.get_season_by_id(season_id).await?;
    let is_active = season
        .as_ref()
        .map(|s| s.status == crate::domain::SeasonStatus::Active)
        .unwrap_or(false);

    if !is_active {
        tracing::debug!(
            username,
            season_id = season_id.as_i64(),
            "skipping whitelist apply for non-active season"
        );
        return Ok(());
    }

    whitelist_player(rcon_config, zomboid_config, username, password, steam_id).await;

    tracing::info!(username, reason = "registration", "whitelist_player_added");
    Ok(())
}

/// Sync all confirmed players for the active season to the PZ server.
/// Retries with backoff if RCON is unavailable (e.g. PZ server hasn't started yet).
/// Uses `rcon_command` (not best_effort) so failures are detected and trigger retries.
pub async fn sync_whitelist_on_startup(
    repo: &SqliteRepo,
    rcon_config: &Arc<RconConfig>,
    _zomboid_config: &ZomboidConfig,
) -> AppResult<()> {
    // PZ RCON takes 2-5 minutes to stabilize after server start.
    // Wait 90s before first attempt, then retry with increasing delays.
    tracing::info!("whitelist_sync_waiting_for_rcon (90s initial delay)");
    tokio::time::sleep(std::time::Duration::from_secs(90)).await;
    let delays = [30, 60, 120, 240];

    let season = repo.get_active_season().await?;
    let players = repo.get_confirmed_players(season.id).await?;
    let count = players.len();

    if count == 0 {
        tracing::info!(season_id = season.id.as_i64(), "whitelist_sync_no_players");
        return Ok(());
    }

    'retry: for attempt in 0..=delays.len() {
        let mut had_failure = false;

        for (username, password, steam_id) in &players {
            if let Some(sid) = steam_id {
                let cmd = format!("addsteamid {sid}");
                if let Err(e) = super::rcon::rcon_command(rcon_config, &cmd).await {
                    if attempt < delays.len() {
                        let delay = delays[attempt];
                        tracing::info!(
                            attempt = attempt + 1,
                            delay_secs = delay,
                            error = %e,
                            "whitelist_sync_rcon_waiting"
                        );
                        tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
                        continue 'retry;
                    }
                    had_failure = true;
                    tracing::warn!(error = %e, command = %cmd, "whitelist_sync_command_failed");
                    continue;
                }
            }

            let cmd = format!("adduser {username} {password}");
            if let Err(e) = super::rcon::rcon_command(rcon_config, &cmd).await {
                if attempt < delays.len() && !had_failure {
                    let delay = delays[attempt];
                    tracing::info!(
                        attempt = attempt + 1,
                        delay_secs = delay,
                        error = %e,
                        "whitelist_sync_rcon_waiting"
                    );
                    tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
                    continue 'retry;
                }
                had_failure = true;
                tracing::warn!(error = %e, command = %cmd, "whitelist_sync_command_failed");
            }
        }

        if had_failure && attempt == delays.len() {
            tracing::error!("whitelist_sync_completed_with_errors");
        }

        tracing::info!(
            season_id = season.id.as_i64(),
            players_synced = count,
            "whitelist_sync_complete"
        );
        return Ok(());
    }

    unreachable!()
}
