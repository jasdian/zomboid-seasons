use std::path::Path;
use std::sync::Arc;

use chrono::{Duration, Utc};
use rand::RngExt;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::config::AppConfig;
use crate::db::{SeasonRepo, SqliteRepo, SupplyDropRepo};
use crate::domain::{DropStatus, SupplyDrop};
use crate::services::rcon::{rcon_command_best_effort, RconConfig};

// ---------------------------------------------------------------------------
// File exchange types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
pub struct PendingFile {
    pub commands: Vec<PendingCommand>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PendingCommand {
    pub action: String,
    pub drop_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poi_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub z: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loot_tier: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ClaimedFile {
    claims: Vec<ClaimEntry>,
}

#[derive(Debug, Deserialize)]
struct ClaimEntry {
    drop_id: String,
    steam_id: String,
    username: String,
}

// ---------------------------------------------------------------------------
// Drop Scheduler
// ---------------------------------------------------------------------------

pub fn spawn_drop_scheduler(
    repo: SqliteRepo,
    config: Arc<AppConfig>,
    rcon_config: Arc<RconConfig>,
) -> tokio::task::JoinHandle<()> {
    let interval = std::time::Duration::from_secs(config.supply_drops.poll_interval_secs);

    tracing::info!(
        poll_interval_secs = config.supply_drops.poll_interval_secs,
        pois = config.supply_drops.pois.len(),
        "drop_scheduler_started"
    );

    tokio::spawn(async move {
        loop {
            tokio::time::sleep(interval).await;

            if let Err(e) = scheduler_tick(&repo, &config, &rcon_config).await {
                tracing::warn!(error = %e, "drop_scheduler_error");
            }
        }
    })
}

async fn scheduler_tick(
    repo: &SqliteRepo,
    config: &AppConfig,
    rcon_config: &RconConfig,
) -> crate::error::AppResult<()> {
    let season = repo.get_active_season().await?;
    let now = Utc::now();
    let sd_config = &config.supply_drops;

    // 1. Hard cleanup — expire stale drops
    let stale = repo.get_stale_drops(now).await?;
    for drop in &stale {
        write_pending_command(
            &config.leaderboard.data_dir,
            PendingCommand {
                action: "despawn".to_string(),
                drop_id: drop.id.to_string(),
                poi_name: None,
                x: None,
                y: None,
                z: None,
                loot_tier: None,
            },
        )
        .await;
        repo.update_drop_status(drop.id, DropStatus::Expired, now)
            .await?;
        rcon_command_best_effort(
            rcon_config,
            &format!(
                "servermsg \"The supply drop near {} has expired.\"",
                drop.poi_name
            ),
        )
        .await;
        tracing::info!(drop_id = %drop.id, poi = %drop.poi_name, "supply_drop_expired");
    }

    // 2. Advance lifecycle: scheduled -> announced
    let scheduled = repo.get_drops_by_status(DropStatus::Scheduled).await?;
    for drop in &scheduled {
        if drop.scheduled_at <= now {
            rcon_command_best_effort(
                rcon_config,
                &format!(
                    "servermsg \"SUPPLY DROP incoming near {} in {} minutes!\"",
                    drop.poi_name, sd_config.announce_minutes
                ),
            )
            .await;
            repo.update_drop_status(drop.id, DropStatus::Announced, now)
                .await?;
            tracing::info!(drop_id = %drop.id, poi = %drop.poi_name, "supply_drop_announced");
        }
    }

    // 3. Advance lifecycle: announced -> active
    let announced = repo.get_drops_by_status(DropStatus::Announced).await?;
    let announce_dur = Duration::minutes(sd_config.announce_minutes as i64);
    for drop in &announced {
        if let Some(ann_at) = drop.announced_at {
            if ann_at + announce_dur <= now {
                write_pending_command(
                    &config.leaderboard.data_dir,
                    PendingCommand {
                        action: "spawn".to_string(),
                        drop_id: drop.id.to_string(),
                        poi_name: Some(drop.poi_name.clone()),
                        x: Some(drop.location_x),
                        y: Some(drop.location_y),
                        z: Some(drop.location_z),
                        loot_tier: Some(drop.loot_tier.clone()),
                    },
                )
                .await;
                rcon_command_best_effort(
                    rcon_config,
                    &format!(
                        "servermsg \"The drop near {} is live. Move fast.\"",
                        drop.poi_name
                    ),
                )
                .await;
                repo.update_drop_status(drop.id, DropStatus::Active, now)
                    .await?;
                tracing::info!(drop_id = %drop.id, poi = %drop.poi_name, "supply_drop_activated");
            }
        }
    }

    // 4. Maybe schedule a new drop
    if sd_config.pois.is_empty() {
        return Ok(());
    }

    let last_drop_time = repo.get_last_drop_time(season.id).await?;
    let should_schedule = match last_drop_time {
        None => true,
        Some(last) => {
            let min_secs = (sd_config.min_interval_hours * 3600.0) as i64;
            let max_secs = (sd_config.max_interval_hours * 3600.0) as i64;
            let range_secs = if max_secs > min_secs {
                rand::rng().random_range(min_secs..=max_secs)
            } else {
                min_secs
            };
            let next_after = last + Duration::seconds(range_secs);
            now >= next_after
        }
    };

    if should_schedule {
        let poi_idx = rand::rng().random_range(0..sd_config.pois.len());
        let poi = &sd_config.pois[poi_idx];

        // Force-clean if occupied
        if repo.has_active_drop_at_poi(season.id, &poi.name).await? {
            repo.force_expire_at_poi(season.id, &poi.name).await?;
            write_pending_command(
                &config.leaderboard.data_dir,
                PendingCommand {
                    action: "despawn".to_string(),
                    drop_id: "cleanup".to_string(),
                    poi_name: Some(poi.name.clone()),
                    x: None,
                    y: None,
                    z: None,
                    loot_tier: None,
                },
            )
            .await;
        }

        let expire_hours = sd_config.expire_hours.min(sd_config.max_interval_hours);
        let expires_at = now + Duration::seconds((expire_hours * 3600.0) as i64);

        let loot_tier = sd_config.random_loot_tier();

        let drop = SupplyDrop {
            id: Uuid::new_v4(),
            season_id: season.id,
            poi_name: poi.name.clone(),
            location_x: poi.x,
            location_y: poi.y,
            location_z: poi.z,
            status: DropStatus::Scheduled,
            loot_tier,
            scheduled_at: now,
            announced_at: None,
            activated_at: None,
            claimed_by_steam_id: None,
            claimed_by_username: None,
            claimed_at: None,
            expires_at,
            created_at: now,
        };

        repo.create_supply_drop(&drop).await?;
        tracing::info!(
            drop_id = %drop.id,
            poi = %drop.poi_name,
            expires_at = %drop.expires_at,
            "supply_drop_scheduled"
        );
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Claim Poller
// ---------------------------------------------------------------------------

pub fn spawn_claim_poller(
    repo: SqliteRepo,
    config: Arc<AppConfig>,
    rcon_config: Arc<RconConfig>,
) -> tokio::task::JoinHandle<()> {
    let interval = std::time::Duration::from_secs(config.supply_drops.poll_interval_secs);
    let claimed_file = config.supply_drops.claimed_file.clone();

    tracing::info!(
        claimed_file = %claimed_file,
        "claim_poller_started"
    );

    tokio::spawn(async move {
        loop {
            tokio::time::sleep(interval).await;

            if let Err(e) = poll_claims(&repo, &claimed_file, &rcon_config).await {
                tracing::warn!(error = %e, "claim_poller_error");
            }
        }
    })
}

async fn poll_claims(
    repo: &SqliteRepo,
    claimed_file: &str,
    rcon_config: &RconConfig,
) -> crate::error::AppResult<()> {
    let path = Path::new(claimed_file);
    if !path.exists() {
        return Ok(());
    }

    let content = tokio::fs::read_to_string(path).await?;
    let file: ClaimedFile = match serde_json::from_str(&content) {
        Ok(f) => f,
        Err(e) => {
            tracing::debug!(error = %e, "drops_claimed_parse_skip");
            return Ok(());
        }
    };

    if file.claims.is_empty() {
        return Ok(());
    }

    let now = Utc::now();
    for claim in &file.claims {
        let drop_id = match Uuid::parse_str(&claim.drop_id) {
            Ok(id) => id,
            Err(e) => {
                tracing::warn!(drop_id = %claim.drop_id, error = %e, "invalid_drop_id_in_claim");
                continue;
            }
        };

        // Check if already claimed (idempotent)
        if let Some(existing) = repo.get_drop_by_id(drop_id).await? {
            if existing.status == DropStatus::Claimed {
                continue;
            }

            repo.claim_drop(drop_id, &claim.steam_id, &claim.username, now)
                .await?;

            rcon_command_best_effort(
                rcon_config,
                &format!(
                    "servermsg \"{} secured the supply drop near {}!\"",
                    claim.username, existing.poi_name
                ),
            )
            .await;

            tracing::info!(
                drop_id = %drop_id,
                username = %claim.username,
                poi = %existing.poi_name,
                "supply_drop_claimed"
            );
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Pending file writer
// ---------------------------------------------------------------------------

async fn write_pending_command(data_dir: &str, cmd: PendingCommand) {
    let path = format!("{}/drops_pending.json", data_dir);
    let path = Path::new(&path);

    // Read existing
    let mut pending = if path.exists() {
        match tokio::fs::read_to_string(path).await {
            Ok(content) => serde_json::from_str::<PendingFile>(&content).unwrap_or(PendingFile {
                commands: Vec::new(),
            }),
            Err(_) => PendingFile {
                commands: Vec::new(),
            },
        }
    } else {
        PendingFile {
            commands: Vec::new(),
        }
    };

    pending.commands.push(cmd);

    match serde_json::to_string_pretty(&pending) {
        Ok(json) => {
            if let Err(e) = tokio::fs::write(path, json).await {
                tracing::warn!(error = %e, "drops_pending_write_failed");
            }
        }
        Err(e) => {
            tracing::warn!(error = %e, "drops_pending_serialize_failed");
        }
    }
}
