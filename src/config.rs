use std::path::Path;

use rand::RngExt;
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub zomboid: ZomboidConfig,
    pub eth: EthConfig,
    pub schedule: ScheduleConfig,
    pub admin: AdminConfig,
    pub database: DatabaseConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub leaderboard: LeaderboardConfig,
    #[serde(default)]
    pub supply_drops: SupplyDropConfig,
    #[serde(default)]
    pub events: EventConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub bind: String,
    pub static_dir: String,
    #[serde(default)]
    pub public_url: String,
    #[serde(default = "default_rate_limit_per_second")]
    pub rate_limit_per_second: u64,
    #[serde(default = "default_rate_limit_burst")]
    pub rate_limit_burst: u32,
}

fn default_rate_limit_per_second() -> u64 {
    50
}
fn default_rate_limit_burst() -> u32 {
    100
}

#[derive(Debug, Clone, Deserialize)]
pub struct ZomboidConfig {
    pub server_name: String,
    pub saves_dir: String,
    pub config_dir: String,
    pub archive_dir: String,
    pub rcon_host: String,
    pub rcon_port: u16,
    pub rcon_pw_file: String,
    #[serde(default)]
    pub game_version: String,
    #[serde(default = "default_sandbox_preset")]
    pub sandbox_preset: String,
    #[serde(default = "default_max_players")]
    pub max_players: u32,
    #[serde(default = "default_pz_install_dir")]
    pub pz_install_dir: String,
}

fn default_sandbox_preset() -> String {
    "Outbreak".to_string()
}

fn default_max_players() -> u32 {
    20
}

fn default_pz_install_dir() -> String {
    "/home/pzuser/pzserver".to_string()
}

#[derive(Debug, Clone, Deserialize)]
pub struct EthConfig {
    pub rpc_url: String,
    pub deposit_address: String,
    #[serde(default = "default_base_fee_wei")]
    pub base_fee_wei: String,
    #[serde(default = "default_payment_expiry_hours")]
    pub payment_expiry_hours: u64,
}

fn default_base_fee_wei() -> String {
    "1000000000000000".to_string()
}

fn default_payment_expiry_hours() -> u64 {
    48
}

#[derive(Debug, Clone, Deserialize)]
pub struct ScheduleConfig {
    #[serde(default = "default_rotation_month")]
    pub rotation_month: u8,
    #[serde(default = "default_rotation_day_of_month")]
    pub rotation_day_of_month: u8,
    #[serde(default = "default_rotation_hour")]
    pub rotation_hour: u8,
}

fn default_rotation_month() -> u8 {
    1
}

fn default_rotation_day_of_month() -> u8 {
    1
}

fn default_rotation_hour() -> u8 {
    4
}

#[derive(Debug, Clone, Deserialize)]
pub struct AdminConfig {
    pub token: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    /// Base64-encoded 32-byte AES-256 key for encrypting passwords at rest.
    /// Generate with: `openssl rand -base64 32`
    pub encryption_key: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub format: String,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            format: "json".to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct LeaderboardConfig {
    /// Host path where PZ Lua sandbox writes files (~/Zomboid/Lua/ZomboidSeasons/)
    #[serde(default = "default_data_dir")]
    pub data_dir: String,
    /// Path to kills.json written by the Lua mod (inside PZ Lua sandbox)
    #[serde(default = "default_kills_file")]
    pub kills_file: String,
    #[serde(default = "default_poll_interval_secs")]
    pub poll_interval_secs: u64,
    #[serde(default = "default_leaderboard_size")]
    pub leaderboard_size: u32,
    #[serde(default = "default_top_killers_reward_count")]
    pub top_killers_reward_count: u32,
}

impl Default for LeaderboardConfig {
    fn default() -> Self {
        Self {
            data_dir: default_data_dir(),
            kills_file: default_kills_file(),
            poll_interval_secs: default_poll_interval_secs(),
            leaderboard_size: default_leaderboard_size(),
            top_killers_reward_count: default_top_killers_reward_count(),
        }
    }
}

fn default_data_dir() -> String {
    "/home/pzuser/Zomboid/Lua/ZomboidSeasons".to_string()
}

fn default_kills_file() -> String {
    "/home/pzuser/Zomboid/Lua/ZomboidSeasons/kills.json".to_string()
}

fn default_poll_interval_secs() -> u64 {
    60
}

fn default_leaderboard_size() -> u32 {
    50
}

fn default_top_killers_reward_count() -> u32 {
    3
}

#[derive(Debug, Clone, Deserialize)]
pub struct SupplyDropConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_min_interval_hours")]
    pub min_interval_hours: f64,
    #[serde(default = "default_max_interval_hours")]
    pub max_interval_hours: f64,
    #[serde(default = "default_announce_minutes")]
    pub announce_minutes: u64,
    #[serde(default = "default_expire_hours")]
    pub expire_hours: f64,
    #[serde(default = "default_drops_poll_secs")]
    pub poll_interval_secs: u64,
    #[serde(default = "default_drops_claimed_file")]
    pub claimed_file: String,
    #[serde(default)]
    pub pois: Vec<PoiConfig>,
    /// Weighted probability table for random loot tier selection.
    /// Tier is picked randomly on every drop. No per-POI overrides.
    #[serde(default = "default_loot_weights")]
    pub loot_weights: Vec<LootWeight>,
}

