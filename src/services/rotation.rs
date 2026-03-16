use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use chrono::{TimeDelta, Utc};

use crate::config::AppConfig;
use crate::db::{RewardRepo, RotationRepo, SeasonRepo, SqliteRepo};
use crate::domain::SeasonId;
use crate::error::{AppError, AppResult};
use crate::services::rcon::{rcon_command_best_effort, RconConfig};
use crate::services::whitelist;

static ROTATING: AtomicBool = AtomicBool::new(false);

/// Drop guard that resets ROTATING to false when dropped, ensuring the lock
/// is released even if the rotation function returns early via `?` or panic.
struct RotationGuard;

impl Drop for RotationGuard {
    fn drop(&mut self) {
        ROTATING.store(false, Ordering::SeqCst);
    }
}

/// Read world_age_hours from gametime.json. Returns 0.0 if unavailable.
async fn read_world_age(data_dir: &str) -> f64 {
    let path = format!("{data_dir}/gametime.json");
    match tokio::fs::read_to_string(&path).await {
        Ok(content) => {
            // Parse just the world_age_hours field
            #[derive(serde::Deserialize)]
            struct Gt {
                world_age_hours: Option<f64>,
            }
            match serde_json::from_str::<Gt>(&content) {
                Ok(gt) => gt.world_age_hours.unwrap_or(0.0),
                Err(_) => 0.0,
            }
        }
        Err(_) => 0.0,
    }
}

