pub mod api;
pub mod config;
pub mod crypto;
pub mod db;
pub mod domain;
pub mod error;
pub mod rate_limit;
pub mod services;

use std::path::PathBuf;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::Serialize;
use tokio::sync::RwLock;

use crate::config::AppConfig;
use crate::db::SqliteRepo;
use crate::domain::SeasonId;
use crate::services::gametime::GameTimeCache;
use crate::services::rcon::RconConfig;

// ---------------------------------------------------------------------------
// Leaderboard cache
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct CachedLeaderboard {
    pub season_id: SeasonId,
    pub entries: Arc<[LeaderboardEntry]>,
    pub fetched_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LeaderboardEntry {
    pub rank: u32,
    pub zomboid_username: String,
    pub zombie_kills: u64,
}

pub type LeaderboardCache = Arc<RwLock<Option<CachedLeaderboard>>>;

// ---------------------------------------------------------------------------
// App state
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct AppState {
    pub repo: SqliteRepo,
    pub archive_dir: PathBuf,
    pub deposit_address: String,
    pub base_fee_wei: String,
    pub admin_token: Arc<str>,
    pub static_dir: PathBuf,
    pub config: Arc<AppConfig>,
    pub rcon_config: Arc<RconConfig>,
    pub leaderboard_cache: LeaderboardCache,
    pub gametime_cache: GameTimeCache,
}