impl Default for SupplyDropConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            min_interval_hours: default_min_interval_hours(),
            max_interval_hours: default_max_interval_hours(),
            announce_minutes: default_announce_minutes(),
            expire_hours: default_expire_hours(),
            poll_interval_secs: default_drops_poll_secs(),
            claimed_file: default_drops_claimed_file(),
            pois: Vec::new(),
            loot_weights: default_loot_weights(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LootWeight {
    pub tier: String,
    pub weight: u32,
}

fn default_loot_weights() -> Vec<LootWeight> {
    vec![
        LootWeight {
            tier: "standard".into(),
            weight: 25,
        },
        LootWeight {
            tier: "mixed".into(),
            weight: 20,
        },
        LootWeight {
            tier: "survival".into(),
            weight: 15,
        },
        LootWeight {
            tier: "medical".into(),
            weight: 15,
        },
        LootWeight {
            tier: "builder".into(),
            weight: 10,
        },
        LootWeight {
            tier: "tactical".into(),
            weight: 10,
        },
        LootWeight {
            tier: "military".into(),
            weight: 5,
        },
    ]
}

impl SupplyDropConfig {
    /// Pick a random loot tier using weighted probabilities.
    pub fn random_loot_tier(&self) -> String {
        let total: u32 = self.loot_weights.iter().map(|w| w.weight).sum();
        let mut roll = rand::rng().random_range(0..total);
        for w in &self.loot_weights {
            if roll < w.weight {
                return w.tier.clone();
            }
            roll -= w.weight;
        }
        unreachable!("loot_weights has defaults and total > 0")
    }
}

fn default_min_interval_hours() -> f64 {
    6.0
}

fn default_max_interval_hours() -> f64 {
    12.0
}

fn default_announce_minutes() -> u64 {
    15
}

fn default_expire_hours() -> f64 {
    2.0
}

fn default_drops_poll_secs() -> u64 {
    30
}

fn default_drops_claimed_file() -> String {
    "/home/pzuser/Zomboid/Lua/ZomboidSeasons/drops_claimed.json".to_string()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PoiConfig {
    pub name: String,
    pub x: i32,
    pub y: i32,
    #[serde(default)]
    pub z: i32,
    #[serde(default = "default_loot_tier")]
    pub loot_tier: String,
}

fn default_loot_tier() -> String {
    "standard".to_string()
}

#[derive(Debug, Clone, Deserialize)]
pub struct EventConfig {
    #[serde(default = "default_events_file")]
    pub events_file: String,
    #[serde(default = "default_event_poll_secs")]
    pub poll_interval_secs: u64,
}

impl Default for EventConfig {
    fn default() -> Self {
        Self {
            events_file: default_events_file(),
            poll_interval_secs: default_event_poll_secs(),
        }
    }
}

fn default_events_file() -> String {
    "/home/pzuser/Zomboid/Lua/ZomboidSeasons/events.json".to_string()
}

fn default_event_poll_secs() -> u64 {
    30
}

impl AppConfig {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(toml_str: &str) -> AppResult<Self> {
        toml::from_str(toml_str).map_err(|e| AppError::Config(e.to_string()))
    }

    pub fn from_file(path: &Path) -> AppResult<Self> {
        let contents = std::fs::read_to_string(path).map_err(|e| {
            AppError::Config(format!(
                "failed to read config file {}: {e}",
                path.display()
            ))
        })?;
        Self::from_str(&contents)
    }
}