pub async fn rotate_season(
    repo: &SqliteRepo,
    config: &AppConfig,
    rcon_config: &RconConfig,
    force: bool,
) -> AppResult<()> {
    if ROTATING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err(AppError::Rotation(
            "rotation already in progress".to_string(),
        ));
    }
    let _guard = RotationGuard;

    let started = Instant::now();
    let current = repo.get_active_season().await?;
    let new_id = current.id.next();

    // Step 0: Guard
    if !force {
        let now = Utc::now();
        if now < current.ends_at {
            tracing::info!(
                season_id = current.id.as_i64(),
                ends_at = %current.ends_at,
                "rotation_skipped_not_ended"
            );
            return Ok(());
        }
    }

    tracing::info!(
        current_season = current.id.as_i64(),
        new_season = new_id.as_i64(),
        "rotation_started"
    );

    // Step 1: RCON /save then /quit
    tracing::info!(step = "save_game", "rotation_step");
    rcon_command_best_effort(rcon_config, "save").await;
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

    tracing::info!(step = "quit_server", "rotation_step");
    rcon_command_best_effort(rcon_config, "quit").await;
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    // Step 2: systemctl stop
    tracing::info!(step = "systemctl_stop", "rotation_step");
    run_systemctl("stop").await?;

    // Step 3: Archive save directory as .tar.gz
    tracing::info!(step = "archive_save", "rotation_step");
    let archive_dir = Path::new(&config.zomboid.archive_dir);
    let saves_dir = Path::new(&config.zomboid.saves_dir);
    let save_dir = saves_dir.join(&config.zomboid.server_name);
    let archive_path = archive_dir.join(format!("season_{}.tar.gz", current.id.as_i64()));

    tokio::fs::create_dir_all(archive_dir)
        .await
        .map_err(|e| AppError::Rotation(format!("failed to create archive dir: {e}")))?;

    if save_dir.exists() {
        archive_directory(&save_dir, &archive_path).await?;
    } else {
        tracing::warn!(
            path = %save_dir.display(),
            "save directory not found, skipping archive"
        );
    }

    // Step 4: DB archive season
    tracing::info!(step = "archive_season_db", "rotation_step");
    repo.archive_season(current.id).await?;
    repo.update_season_save_path(current.id, &archive_path.to_string_lossy())
        .await?;

    // Step 5: DB create new season (+90 days)
    tracing::info!(step = "create_season_db", "rotation_step");
    let now = Utc::now();
    let ends_at = now + TimeDelta::days(90);
    repo.create_season(new_id, now, ends_at).await?;

    // Step 6a: Finalize season — freeze alive characters, credit scores
    tracing::info!(step = "finalize_season", "rotation_step");
    if !repo.is_season_finalized(current.id.as_i64()).await? {
        // Read current world age from gametime.json for survival calculation
        let world_age = read_world_age(&config.leaderboard.data_dir).await;
        let frozen = repo
            .freeze_alive_characters(current.id.as_i64(), world_age)
            .await?;
        tracing::info!(
            frozen_characters = frozen,
            world_age_hours = world_age,
            "freeze_alive_complete"
        );

        let credited = repo.credit_season_scores(current.id.as_i64()).await?;
        tracing::info!(players_credited = credited, "season_scores_credited");

        repo.finalize_season(current.id.as_i64()).await?;
        tracing::info!(season_id = current.id.as_i64(), "season_finalized");
    } else {
        tracing::info!(season_id = current.id.as_i64(), "season_already_finalized");
    }

    // Step 6b: Carry forward top scorers (free registration in next season)
    tracing::info!(step = "carry_forward_top_killers", "rotation_step");
    let top_count = config.leaderboard.top_killers_reward_count;
    let carried = repo
        .carry_forward_top_killers(current.id, new_id, top_count)
        .await?;
    tracing::info!(
        from_season = current.id.as_i64(),
        to_season = new_id.as_i64(),
        top_killers_carried = carried,
        max_slots = top_count,
        "carry_forward_top_killers_complete"
    );

    // Step 7: Delete save directory (PZ generates new world on restart)
    tracing::info!(step = "delete_save", "rotation_step");
    if save_dir.exists() {
        tokio::fs::remove_dir_all(&save_dir).await.map_err(|e| {
            AppError::Rotation(format!(
                "failed to delete save dir {}: {e}",
                save_dir.display()
            ))
        })?;
    }

    // Step 7a: Reset gametime.json so the new season starts fresh
    tracing::info!(step = "reset_gametime", "rotation_step");
    let gametime_path = format!("{}/gametime.json", config.leaderboard.data_dir);
    let gametime_reset = r#"{"month":null,"day":null,"hour":null,"minute":null}"#;
    if let Err(e) = tokio::fs::write(&gametime_path, gametime_reset).await {
        tracing::warn!(error = %e, path = %gametime_path, "gametime_reset_failed");
    }

    // Step 7b: Write sandbox preset (Outbreak mode)
    tracing::info!(step = "write_sandbox_preset", "rotation_step");
    if let Err(e) = write_sandbox_preset(config).await {
        tracing::warn!(error = %e, "sandbox_preset_write_failed");
    }

    // Step 8: Update server name in .ini
    tracing::info!(step = "update_server_name", "rotation_step");
    if let Err(e) = update_server_ini(config, new_id).await {
        tracing::warn!(error = %e, "server_ini_update_failed");
    }

    // Step 9: systemctl start
    tracing::info!(step = "systemctl_start", "rotation_step");
    run_systemctl("start").await?;

    // Step 10: Wait for RCON ready, then whitelist carried-forward players
    tracing::info!(step = "whitelist_players", "rotation_step");
    wait_for_rcon(rcon_config).await;

    let players = repo.get_confirmed_players(new_id).await?;
    for (username, password, steam_id) in &players {
        whitelist::whitelist_player(rcon_config, &config.zomboid, username, password, steam_id)
            .await;
    }

    // Step 11: Purge old archives
    tracing::info!(step = "purge_archives", "rotation_step");
    if let Err(e) = purge_old_archives(repo, archive_dir, 3).await {
        tracing::warn!(error = %e, "archive_purge_failed");
    }

    let duration_ms = started.elapsed().as_millis();
    tracing::info!(
        new_season = new_id.as_i64(),
        duration_ms = duration_ms as u64,
        "rotation_complete"
    );

    Ok(())
}

async fn run_systemctl(action: &str) -> AppResult<()> {
    let output = tokio::process::Command::new("systemctl")
        .args([action, "zomboid.service"])
        .output()
        .await
        .map_err(|e| AppError::Rotation(format!("failed to run systemctl {action}: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::Rotation(format!(
            "systemctl {action} failed: {stderr}"
        )));
    }
    Ok(())
}

