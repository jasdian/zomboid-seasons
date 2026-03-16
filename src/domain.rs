use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{AppError, AppResult};

// ---------------------------------------------------------------------------
// Newtypes with validated constructors
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SeasonId(i64);

impl SeasonId {
    pub fn new(id: i64) -> AppResult<Self> {
        if id > 0 {
            Ok(Self(id))
        } else {
            Err(AppError::BadRequest(format!(
                "season id must be positive, got {id}"
            )))
        }
    }

    pub fn as_i64(self) -> i64 {
        self.0
    }

    pub fn next(self) -> Self {
        Self(self.0 + 1)
    }
}

impl std::fmt::Display for SeasonId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ---

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ZomboidUsername(String);

impl ZomboidUsername {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ZomboidUsername {
    type Error = AppError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        if s.is_empty() || s.len() > 50 {
            return Err(AppError::BadRequest(
                "username must be 1-50 characters".into(),
            ));
        }
        if !s.chars().all(|c| c.is_ascii_graphic() || c == ' ') {
            return Err(AppError::BadRequest(
                "username must contain only printable ASCII".into(),
            ));
        }
        Ok(Self(s))
    }
}

impl std::fmt::Display for ZomboidUsername {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

// ---

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ZomboidPassword(String);

impl ZomboidPassword {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ZomboidPassword {
    type Error = AppError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        if s.is_empty() || s.len() > 128 {
            return Err(AppError::BadRequest(
                "password must be 1-128 characters".into(),
            ));
        }
        Ok(Self(s))
    }
}

impl std::fmt::Display for ZomboidPassword {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

// ---

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SteamId(String);

impl SteamId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for SteamId {
    type Error = AppError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        let s = s.trim().to_string();
        // PZ's Kahlua2 Lua may emit Steam IDs in scientific notation
        // (e.g. "7.656119798478254E16") because getSteamID() returns a Java
        // long that Kahlua wraps as a double. We MUST NOT parse through f64
        // because float64 loses precision on 17-digit steam IDs (> 2^53).
        // Instead, do string manipulation to shift the decimal point.
        let s = if let Some(e_pos) = s.find(['E', 'e']) {
            let mantissa = &s[..e_pos];
            let exp: usize = s[e_pos + 1..]
                .trim_start_matches('+')
                .parse()
                .map_err(|_| {
                    AppError::BadRequest("steam ID: invalid scientific notation exponent".into())
                })?;
            // Split mantissa into integer and fractional parts
            let (int_part, frac_part) = if let Some(dot) = mantissa.find('.') {
                (&mantissa[..dot], &mantissa[dot + 1..])
            } else {
                (mantissa, "")
            };
            let mut digits = format!("{}{}", int_part, frac_part);
            let needed_len = int_part.len() + exp;
            if digits.len() < needed_len {
                // Pad with zeros
                digits.extend(std::iter::repeat_n('0', needed_len - digits.len()));
            } else if digits.len() > needed_len {
                digits.truncate(needed_len);
            }
            digits
        } else {
            s
        };
        if s.is_empty() || s.len() > 20 {
            return Err(AppError::BadRequest(
                "steam ID must be 1-20 characters".into(),
            ));
        }
        if !s.chars().all(|c| c.is_ascii_digit()) {
            return Err(AppError::BadRequest(
                "steam ID must contain only digits".into(),
            ));
        }
        Ok(Self(s))
    }
}

impl std::fmt::Display for SteamId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

// ---

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EthAddress(String);

impl EthAddress {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EthAddress {
    type Error = AppError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        let s = s.to_lowercase();
        if s.len() != 42 {
            return Err(AppError::BadRequest(
                "eth address must be 42 characters (0x + 40 hex)".into(),
            ));
        }
        if !s.starts_with("0x") {
            return Err(AppError::BadRequest(
                "eth address must start with 0x".into(),
            ));
        }
        if !s[2..].chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(AppError::BadRequest(
                "eth address must contain only hex digits after 0x".into(),
            ));
        }
        Ok(Self(s))
    }
}

impl std::fmt::Display for EthAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

// ---

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Wei(String);

impl Wei {
    pub fn from_decimal(s: String) -> AppResult<Self> {
        if s.is_empty() {
            return Err(AppError::BadRequest("wei amount cannot be empty".into()));
        }
        if !s.chars().all(|c| c.is_ascii_digit()) {
            return Err(AppError::BadRequest(
                "wei amount must be non-negative decimal digits".into(),
            ));
        }
        Ok(Self(s))
    }

    pub fn zero() -> Self {
        Self("0".to_string())
    }

