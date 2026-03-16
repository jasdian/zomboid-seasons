use std::path::Path;
use std::sync::Arc;

use chrono::Utc;
use serde::Deserialize;

use crate::config::LeaderboardConfig;
use crate::db::{PlayerStatsRepo, SeasonRepo, SqliteRepo};
use crate::domain::{SteamId, ZombieKills, ZomboidUsername};
use crate::error::AppResult;
use crate::{CachedLeaderboard, LeaderboardCache, LeaderboardEntry};

/// Shape of the JSON file written by the Lua mod.
#[derive(Debug, Deserialize)]
struct KillStatsFile {
    players: Vec<KillStatsPlayer>,
}

#[derive(Debug, Deserialize)]
struct KillStatsPlayer {
    steam_id: String,
    username: String,
    kills: u64,
}

pub fn spawn_leaderboard_poller(
    repo: SqliteRepo,
    config: LeaderboardConfig,
    cache: LeaderboardCache,
) -> tokio::task::JoinHandle<()> {
    let interval = std::time::Duration::from_secs(config.poll_interval_secs);
    let kills_file = config.kills_file.clone();
    let leaderboard_size = config.leaderboard_size;

    tracing::info!(
        kills_file = %kills_file,
        poll_interval_secs = config.poll_interval_secs,
        "leaderboard_poller_started"
    );

    tokio::spawn(async move {
        loop {
            tokio::time::sleep(interval).await;

            if let Err(e) = poll_once(&repo, &kills_file, leaderboard_size, &cache).await {
                tracing::warn!(error = %e, "leaderboard_poll_error");
            }
        }
    })
}

async fn poll_once(
    repo: &SqliteRepo,
    kills_file: &str,
    leaderboard_size: u32,
    cache: &LeaderboardCache,
) -> AppResult<()> {
    let path = Path::new(kills_file);
    if !path.exists() {
        tracing::debug!(path = %kills_file, "kills_file_not_found");
        return Ok(());
    }

    let content = tokio::fs::read_to_string(path).await?;

    // Empty or placeholder file — skip without error
    let stats: KillStatsFile = match serde_json::from_str(&content) {
        Ok(s) => s,
        Err(e) => {
            tracing::debug!(error = %e, "kills_file_parse_skip");
            return Ok(());
        }
    };

    if stats.players.is_empty() {
        return Ok(());
    }

    let season = repo.get_active_season().await?;
    let season_id = season.id;
    let now = Utc::now();

    for player in &stats.players {
        let steam_id = match SteamId::try_from(player.steam_id.clone()) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(steam_id = %player.steam_id, error = %e, "invalid_steam_id_in_kills_file");
                continue;
            }
        };
        let username = match ZomboidUsername::try_from(player.username.clone()) {
            Ok(u) => u,
            Err(e) => {
                tracing::warn!(username = %player.username, error = %e, "invalid_username_in_kills_file");
                continue;
            }
        };
        let kills = ZombieKills::new(player.kills);

        repo.upsert_player_stats(season_id, &steam_id, &username, kills, now)
            .await?;
    }

    // Refresh cache
    let top = repo.get_leaderboard(season_id, leaderboard_size).await?;
    let entries: Arc<[LeaderboardEntry]> = top
        .iter()
        .enumerate()
        .map(|(i, ps)| LeaderboardEntry {
            rank: (i as u32) + 1,
            zomboid_username: ps.zomboid_username.as_str().to_string(),
            zombie_kills: ps.zombie_kills.as_u64(),
        })
        .collect::<Vec<_>>()
        .into();

    let cached = CachedLeaderboard {
        season_id,
        entries,
        fetched_at: Utc::now(),
    };

    *cache.write().await = Some(cached);

    tracing::debug!(
        season_id = season_id.as_i64(),
        players_synced = stats.players.len(),
        "leaderboard_synced"
    );

    Ok(())
}

/// Write mod config JSON for the Lua mod to read.
/// `data_dir` is the host-absolute path (e.g. ~/Zomboid/Lua/ZomboidSeasons/).
/// PZ sandboxes file I/O under ~/Zomboid/Lua/, so we strip that prefix to get
/// the relative path the Lua mod should use with getFileReader/getFileWriter.
///
/// Includes a `steam_id_map` (username -> correct steam_id) so the Lua mod can
/// look up authoritative steam IDs instead of relying on Kahlua2's lossy
/// double-coercion of Java longs.
pub async fn write_mod_config(data_dir: &str, repo: &SqliteRepo) -> AppResult<()> {
    let dir = Path::new(data_dir);
    tokio::fs::create_dir_all(dir).await?;
    let config_path = dir.join("config.json");

    // Derive PZ-relative path: strip everything up to and including "Lua/"
    let pz_relative = dir
        .iter()
        .skip_while(|c| c.to_str() != Some("Lua"))
        .skip(1) // skip "Lua" itself
        .collect::<std::path::PathBuf>();
    let pz_data_dir = if pz_relative.as_os_str().is_empty() {
        // Fallback: use last path component
        dir.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("ZomboidSeasons")
            .to_string()
    } else {
        pz_relative.to_string_lossy().to_string()
    };

    // Build username -> steam_id map from accounts table.
    // display_name is the zomboid username set via upsert_account().
    let rows: Vec<(String, String)> =
        sqlx::query_as("SELECT display_name, steam_id FROM accounts WHERE display_name != ''")
            .fetch_all(repo.pool())
            .await?;

    let steam_id_map: serde_json::Map<String, serde_json::Value> = rows
        .into_iter()
        .map(|(name, sid)| (name, serde_json::Value::String(sid)))
        .collect();

    let json = serde_json::json!({
        "data_dir": pz_data_dir,
        "sync_interval_minutes": 5,
        "steam_id_map": steam_id_map
    });
    tokio::fs::write(&config_path, serde_json::to_string_pretty(&json)?).await?;
    tracing::info!(path = %config_path.display(), entries = steam_id_map.len(), "mod_config_written");
    Ok(())
}
