use std::path::Path;

use chrono::Utc;
use serde::Deserialize;
use uuid::Uuid;

use crate::config::EventConfig;
use crate::db::{RewardRepo, SeasonRepo, SqliteRepo};
use crate::domain::{p_kills, p_survival};
use crate::error::AppResult;

// ---------------------------------------------------------------------------
// JSON shapes from the Lua mod
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct EventsFile {
    #[serde(default)]
    events: Vec<GameEvent>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct GameEvent {
    #[serde(rename = "type")]
    event_type: String,
    steam_id: String,
    username: String,
    #[serde(default)]
    payload: serde_json::Value,
    #[serde(default)]
    ts: f64,
    key: String,
}

// ---------------------------------------------------------------------------
// Poller entry point
// ---------------------------------------------------------------------------

pub fn spawn_event_poller(repo: SqliteRepo, config: EventConfig) -> tokio::task::JoinHandle<()> {
    let interval = std::time::Duration::from_secs(config.poll_interval_secs);
    let events_file = config.events_file.clone();

    tracing::info!(
        events_file = %events_file,
        poll_interval_secs = config.poll_interval_secs,
        "event_poller_started"
    );

    tokio::spawn(async move {
        loop {
            tokio::time::sleep(interval).await;

            if let Err(e) = poll_once(&repo, &events_file).await {
                tracing::warn!(error = %e, "event_poll_error");
            }
        }
    })
}

// ---------------------------------------------------------------------------
// Single poll cycle
// ---------------------------------------------------------------------------

async fn poll_once(repo: &SqliteRepo, events_file: &str) -> AppResult<()> {
    let path = Path::new(events_file);
    if !path.exists() {
        return Ok(());
    }

    let content = tokio::fs::read_to_string(path).await?;
    let file: EventsFile = match serde_json::from_str(&content) {
        Ok(f) => f,
        Err(e) => {
            tracing::debug!(error = %e, "events_file_parse_skip");
            return Ok(());
        }
    };

    if file.events.is_empty() {
        return Ok(());
    }

    let season = repo.get_active_season().await?;
    let season_id = season.id.as_i64();

    for event in &file.events {
        if let Err(e) = process_event(repo, season_id, event).await {
            tracing::warn!(
                event_type = %event.event_type,
                steam_id = %event.steam_id,
                key = %event.key,
                error = %e,
                "event_process_error"
            );
        }
    }

    // Truncate the file to prevent re-processing (idempotency handles duplicates,
    // but this avoids growing the file unboundedly).
    let _ = tokio::fs::write(path, b"{\"events\":[]}").await;

    tracing::debug!(count = file.events.len(), "events_processed");
    Ok(())
}

// ---------------------------------------------------------------------------
// Per-event dispatch
// ---------------------------------------------------------------------------

async fn process_event(repo: &SqliteRepo, season_id: i64, event: &GameEvent) -> AppResult<()> {
    let payload_str = event.payload.to_string();
    let event_id = Uuid::new_v4().to_string();

    match event.event_type.as_str() {
        "character_created" => {
            let inserted = repo
                .insert_game_event(
                    &event_id,
                    &event.steam_id,
                    season_id,
                    None,
                    "character_created",
                    &payload_str,
                    &event.key,
                )
                .await?;

            if !inserted {
                return Ok(()); // duplicate
            }

            repo.upsert_account(&event.steam_id, &event.username)
                .await?;

            let char_id = Uuid::new_v4().to_string();
            let world_age = event
                .payload
                .get("world_age_hours")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);

            repo.create_character_snapshot(
                &char_id,
                &event.steam_id,
                season_id,
                &event.username,
                world_age,
            )
            .await?;

            tracing::debug!(
                steam_id = %event.steam_id,
                character_id = %char_id,
                "character_created"
            );
        }

        "zombie_kill" => {
            let inserted = repo
                .insert_game_event(
                    &event_id,
                    &event.steam_id,
                    season_id,
                    None,
                    "zombie_kill",
                    &payload_str,
                    &event.key,
                )
                .await?;

            if !inserted {
                return Ok(());
            }

            // Increment on active character snapshot
            if let Some((char_id, _, _, _)) = repo
                .get_active_character(&event.steam_id, season_id)
                .await?
            {
                repo.increment_character_kills(&char_id).await?;
            }

            // Backward compat note: player_stats is still updated by the
            // existing kills.json leaderboard poller. No need to duplicate here.
        }

        "player_death" => {
            let inserted = repo
                .insert_game_event(
                    &event_id,
                    &event.steam_id,
                    season_id,
                    None,
                    "player_death",
                    &payload_str,
                    &event.key,
                )
                .await?;

            if !inserted {
                return Ok(());
            }

            if let Some((char_id, created_world_age, _online_mins, hours_survived_db)) = repo
                .get_active_character(&event.steam_id, season_id)
                .await?
            {
                // Prefer hours_survived from PZ native tracking (payload or DB),
                // fall back to world-age delta
                let hours_survived = event
                    .payload
                    .get("hours_survived")
                    .and_then(|v| v.as_f64())
                    .filter(|&h| h > 0.0)
                    .or(if hours_survived_db > 0.0 {
                        Some(hours_survived_db)
                    } else {
                        None
                    });

                let game_days = if let Some(hours) = hours_survived {
                    hours / 24.0
                } else {
                    let current_world_age = event
                        .payload
                        .get("world_age_hours")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0);
                    ((current_world_age - created_world_age) / 24.0).max(0.0)
                };

                // Read kills from the death event payload
                let kills = event
                    .payload
                    .get("kills")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);

                let pk = p_kills(kills);
                let ps = p_survival(game_days);
                let died_at = Utc::now().to_rfc3339();

                repo.freeze_character(&char_id, &died_at, game_days, pk, ps)
                    .await?;

                tracing::debug!(
                    steam_id = %event.steam_id,
                    character_id = %char_id,
                    game_days = game_days,
                    "character_frozen"
                );
            }
        }

        "sync_survival" => {
            // Update hours_survived for online players (sent every minute)
            let hours_survived = event
                .payload
                .get("hours_survived")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);

            if hours_survived > 0.0 {
                repo.update_character_hours_survived(&event.steam_id, season_id, hours_survived)
                    .await?;
            }
        }

        "login" => {
            repo.insert_game_event(
                &event_id,
                &event.steam_id,
                season_id,
                None,
                "login",
                &payload_str,
                &event.key,
            )
            .await?;
        }

        "logout" => {
            let inserted = repo
                .insert_game_event(
                    &event_id,
                    &event.steam_id,
                    season_id,
                    None,
                    "logout",
                    &payload_str,
                    &event.key,
                )
                .await?;

            if !inserted {
                return Ok(());
            }

            // Record online minutes if provided
            let minutes = event
                .payload
                .get("minutes_online")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);

            if minutes > 0 {
                let date = Utc::now().format("%Y-%m-%d").to_string();
                repo.record_online_minutes(&event.steam_id, season_id, &date, minutes)
                    .await?;

                // Accumulate online minutes on the active (alive) character
                repo.add_character_online_minutes(&event.steam_id, season_id, minutes)
                    .await?;
            }

            // Update hours_survived from PZ's native tracking
            let hours_survived = event
                .payload
                .get("hours_survived")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);

            if hours_survived > 0.0 {
                repo.update_character_hours_survived(&event.steam_id, season_id, hours_survived)
                    .await?;
            }
        }

        other => {
            tracing::debug!(event_type = %other, "unknown_event_type_skipped");
        }
    }

    Ok(())
}