    pub fn is_zero(&self) -> bool {
        self.0.chars().all(|c| c == '0')
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Wei {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

// ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DiscountPercent(u8);

impl DiscountPercent {
    pub fn as_u8(self) -> u8 {
        self.0
    }

    #[allow(dead_code)]
    pub fn is_full_discount(self) -> bool {
        self.0 == 100
    }
}

impl TryFrom<u8> for DiscountPercent {
    type Error = AppError;

    fn try_from(v: u8) -> Result<Self, Self::Error> {
        if v > 100 {
            Err(AppError::BadRequest(format!(
                "discount percent must be 0..=100, got {v}"
            )))
        } else {
            Ok(Self(v))
        }
    }
}

impl std::fmt::Display for DiscountPercent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}%", self.0)
    }
}

// ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ZombieKills(u64);

impl ZombieKills {
    pub fn new(n: u64) -> Self {
        Self(n)
    }

    pub fn as_u64(self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for ZombieKills {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ---------------------------------------------------------------------------
// Enums with exhaustive DB string conversion
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SeasonStatus {
    Pending,
    Active,
    Archived,
}

impl SeasonStatus {
    pub fn as_db_str(self) -> &'static str {
        match self {
            SeasonStatus::Pending => "pending",
            SeasonStatus::Active => "active",
            SeasonStatus::Archived => "archived",
        }
    }
}

impl TryFrom<&str> for SeasonStatus {
    type Error = AppError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "pending" => Ok(SeasonStatus::Pending),
            "active" => Ok(SeasonStatus::Active),
            "archived" => Ok(SeasonStatus::Archived),
            other => Err(AppError::BadRequest(format!(
                "unknown season status: {other}"
            ))),
        }
    }
}

impl std::fmt::Display for SeasonStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_db_str())
    }
}

// ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RegStatus {
    AwaitingPayment,
    Confirmed,
    Expired,
}

impl RegStatus {
    pub fn as_db_str(self) -> &'static str {
        match self {
            RegStatus::AwaitingPayment => "awaiting_payment",
            RegStatus::Confirmed => "confirmed",
            RegStatus::Expired => "expired",
        }
    }
}

impl TryFrom<&str> for RegStatus {
    type Error = AppError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "awaiting_payment" => Ok(RegStatus::AwaitingPayment),
            "confirmed" => Ok(RegStatus::Confirmed),
            "expired" => Ok(RegStatus::Expired),
            other => Err(AppError::BadRequest(format!(
                "unknown registration status: {other}"
            ))),
        }
    }
}

impl std::fmt::Display for RegStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_db_str())
    }
}

// ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DropStatus {
    Scheduled,
    Announced,
    Active,
    Claimed,
    Expired,
}

impl DropStatus {
    pub fn as_db_str(self) -> &'static str {
        match self {
            DropStatus::Scheduled => "scheduled",
            DropStatus::Announced => "announced",
            DropStatus::Active => "active",
            DropStatus::Claimed => "claimed",
            DropStatus::Expired => "expired",
        }
    }
}

impl TryFrom<&str> for DropStatus {
    type Error = AppError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "scheduled" => Ok(DropStatus::Scheduled),
            "announced" => Ok(DropStatus::Announced),
            "active" => Ok(DropStatus::Active),
            "claimed" => Ok(DropStatus::Claimed),
            "expired" => Ok(DropStatus::Expired),
            other => Err(AppError::BadRequest(format!(
                "unknown drop status: {other}"
            ))),
        }
    }
}

impl std::fmt::Display for DropStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_db_str())
    }
}

