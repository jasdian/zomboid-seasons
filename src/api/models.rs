use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::{
    DropStatus, PromoCode, RegStatus, Registration, SeasonId, SeasonStatus, SupplyDrop, Wei,
};

// ---------------------------------------------------------------------------
// Request types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub zomboid_username: String,
    pub zomboid_password: String,
    pub steam_id: String,
    pub eth_address: Option<String>,
    pub promo_code: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreatePromoRequest {
    pub code: String,
    pub discount_percent: u8,
    pub max_uses: Option<u32>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct ListRegistrationsQuery {
    pub season_id: Option<i64>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LeaderboardQuery {
    pub season_id: Option<i64>,
    pub limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct TriggerDropRequest {
    pub poi_name: String,
    pub loot_tier: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ListDropsQuery {
    pub season_id: Option<i64>,
}

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct RegisterResponse {
    pub registration_id: Uuid,
    pub status: RegStatus,
    pub amount_wei: Wei,
    pub deposit_address: Option<String>,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct SeasonInfoResponse {
    pub id: SeasonId,
    pub status: SeasonStatus,
    pub started_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
    pub player_count: u64,
    pub base_fee_wei: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub game_version: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SeasonSummary {
    pub id: SeasonId,
    pub status: SeasonStatus,
    pub started_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
    pub has_save_download: bool,
}

#[derive(Debug, Serialize)]
pub struct RegistrationStatusResponse {
    pub id: Uuid,
    pub season_id: SeasonId,
    pub zomboid_username: String,
    pub steam_id: String,
    pub status: RegStatus,
    pub amount_wei: Wei,
    pub tx_hash: Option<String>,
    pub created_at: DateTime<Utc>,
    pub confirmed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct PromoCodeResponse {
    pub code: String,
    pub discount_percent: u8,
    pub max_uses: Option<u32>,
    pub times_used: u32,
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct RegistrationAdminResponse {
    pub id: Uuid,
    pub season_id: SeasonId,
    pub zomboid_username: String,
    pub steam_id: String,
    pub eth_address: String,
    pub promo_code: Option<String>,
    pub tx_hash: Option<String>,
    pub status: RegStatus,
    pub amount_wei: Wei,
    pub created_at: DateTime<Utc>,
    pub confirmed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct RotateResponse {
    pub status: &'static str,
}

#[derive(Debug, Serialize)]
pub struct LeaderboardResponse {
    pub season_id: SeasonId,
    pub entries: Vec<LeaderboardEntryResponse>,
    pub fetched_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct LeaderboardEntryResponse {
    pub rank: u32,
    pub zomboid_username: String,
    pub season_score: i64,
    pub p_online: f64,
    pub p_kills: f64,
    pub p_survival: f64,
    pub best_kills: u64,
    pub best_survival_days: f64,
    pub days_logged: u32,
    pub characters: u32,
    /// Backward compat: same as best_kills.
    pub zombie_kills: u64,
}

#[derive(Debug, Serialize)]
pub struct AccountResponse {
    pub steam_id: String,
    pub balance: i64,
    pub daily_online_bonus: bool,
    pub seasons: Vec<AccountSeasonResponse>,
}

#[derive(Debug, Serialize)]
pub struct AccountSeasonResponse {
    pub season_id: i64,
    pub score: i64,
    pub characters: u32,
}

#[derive(Debug, Serialize)]
pub struct SupplyDropResponse {
    pub id: Uuid,
    pub season_id: SeasonId,
    pub poi_name: String,
    pub status: DropStatus,
    pub loot_tier: String,
    pub scheduled_at: DateTime<Utc>,
    pub claimed_by_username: Option<String>,
    pub claimed_at: Option<DateTime<Utc>>,
    pub expires_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Conversion impls
// ---------------------------------------------------------------------------

impl From<&Registration> for RegistrationStatusResponse {
    fn from(r: &Registration) -> Self {
        Self {
            id: r.id,
            season_id: r.season_id,
            zomboid_username: r.zomboid_username.as_str().to_string(),
            steam_id: r.steam_id.as_str().to_string(),
            status: r.status,
            amount_wei: r.amount_wei.clone(),
            tx_hash: r.tx_hash.clone(),
            created_at: r.created_at,
            confirmed_at: r.confirmed_at,
        }
    }
}

impl From<&Registration> for RegistrationAdminResponse {
    fn from(r: &Registration) -> Self {
        Self {
            id: r.id,
            season_id: r.season_id,
            zomboid_username: r.zomboid_username.as_str().to_string(),
            steam_id: r.steam_id.as_str().to_string(),
            eth_address: r.eth_address.as_str().to_string(),
            promo_code: r.promo_code.clone(),
            tx_hash: r.tx_hash.clone(),
            status: r.status,
            amount_wei: r.amount_wei.clone(),
            created_at: r.created_at,
            confirmed_at: r.confirmed_at,
        }
    }
}

impl From<&SupplyDrop> for SupplyDropResponse {
    fn from(d: &SupplyDrop) -> Self {
        Self {
            id: d.id,
            season_id: d.season_id,
            poi_name: d.poi_name.clone(),
            status: d.status,
            loot_tier: d.loot_tier.clone(),
            scheduled_at: d.scheduled_at,
            claimed_by_username: d.claimed_by_username.clone(),
            claimed_at: d.claimed_at,
            expires_at: d.expires_at,
        }
    }
}

impl From<&PromoCode> for PromoCodeResponse {
    fn from(p: &PromoCode) -> Self {
        Self {
            code: p.code.clone(),
            discount_percent: p.discount_percent.as_u8(),
            max_uses: p.max_uses,
            times_used: p.times_used,
            active: p.active,
            created_at: p.created_at,
            expires_at: p.expires_at,
        }
    }
}