async fn archive_directory(src: &Path, dest: &Path) -> AppResult<()> {
    let output = tokio::process::Command::new("tar")
        .args([
            "-czf",
            &dest.to_string_lossy(),
            "-C",
            &src.parent().unwrap_or(src).to_string_lossy(),
            src.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .as_ref(),
        ])
        .output()
        .await
        .map_err(|e| AppError::Rotation(format!("failed to run tar: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::Rotation(format!("tar archive failed: {stderr}")));
    }
    Ok(())
}

pub async fn resolve_public_ip(public_url: &str) -> AppResult<String> {
    use std::net::ToSocketAddrs;

    let lookup = format!("{public_url}:0");
    let addr = tokio::task::spawn_blocking(move || {
        lookup
            .to_socket_addrs()
            .ok()
            .and_then(|mut addrs| addrs.find(|a| a.is_ipv4()))
    })
    .await
    .map_err(|e| AppError::Rotation(format!("DNS resolve task failed: {e}")))?;

    match addr {
        Some(a) => Ok(a.ip().to_string()),
        None => Err(AppError::Rotation(format!(
            "failed to resolve public IP from '{public_url}'"
        ))),
    }
}

pub async fn update_server_ini(config: &AppConfig, season_id: SeasonId) -> AppResult<()> {
    let ini_path =
        Path::new(&config.zomboid.config_dir).join(format!("{}.ini", config.zomboid.server_name));

    if !ini_path.exists() {
        tracing::warn!(path = %ini_path.display(), "server ini not found");
        return Ok(());
    }

    let contents = tokio::fs::read_to_string(&ini_path)
        .await
        .map_err(|e| AppError::Rotation(format!("failed to read server ini: {e}")))?;

    let new_name = format!(
        "[PvPvE] Zomboid Seasons S{:02} | 90-Day Wipe | Leaderboard",
        season_id.as_i64()
    );

    let public_ip = resolve_public_ip(&config.server.public_url).await?;
    tracing::info!(%public_ip, public_url = %config.server.public_url, "resolved_public_ip");

    let description = format!(
        "Seasonal survival server. Register at {url} to play. Pay ETH, get whitelisted. Season resets every 90 days.",
        url = config.server.public_url
    );

    let mut updated = String::new();
    let mut found_announced_ip = false;
    let mut found_open = false;
    let mut found_auto_create = false;
    let mut found_description = false;
    let mut found_sleep_allowed = false;
    let mut found_sleep_needed = false;
    let mut found_pause_empty = false;
    let mut found_max_players = false;
    let mut found_pvp = false;
    let mut found_safety_system = false;
    for line in contents.lines() {
        if line.starts_with("PublicName=") {
            updated.push_str(&format!("PublicName={new_name}"));
        } else if line.starts_with("PublicDescription=") {
            updated.push_str(&format!("PublicDescription={description}"));
            found_description = true;
        } else if line.starts_with("server_browser_announced_ip=") {
            updated.push_str(&format!("server_browser_announced_ip={public_ip}"));
            found_announced_ip = true;
        } else if line.starts_with("Open=") {
            updated.push_str("Open=false");
            found_open = true;
        } else if line.starts_with("AutoCreateUserInWhiteList=") {
            updated.push_str("AutoCreateUserInWhiteList=false");
            found_auto_create = true;
        } else if line.starts_with("SleepAllowed=") {
            updated.push_str("SleepAllowed=true");
            found_sleep_allowed = true;
        } else if line.starts_with("SleepNeeded=") {
            updated.push_str("SleepNeeded=true");
            found_sleep_needed = true;
        } else if line.starts_with("PauseEmpty=") {
            updated.push_str("PauseEmpty=true");
            found_pause_empty = true;
        } else if line.starts_with("MaxPlayers=") {
            updated.push_str(&format!("MaxPlayers={}", config.zomboid.max_players));
            found_max_players = true;
        } else if line.starts_with("PVP=") {
            updated.push_str("PVP=true");
            found_pvp = true;
        } else if line.starts_with("SafetySystem=") {
            updated.push_str("SafetySystem=true");
            found_safety_system = true;
        } else if line.starts_with("Mods=") {
            // ZomboidSeasons is deployed directly into PZ's media/lua/server/,
            // NOT via the mod system. Strip it from Mods= to avoid forcing
            // clients to install it (no Workshop ID exists).
            let filtered: Vec<&str> = line
                .strip_prefix("Mods=")
                .unwrap_or("")
                .split(';')
                .map(|m| m.trim())
                .filter(|m| !m.is_empty() && *m != "ZomboidSeasons")
                .collect();
            updated.push_str(&format!("Mods={}", filtered.join(";")));
        } else {
            updated.push_str(line);
        }
        updated.push('\n');
    }

    if !found_announced_ip {
        updated.push_str(&format!("server_browser_announced_ip={public_ip}\n"));
    }
    if !found_open {
        updated.push_str("Open=false\n");
    }
    if !found_auto_create {
        updated.push_str("AutoCreateUserInWhiteList=false\n");
    }
    if !found_description {
        updated.push_str(&format!("PublicDescription={description}\n"));
    }
    if !found_sleep_allowed {
        updated.push_str("SleepAllowed=true\n");
    }
    if !found_sleep_needed {
        updated.push_str("SleepNeeded=true\n");
    }
    if !found_pause_empty {
        updated.push_str("PauseEmpty=true\n");
    }
    if !found_max_players {
        updated.push_str(&format!("MaxPlayers={}\n", config.zomboid.max_players));
    }
    if !found_pvp {
        updated.push_str("PVP=true\n");
    }
    if !found_safety_system {
        updated.push_str("SafetySystem=true\n");
    }

    tokio::fs::write(&ini_path, updated)
        .await
        .map_err(|e| AppError::Rotation(format!("failed to write server ini: {e}")))?;

    tracing::info!(server_name = %new_name, %public_ip, "server_ini_updated (Open=false, AutoCreateUserInWhiteList=false, PauseEmpty=true, SleepNeeded=true, PVP=true, SafetySystem=true)");
    Ok(())
}

/// Write the sandbox preset as SandboxVars.lua for the next season.
///
/// Reads the preset from PZ's install (e.g. media/lua/shared/Sandbox/Outbreak.lua)
/// and writes it as `{config_dir}/{server_name}_SandboxVars.lua`, which PZ loads on start.
pub async fn write_sandbox_preset(config: &AppConfig) -> AppResult<()> {
    let preset = &config.zomboid.sandbox_preset;
    let config_dir = Path::new(&config.zomboid.config_dir);
    let sandbox_vars_path =
        config_dir.join(format!("{}_SandboxVars.lua", config.zomboid.server_name));

    // Delete old SandboxVars so PZ doesn't merge with stale values
    if sandbox_vars_path.exists() {
        tokio::fs::remove_file(&sandbox_vars_path)
            .await
            .map_err(|e| AppError::Rotation(format!("failed to delete old SandboxVars: {e}")))?;
    }

    // Find the preset source: {pz_install_dir}/media/lua/shared/Sandbox/{preset}.lua
    let pz_install_dir = Path::new(&config.zomboid.pz_install_dir);
    let preset_path = pz_install_dir
        .join("media/lua/shared/Sandbox")
        .join(format!("{preset}.lua"));

    if !preset_path.exists() {
        tracing::warn!(
            preset = preset,
            path = %preset_path.display(),
            "sandbox preset file not found, PZ will use defaults"
        );
        return Ok(());
    }

    // Read preset and wrap as SandboxVars table
    let preset_content = tokio::fs::read_to_string(&preset_path)
        .await
        .map_err(|e| AppError::Rotation(format!("failed to read preset {preset}: {e}")))?;

    // The preset file contains `return { ... }`, convert to `SandboxVars = { ... }`
    let sandbox_vars = preset_content.replacen("return {", "SandboxVars = {", 1);

    // Override DayLength: 4 = 1.5 real hours per in-game day
    let sandbox_vars = if let Some(start) = sandbox_vars.find("DayLength = ") {
        let after = start + "DayLength = ".len();
        if let Some(end) = sandbox_vars[after..].find(',') {
            format!(
                "{}DayLength = 4{}",
                &sandbox_vars[..start],
                &sandbox_vars[after + end..]
            )
        } else {
            sandbox_vars
        }
    } else {
        sandbox_vars
    };

    tokio::fs::write(&sandbox_vars_path, &sandbox_vars)
        .await
        .map_err(|e| AppError::Rotation(format!("failed to write SandboxVars: {e}")))?;

    tracing::info!(
        preset = preset,
        path = %sandbox_vars_path.display(),
        "sandbox_preset_written"
    );
    Ok(())
}

async fn wait_for_rcon(rcon_config: &RconConfig) {
    for attempt in 1..=30 {
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        match crate::services::rcon::rcon_command(rcon_config, "help").await {
            Ok(_) => {
                tracing::info!(attempt, "rcon_ready");
                return;
            }
            Err(_) => {
                tracing::debug!(attempt, "rcon_not_ready_yet");
            }
        }
    }
    tracing::warn!("rcon_wait_timeout");
}

async fn purge_old_archives(repo: &SqliteRepo, archive_dir: &Path, keep: usize) -> AppResult<()> {
    let to_purge = repo.get_archived_seasons_for_purge(keep).await?;
    if to_purge.is_empty() {
        return Ok(());
    }

    let mut purged = 0u32;
    for season in &to_purge {
        if let Some(ref save_path) = season.save_path {
            let path = Path::new(save_path);
            if path.exists() {
                if let Err(e) = tokio::fs::remove_file(path).await {
                    tracing::warn!(
                        season_id = season.id.as_i64(),
                        path = %path.display(),
                        error = %e,
                        "archive_delete_failed"
                    );
                    continue;
                }
            }
            repo.update_season_save_path(season.id, "").await?;
            purged += 1;
        }
    }

    let _ = archive_dir;

    tracing::info!(purged_seasons = purged, kept = keep, "archives_purged");
    Ok(())
}