// ---------------------------------------------------------------------------
// Domain structs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct Season {
    pub id: SeasonId,
    pub status: SeasonStatus,
    pub started_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
    pub save_path: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Registration {
    pub id: Uuid,
    pub season_id: SeasonId,
    pub zomboid_username: ZomboidUsername,
    pub zomboid_password: ZomboidPassword,
    pub steam_id: SteamId,
    pub eth_address: EthAddress,
    pub promo_code: Option<String>,
    pub tx_hash: Option<String>,
    pub status: RegStatus,
    pub amount_wei: Wei,
    pub created_at: DateTime<Utc>,
    pub confirmed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PromoCode {
    pub code: String,
    pub discount_percent: DiscountPercent,
    pub max_uses: Option<u32>,
    pub times_used: u32,
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlayerStats {
    pub season_id: SeasonId,
    pub steam_id: SteamId,
    pub zomboid_username: ZomboidUsername,
    pub zombie_kills: ZombieKills,
    pub last_synced_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SupplyDrop {
    pub id: Uuid,
    pub season_id: SeasonId,
    pub poi_name: String,
    pub location_x: i32,
    pub location_y: i32,
    pub location_z: i32,
    pub status: DropStatus,
    pub loot_tier: String,
    pub scheduled_at: DateTime<Utc>,
    pub announced_at: Option<DateTime<Utc>>,
    pub activated_at: Option<DateTime<Utc>>,
    pub claimed_by_steam_id: Option<String>,
    pub claimed_by_username: Option<String>,
    pub claimed_at: Option<DateTime<Utc>>,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Pure domain functions
// ---------------------------------------------------------------------------

pub fn determine_initial_status(amount_is_zero: bool) -> RegStatus {
    if amount_is_zero {
        RegStatus::Confirmed
    } else {
        RegStatus::AwaitingPayment
    }
}

pub fn validate_promo_usable(promo: &PromoCode, now: DateTime<Utc>) -> Result<(), AppError> {
    if !promo.active {
        return Err(AppError::InvalidPromoCode);
    }
    if let Some(expires) = promo.expires_at {
        if now > expires {
            return Err(AppError::PromoExpired);
        }
    }
    if let Some(max) = promo.max_uses {
        if promo.times_used >= max {
            return Err(AppError::PromoExhausted);
        }
    }
    Ok(())
}

pub fn compute_effective_price(base_fee: u128, discount: DiscountPercent, offset: u64) -> u128 {
    let discount_amount = base_fee * u128::from(discount.as_u8()) / 100;
    let effective = base_fee - discount_amount;
    if effective == 0 {
        0
    } else {
        effective + u128::from(offset)
    }
}

// ---------------------------------------------------------------------------
// Reward System Scoring
// ---------------------------------------------------------------------------

/// Points from zombie kills. Exponential saturation, capped at 1000 kills.
pub fn p_kills(kills: u64) -> f64 {
    let capped = kills.min(1000) as f64;
    500.0 * (1.0 - (-capped / 200.0).exp())
}

/// Points from survival time. Quadratic growth, capped at 30 game days.
pub fn p_survival(game_days: f64) -> f64 {
    let capped = game_days.min(30.0);
    700.0 * (capped / 30.0).powi(2)
}

/// Points from online presence. 10 points per day with >= 30 min.
pub fn p_online(days_logged: u32) -> f64 {
    10.0 * days_logged as f64
}

/// Aggregate season score from the three components.
pub fn season_score(p_online_val: f64, best_p_kills: f64, best_p_survival: f64) -> i64 {
    (p_online_val + best_p_kills + best_p_survival).round() as i64
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_p_kills_zero() {
        assert!((p_kills(0) - 0.0).abs() < 1e-9);
    }

    #[test]
    fn test_p_kills_moderate() {
        // At 200 kills: 500 * (1 - e^(-1)) ~ 500 * 0.6321 ~ 316.06
        let val = p_kills(200);
        assert!((val - 316.06).abs() < 1.0, "got {val}");
    }

    #[test]
    fn test_p_kills_capped() {
        // Kills > 1000 should produce the same value as 1000
        let at_cap = p_kills(1000);
        let over_cap = p_kills(5000);
        assert!((at_cap - over_cap).abs() < 1e-9);
    }

    #[test]
    fn test_p_kills_max_value() {
        // At 1000: 500 * (1 - e^(-5)) ~ 500 * 0.9933 ~ 496.6
        let val = p_kills(1000);
        assert!(val > 490.0 && val < 500.0, "got {val}");
    }

    #[test]
    fn test_p_survival_zero() {
        assert!((p_survival(0.0) - 0.0).abs() < 1e-9);
    }

    #[test]
    fn test_p_survival_full() {
        // At 30 days: 700 * (30/30)^2 = 700
        assert!((p_survival(30.0) - 700.0).abs() < 1e-9);
    }

    #[test]
    fn test_p_survival_capped() {
        assert!((p_survival(60.0) - 700.0).abs() < 1e-9);
    }

    #[test]
    fn test_p_survival_half() {
        // At 15 days: 700 * (15/30)^2 = 700 * 0.25 = 175
        assert!((p_survival(15.0) - 175.0).abs() < 1e-9);
    }

    #[test]
    fn test_p_online() {
        assert!((p_online(0) - 0.0).abs() < 1e-9);
        assert!((p_online(10) - 100.0).abs() < 1e-9);
        assert!((p_online(90) - 900.0).abs() < 1e-9);
    }

    #[test]
    fn test_season_score() {
        assert_eq!(season_score(100.0, 316.0, 175.0), 591);
        assert_eq!(season_score(0.0, 0.0, 0.0), 0);
    }

    #[test]
    fn test_season_score_rounding() {
        // 100.4 rounds to 100
        assert_eq!(season_score(50.2, 25.1, 25.1), 100);
        // 100.5 rounds to 101
        assert_eq!(season_score(50.3, 25.1, 25.1), 101);
    }
}
