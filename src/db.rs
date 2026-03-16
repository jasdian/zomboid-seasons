use std::future::Future;

use chrono::{DateTime, NaiveDateTime, Utc};
use sqlx::sqlite::SqlitePool;
use sqlx::FromRow;
use uuid::Uuid;

use crate::crypto::Cipher;
use crate::domain::{
    DiscountPercent, DropStatus, EthAddress, PlayerStats, PromoCode, RegStatus, Registration,
    Season, SeasonId, SeasonStatus, SteamId, SupplyDrop, Wei, ZombieKills, ZomboidPassword,
    ZomboidUsername,
};
use crate::error::{AppError, AppResult};

// ---------------------------------------------------------------------------
// Row types (raw DB representation)
// ---------------------------------------------------------------------------

#[derive(Debug, FromRow)]
pub struct SeasonRow {
    pub id: i64,
    pub status: String,
    pub started_at: String,
    pub ends_at: String,
    pub save_path: Option<String>,
    pub created_at: String,
}

#[derive(Debug, FromRow)]
pub struct RegistrationRow {
    pub id: String,
    pub season_id: i64,
    pub zomboid_username: String,
    pub zomboid_password: String,
    pub steam_id: String,
    pub eth_address: String,
    pub promo_code: Option<String>,
    pub tx_hash: Option<String>,
    pub status: String,
    pub amount_wei: String,
    pub created_at: String,
    pub confirmed_at: Option<String>,
}

#[derive(Debug, FromRow)]
pub struct PromoCodeRow {
    pub code: String,
    pub discount_percent: i32,
    pub max_uses: Option<i32>,
    pub times_used: i32,
    pub active: i32,
    pub created_at: String,
    pub expires_at: Option<String>,
}

#[derive(Debug, FromRow)]
pub struct PlayerStatsRow {
    pub season_id: i64,
    pub steam_id: String,
    pub zomboid_username: String,
    pub zombie_kills: i64,
    pub last_synced_at: String,
}

#[derive(Debug, FromRow)]
pub struct SupplyDropRow {
    pub id: String,
    pub season_id: i64,
    pub poi_name: String,
    pub location_x: i32,
    pub location_y: i32,
    pub location_z: i32,
    pub status: String,
    pub loot_tier: String,
    pub scheduled_at: String,
    pub announced_at: Option<String>,
    pub activated_at: Option<String>,
    pub claimed_by_steam_id: Option<String>,
    pub claimed_by_username: Option<String>,
    pub claimed_at: Option<String>,
    pub expires_at: String,
    pub created_at: String,
}

// ---------------------------------------------------------------------------
// Datetime parsing helper
// ---------------------------------------------------------------------------

fn parse_datetime(s: &str) -> AppResult<DateTime<Utc>> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Ok(dt.with_timezone(&Utc));
    }
    NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
        .map(|ndt| ndt.and_utc())
        .map_err(|e| AppError::BadRequest(format!("invalid datetime '{s}': {e}")))
}

// ---------------------------------------------------------------------------
// TryFrom<Row> for domain types
// ---------------------------------------------------------------------------

impl TryFrom<SeasonRow> for Season {
    type Error = AppError;

    fn try_from(row: SeasonRow) -> Result<Self, Self::Error> {
        Ok(Season {
            id: SeasonId::new(row.id)?,
            status: SeasonStatus::try_from(row.status.as_str())?,
            started_at: parse_datetime(&row.started_at)?,
            ends_at: parse_datetime(&row.ends_at)?,
            save_path: row.save_path,
            created_at: parse_datetime(&row.created_at)?,
        })
    }
}

impl TryFrom<RegistrationRow> for Registration {
    type Error = AppError;

    fn try_from(row: RegistrationRow) -> Result<Self, Self::Error> {
        Ok(Registration {
            id: Uuid::parse_str(&row.id)
                .map_err(|e| AppError::BadRequest(format!("invalid uuid: {e}")))?,
            season_id: SeasonId::new(row.season_id)?,
            zomboid_username: ZomboidUsername::try_from(row.zomboid_username)?,
            zomboid_password: ZomboidPassword::try_from(row.zomboid_password)?,
            steam_id: SteamId::try_from(row.steam_id)?,
            eth_address: EthAddress::try_from(row.eth_address)?,
            promo_code: row.promo_code,
            tx_hash: row.tx_hash,
            status: RegStatus::try_from(row.status.as_str())?,
            amount_wei: Wei::from_decimal(row.amount_wei)?,
            created_at: parse_datetime(&row.created_at)?,
            confirmed_at: row.confirmed_at.map(|s| parse_datetime(&s)).transpose()?,
        })
    }
}

impl TryFrom<PromoCodeRow> for PromoCode {
    type Error = AppError;

    fn try_from(row: PromoCodeRow) -> Result<Self, Self::Error> {
        Ok(PromoCode {
            code: row.code,
            discount_percent: DiscountPercent::try_from(row.discount_percent as u8)?,
            max_uses: row.max_uses.map(|v| v as u32),
            times_used: row.times_used as u32,
            active: row.active != 0,
            created_at: parse_datetime(&row.created_at)?,
            expires_at: row.expires_at.map(|s| parse_datetime(&s)).transpose()?,
        })
    }
}

impl TryFrom<PlayerStatsRow> for PlayerStats {
    type Error = AppError;

    fn try_from(row: PlayerStatsRow) -> Result<Self, Self::Error> {
        Ok(PlayerStats {
            season_id: SeasonId::new(row.season_id)?,
            steam_id: SteamId::try_from(row.steam_id)?,
            zomboid_username: ZomboidUsername::try_from(row.zomboid_username)?,
            zombie_kills: ZombieKills::new(row.zombie_kills as u64),
            last_synced_at: parse_datetime(&row.last_synced_at)?,
        })
    }
}

impl TryFrom<SupplyDropRow> for SupplyDrop {
    type Error = AppError;

    fn try_from(row: SupplyDropRow) -> Result<Self, Self::Error> {
        Ok(SupplyDrop {
            id: Uuid::parse_str(&row.id)
                .map_err(|e| AppError::BadRequest(format!("invalid uuid: {e}")))?,
            season_id: SeasonId::new(row.season_id)?,
            poi_name: row.poi_name,
            location_x: row.location_x,
            location_y: row.location_y,
            location_z: row.location_z,
            status: DropStatus::try_from(row.status.as_str())?,
            loot_tier: row.loot_tier,
            scheduled_at: parse_datetime(&row.scheduled_at)?,
            announced_at: row.announced_at.map(|s| parse_datetime(&s)).transpose()?,
            activated_at: row.activated_at.map(|s| parse_datetime(&s)).transpose()?,
            claimed_by_steam_id: row.claimed_by_steam_id,
            claimed_by_username: row.claimed_by_username,
            claimed_at: row.claimed_at.map(|s| parse_datetime(&s)).transpose()?,
            expires_at: parse_datetime(&row.expires_at)?,
            created_at: parse_datetime(&row.created_at)?,
        })
    }
}

// ---------------------------------------------------------------------------
// Repo traits (RPITIT)
// ---------------------------------------------------------------------------

pub trait SeasonRepo {
    fn get_active_season(&self) -> impl Future<Output = AppResult<Season>> + Send;
    fn get_season_by_id(
        &self,
        id: SeasonId,
    ) -> impl Future<Output = AppResult<Option<Season>>> + Send;
    fn list_seasons(&self) -> impl Future<Output = AppResult<Vec<Season>>> + Send;
    fn create_season(
        &self,
        id: SeasonId,
        started_at: DateTime<Utc>,
        ends_at: DateTime<Utc>,
    ) -> impl Future<Output = AppResult<Season>> + Send;
    fn archive_season(&self, id: SeasonId) -> impl Future<Output = AppResult<()>> + Send;
}

pub trait RegistrationRepo {
    fn create_registration(&self, reg: &Registration)
        -> impl Future<Output = AppResult<()>> + Send;
    fn get_registration_by_id(
        &self,
        id: Uuid,
    ) -> impl Future<Output = AppResult<Option<Registration>>> + Send;
    fn find_pending_by_amount(
        &self,
        amount: &Wei,
    ) -> impl Future<Output = AppResult<Option<Registration>>> + Send;
    fn confirm_registration(
        &self,
        id: Uuid,
        tx_hash: &str,
    ) -> impl Future<Output = AppResult<()>> + Send;
    fn get_confirmed_for_season(
        &self,
        season_id: SeasonId,
    ) -> impl Future<Output = AppResult<Vec<Registration>>> + Send;
    fn expire_stale_registrations(
        &self,
        cutoff: DateTime<Utc>,
    ) -> impl Future<Output = AppResult<u64>> + Send;
    fn find_registration_by_name_and_season(
        &self,
        name: &ZomboidUsername,
        season_id: SeasonId,
    ) -> impl Future<Output = AppResult<Option<Registration>>> + Send;
    fn expire_registration(&self, id: Uuid) -> impl Future<Output = AppResult<()>> + Send;
}

pub trait RotationRepo {
    fn carry_forward_top_killers(
        &self,
        prev: SeasonId,
        new: SeasonId,
        count: u32,
    ) -> impl Future<Output = AppResult<u64>> + Send;
    fn get_confirmed_players(
        &self,
        season_id: SeasonId,
    ) -> impl Future<Output = AppResult<Vec<(String, String, String)>>> + Send;
    fn get_archived_seasons_for_purge(
        &self,
        keep: usize,
    ) -> impl Future<Output = AppResult<Vec<Season>>> + Send;
    fn update_season_save_path(
        &self,
        id: SeasonId,
        path: &str,
    ) -> impl Future<Output = AppResult<()>> + Send;
}

pub trait PromoCodeRepo {
    fn get_promo_code(
        &self,
        code: &str,
    ) -> impl Future<Output = AppResult<Option<PromoCode>>> + Send;
    fn increment_promo_usage(&self, code: &str) -> impl Future<Output = AppResult<()>> + Send;
    fn create_promo_code(&self, promo: &PromoCode) -> impl Future<Output = AppResult<()>> + Send;
    fn list_promo_codes(&self) -> impl Future<Output = AppResult<Vec<PromoCode>>> + Send;
    fn deactivate_promo_code(&self, code: &str) -> impl Future<Output = AppResult<bool>> + Send;
}

pub trait PlayerStatsRepo {
    fn upsert_player_stats(
        &self,
        season_id: SeasonId,
        steam_id: &SteamId,
        username: &ZomboidUsername,
        kills: ZombieKills,
        synced_at: DateTime<Utc>,
    ) -> impl Future<Output = AppResult<()>> + Send;

    fn get_leaderboard(
        &self,
        season_id: SeasonId,
        limit: u32,
    ) -> impl Future<Output = AppResult<Vec<PlayerStats>>> + Send;

    fn get_top_killers(
        &self,
        season_id: SeasonId,
        count: u32,
    ) -> impl Future<Output = AppResult<Vec<PlayerStats>>> + Send;
}

pub trait SupplyDropRepo {
    fn create_supply_drop(&self, drop: &SupplyDrop) -> impl Future<Output = AppResult<()>> + Send;
    fn get_drops_by_status(
        &self,
        status: DropStatus,
    ) -> impl Future<Output = AppResult<Vec<SupplyDrop>>> + Send;
    fn get_drops_for_season(
        &self,
        season_id: SeasonId,
    ) -> impl Future<Output = AppResult<Vec<SupplyDrop>>> + Send;
    fn update_drop_status(
        &self,
        id: Uuid,
        status: DropStatus,
        now: DateTime<Utc>,
    ) -> impl Future<Output = AppResult<()>> + Send;
    fn claim_drop(
        &self,
        id: Uuid,
        steam_id: &str,
        username: &str,
        now: DateTime<Utc>,
    ) -> impl Future<Output = AppResult<()>> + Send;
    fn get_last_drop_time(
        &self,
        season_id: SeasonId,
    ) -> impl Future<Output = AppResult<Option<DateTime<Utc>>>> + Send;
    fn has_active_drop_at_poi(
        &self,
        season_id: SeasonId,
        poi_name: &str,
    ) -> impl Future<Output = AppResult<bool>> + Send;
    fn get_stale_drops(
        &self,
        now: DateTime<Utc>,
    ) -> impl Future<Output = AppResult<Vec<SupplyDrop>>> + Send;
    fn force_expire_at_poi(
        &self,
        season_id: SeasonId,
        poi_name: &str,
    ) -> impl Future<Output = AppResult<()>> + Send;
    fn get_recent_drops_for_season(
        &self,
        season_id: SeasonId,
        limit: u32,
    ) -> impl Future<Output = AppResult<Vec<SupplyDrop>>> + Send;
    fn get_drop_by_id(
        &self,
        id: Uuid,
    ) -> impl Future<Output = AppResult<Option<SupplyDrop>>> + Send;
}

// ---------------------------------------------------------------------------
// Reward system row types
// ---------------------------------------------------------------------------

#[derive(Debug, FromRow)]
pub struct ScoredLeaderboardRow {
    pub steam_id: String,
    pub zomboid_username: String,
    pub best_p_kills: f64,
    pub best_p_survival: f64,
    pub best_kills: i64,
    pub best_survival_days: f64,
    pub online_days_count: i64,
    pub total_characters: i64,
    pub best_online_minutes: i64,
    pub best_hours_survived: f64,
    pub is_alive: i64,
}

#[derive(Debug, FromRow)]
pub struct AccountRow {
    pub steam_id: String,
    pub display_name: String,
    pub balance: i64,
}

#[derive(Debug, FromRow)]
pub struct AccountSeasonRow {
    pub season_id: i64,
    pub score: i64,
    pub characters: i64,
}

#[allow(clippy::too_many_arguments)]
pub trait RewardRepo {
    /// Create or update the permanent player account.
    fn upsert_account(
        &self,
        steam_id: &str,
        display_name: &str,
    ) -> impl Future<Output = AppResult<()>> + Send;

    /// Insert a game event. Returns true if inserted, false if idempotency conflict.
    fn insert_game_event(
        &self,
        id: &str,
        steam_id: &str,
        season_id: i64,
        character_id: Option<&str>,
        event_type: &str,
        payload: &str,
        idempotency_key: &str,
    ) -> impl Future<Output = AppResult<bool>> + Send;

    /// Create a new character snapshot (on character_created event).
    fn create_character_snapshot(
        &self,
        id: &str,
        steam_id: &str,
        season_id: i64,
        username: &str,
        created_at_world_age: f64,
    ) -> impl Future<Output = AppResult<()>> + Send;

    /// Increment zombie_kills on the given character snapshot.
    fn increment_character_kills(
        &self,
        character_id: &str,
    ) -> impl Future<Output = AppResult<()>> + Send;

    /// Get the active (alive) character for a player in a season.
    /// Returns (id, created_at_world_age, online_minutes, hours_survived).
    fn get_active_character(
        &self,
        steam_id: &str,
        season_id: i64,
    ) -> impl Future<Output = AppResult<Option<(String, f64, i64, f64)>>> + Send;

    /// Freeze a character on death: set died_at, game_days_survived, and score components.
    fn freeze_character(
        &self,
        character_id: &str,
        died_at: &str,
        game_days: f64,
        p_kills: f64,
        p_survival: f64,
    ) -> impl Future<Output = AppResult<()>> + Send;

    /// Add online minutes to the active (alive) character snapshot for a player.
    fn add_character_online_minutes(
        &self,
        steam_id: &str,
        season_id: i64,
        minutes: i64,
    ) -> impl Future<Output = AppResult<()>> + Send;

    /// Update hours_survived on the active (alive) character for a player.
    /// This is a SET (not accumulate) — PZ's getHoursSurvived() returns the total.
    fn update_character_hours_survived(
        &self,
        steam_id: &str,
        season_id: i64,
        hours: f64,
    ) -> impl Future<Output = AppResult<()>> + Send;

    /// Record online minutes for a player on a given date (accumulates).
    fn record_online_minutes(
        &self,
        steam_id: &str,
        season_id: i64,
        date: &str,
        minutes: i64,
    ) -> impl Future<Output = AppResult<()>> + Send;

    /// Get the scored leaderboard for a season.
    fn get_scored_leaderboard(
        &self,
        season_id: i64,
        limit: u32,
    ) -> impl Future<Output = AppResult<Vec<ScoredLeaderboardRow>>> + Send;

    /// Get account info.
    fn get_account(
        &self,
        steam_id: &str,
    ) -> impl Future<Output = AppResult<Option<AccountRow>>> + Send;

    /// Get season scores for an account (history).
    fn get_account_season_history(
        &self,
        steam_id: &str,
    ) -> impl Future<Output = AppResult<Vec<AccountSeasonRow>>> + Send;

    /// Check if a player earned their daily online bonus today.
    fn has_daily_online_bonus(
        &self,
        steam_id: &str,
        season_id: i64,
        date: &str,
    ) -> impl Future<Output = AppResult<bool>> + Send;

    /// Freeze all alive characters for a season (on season close).
    /// Sets game_days_survived, p_kills, p_survival based on current world_age.
    fn freeze_alive_characters(
        &self,
        season_id: i64,
        current_world_age: f64,
    ) -> impl Future<Output = AppResult<u64>> + Send;

    /// Compute and credit season scores for all players. Returns number credited.
    fn credit_season_scores(&self, season_id: i64) -> impl Future<Output = AppResult<u64>> + Send;

    /// Mark a season as finalized (prevents double-crediting).
    fn finalize_season(&self, season_id: i64) -> impl Future<Output = AppResult<()>> + Send;

    /// Check if a season is finalized.
    fn is_season_finalized(&self, season_id: i64) -> impl Future<Output = AppResult<bool>> + Send;
}

// ---------------------------------------------------------------------------
// SqliteRepo
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct SqliteRepo {
    pool: SqlitePool,
    cipher: Cipher,
}

impl SqliteRepo {
    pub fn new(pool: SqlitePool, cipher: Cipher) -> Self {
        Self { pool, cipher }
    }

    /// Borrow the underlying connection pool.
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Decrypt a RegistrationRow's password before converting to domain type.
    fn decrypt_row(&self, mut row: RegistrationRow) -> AppResult<Registration> {
        row.zomboid_password = self.cipher.decrypt_or_plaintext(&row.zomboid_password);
        Registration::try_from(row)
    }

    /// Decrypt rows best-effort: skip rows that fail to decrypt/convert
    /// so one corrupt row doesn't break the entire admin listing.
    fn decrypt_rows_best_effort(&self, rows: Vec<RegistrationRow>) -> Vec<Registration> {
        rows.into_iter()
            .filter_map(|row| {
                let id = row.id.clone();
                match self.decrypt_row(row) {
                    Ok(reg) => Some(reg),
                    Err(e) => {
                        tracing::warn!(registration_id = %id, error = %e, "skipped_corrupt_registration");
                        None
                    }
                }
            })
            .collect()
    }

    pub async fn run_migrations(&self) -> AppResult<()> {
        sqlx::migrate!("./migrations")
            .run(&self.pool)
            .await
            .map_err(|e| AppError::Config(format!("migration failed: {e}")))?;
        Ok(())
    }

    /// Encrypt any plaintext passwords still in the database.
    /// Idempotent: already-encrypted passwords are detected by successful decryption.
    pub async fn encrypt_plaintext_passwords(&self) -> AppResult<u64> {
        let rows: Vec<(String, String)> =
            sqlx::query_as("SELECT id, zomboid_password FROM registrations")
                .fetch_all(&self.pool)
                .await?;

        let mut migrated = 0u64;
        for (id, password) in rows {
            // If decryption succeeds, it's already encrypted — skip
            if self.cipher.decrypt(&password).is_ok() {
                continue;
            }
            // It's plaintext — encrypt it
            let encrypted = self.cipher.encrypt(&password)?;
            sqlx::query("UPDATE registrations SET zomboid_password = ? WHERE id = ?")
                .bind(&encrypted)
                .bind(&id)
                .execute(&self.pool)
                .await?;
            migrated += 1;
        }
        Ok(migrated)
    }

    pub async fn count_season_registrations(&self, season_id: SeasonId) -> AppResult<u64> {
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM registrations WHERE season_id = ? AND status != 'expired'",
        )
        .bind(season_id.as_i64())
        .fetch_one(&self.pool)
        .await?;
        Ok(row.0 as u64)
    }

    pub async fn get_player_count(&self, season_id: SeasonId) -> AppResult<u64> {
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(DISTINCT zomboid_username) FROM registrations \
             WHERE season_id = ? AND status = 'confirmed'",
        )
        .bind(season_id.as_i64())
        .fetch_one(&self.pool)
        .await?;
        Ok(row.0 as u64)
    }

    pub async fn list_registrations_for_season(
        &self,
        season_id: SeasonId,
    ) -> AppResult<Vec<Registration>> {
        let rows = sqlx::query_as::<_, RegistrationRow>(
            "SELECT id, season_id, zomboid_username, zomboid_password, steam_id, eth_address, promo_code, \
             tx_hash, status, amount_wei, created_at, confirmed_at \
             FROM registrations WHERE season_id = ? ORDER BY created_at DESC",
        )
        .bind(season_id.as_i64())
        .fetch_all(&self.pool)
        .await?;

        Ok(self.decrypt_rows_best_effort(rows))
    }

    pub async fn list_all_registrations(&self) -> AppResult<Vec<Registration>> {
        let rows = sqlx::query_as::<_, RegistrationRow>(
            "SELECT id, season_id, zomboid_username, zomboid_password, steam_id, eth_address, promo_code, \
             tx_hash, status, amount_wei, created_at, confirmed_at \
             FROM registrations ORDER BY created_at DESC",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(self.decrypt_rows_best_effort(rows))
    }
}

// ---------------------------------------------------------------------------
// SeasonRepo impl
// ---------------------------------------------------------------------------

impl SeasonRepo for SqliteRepo {
    async fn get_active_season(&self) -> AppResult<Season> {
        let row = sqlx::query_as::<_, SeasonRow>(
            "SELECT id, status, started_at, ends_at, save_path, created_at \
             FROM seasons WHERE status = 'active' LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or(AppError::NoActiveSeason)?;

        Season::try_from(row)
    }

    async fn get_season_by_id(&self, id: SeasonId) -> AppResult<Option<Season>> {
        sqlx::query_as::<_, SeasonRow>(
            "SELECT id, status, started_at, ends_at, save_path, created_at \
             FROM seasons WHERE id = ?",
        )
        .bind(id.as_i64())
        .fetch_optional(&self.pool)
        .await?
        .map(Season::try_from)
        .transpose()
    }

    async fn list_seasons(&self) -> AppResult<Vec<Season>> {
        sqlx::query_as::<_, SeasonRow>(
            "SELECT id, status, started_at, ends_at, save_path, created_at \
             FROM seasons ORDER BY id DESC",
        )
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(Season::try_from)
        .collect()
    }

    async fn create_season(
        &self,
        id: SeasonId,
        started_at: DateTime<Utc>,
        ends_at: DateTime<Utc>,
    ) -> AppResult<Season> {
        let started_str = started_at.to_rfc3339();
        let ends_str = ends_at.to_rfc3339();
        let created_str = Utc::now().to_rfc3339();

        sqlx::query(
            "INSERT INTO seasons (id, status, started_at, ends_at, created_at) \
             VALUES (?, 'active', ?, ?, ?)",
        )
        .bind(id.as_i64())
        .bind(&started_str)
        .bind(&ends_str)
        .bind(&created_str)
        .execute(&self.pool)
        .await?;

        self.get_season_by_id(id).await?.ok_or(AppError::NotFound)
    }

    async fn archive_season(&self, id: SeasonId) -> AppResult<()> {
        let now = Utc::now().to_rfc3339();
        sqlx::query("UPDATE seasons SET status = 'archived', ends_at = ? WHERE id = ?")
            .bind(&now)
            .bind(id.as_i64())
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// RegistrationRepo impl
// ---------------------------------------------------------------------------

impl RegistrationRepo for SqliteRepo {
    async fn create_registration(&self, reg: &Registration) -> AppResult<()> {
        let confirmed_str = reg.confirmed_at.map(|dt| dt.to_rfc3339());
        let encrypted_password = self.cipher.encrypt(reg.zomboid_password.as_str())?;
        sqlx::query(
            "INSERT INTO registrations \
             (id, season_id, zomboid_username, zomboid_password, steam_id, eth_address, promo_code, \
              tx_hash, status, amount_wei, created_at, confirmed_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(reg.id.to_string())
        .bind(reg.season_id.as_i64())
        .bind(reg.zomboid_username.as_str())
        .bind(&encrypted_password)
        .bind(reg.steam_id.as_str())
        .bind(reg.eth_address.as_str())
        .bind(&reg.promo_code)
        .bind(&reg.tx_hash)
        .bind(reg.status.as_db_str())
        .bind(reg.amount_wei.as_str())
        .bind(reg.created_at.to_rfc3339())
        .bind(&confirmed_str)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_registration_by_id(&self, id: Uuid) -> AppResult<Option<Registration>> {
        sqlx::query_as::<_, RegistrationRow>(
            "SELECT id, season_id, zomboid_username, zomboid_password, steam_id, eth_address, promo_code, \
             tx_hash, status, amount_wei, created_at, confirmed_at \
             FROM registrations WHERE id = ?",
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?
        .map(|row| self.decrypt_row(row))
        .transpose()
    }

    async fn find_pending_by_amount(&self, amount: &Wei) -> AppResult<Option<Registration>> {
        sqlx::query_as::<_, RegistrationRow>(
            "SELECT id, season_id, zomboid_username, zomboid_password, steam_id, eth_address, promo_code, \
             tx_hash, status, amount_wei, created_at, confirmed_at \
             FROM registrations \
             WHERE status = 'awaiting_payment' AND amount_wei = ? \
             ORDER BY created_at ASC LIMIT 1",
        )
        .bind(amount.as_str())
        .fetch_optional(&self.pool)
        .await?
        .map(|row| self.decrypt_row(row))
        .transpose()
    }

    async fn confirm_registration(&self, id: Uuid, tx_hash: &str) -> AppResult<()> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE registrations SET status = 'confirmed', tx_hash = ?, confirmed_at = ? \
             WHERE id = ?",
        )
        .bind(tx_hash)
        .bind(&now)
        .bind(id.to_string())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn get_confirmed_for_season(&self, season_id: SeasonId) -> AppResult<Vec<Registration>> {
        sqlx::query_as::<_, RegistrationRow>(
            "SELECT id, season_id, zomboid_username, zomboid_password, steam_id, eth_address, promo_code, \
             tx_hash, status, amount_wei, created_at, confirmed_at \
             FROM registrations \
             WHERE season_id = ? AND status = 'confirmed'",
        )
        .bind(season_id.as_i64())
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|row| self.decrypt_row(row))
        .collect()
    }

    async fn expire_stale_registrations(&self, cutoff: DateTime<Utc>) -> AppResult<u64> {
        let cutoff_str = cutoff.to_rfc3339();
        let result = sqlx::query(
            "UPDATE registrations SET status = 'expired' \
             WHERE status = 'awaiting_payment' AND created_at < ?",
        )
        .bind(&cutoff_str)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    async fn find_registration_by_name_and_season(
        &self,
        name: &ZomboidUsername,
        season_id: SeasonId,
    ) -> AppResult<Option<Registration>> {
        sqlx::query_as::<_, RegistrationRow>(
            "SELECT id, season_id, zomboid_username, zomboid_password, steam_id, eth_address, promo_code, \
             tx_hash, status, amount_wei, created_at, confirmed_at \
             FROM registrations \
             WHERE zomboid_username = ? AND season_id = ? AND status != 'expired' \
             LIMIT 1",
        )
        .bind(name.as_str())
        .bind(season_id.as_i64())
        .fetch_optional(&self.pool)
        .await?
        .map(|row| self.decrypt_row(row))
        .transpose()
    }

    async fn expire_registration(&self, id: Uuid) -> AppResult<()> {
        sqlx::query("UPDATE registrations SET status = 'expired' WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// RotationRepo impl
// ---------------------------------------------------------------------------

impl RotationRepo for SqliteRepo {
    async fn carry_forward_top_killers(
        &self,
        prev: SeasonId,
        new: SeasonId,
        count: u32,
    ) -> AppResult<u64> {
        let now = Utc::now().to_rfc3339();
        let result = sqlx::query(
            "INSERT INTO registrations \
             (id, season_id, zomboid_username, zomboid_password, steam_id, eth_address, promo_code, \
              tx_hash, status, amount_wei, created_at, confirmed_at) \
             SELECT hex(randomblob(16)), ?, r.zomboid_username, r.zomboid_password, r.steam_id, \
                    r.eth_address, NULL, NULL, 'confirmed', '0', ?, ? \
             FROM registrations r \
             INNER JOIN player_stats ps ON ps.season_id = ? AND ps.zomboid_username = r.zomboid_username \
             WHERE r.season_id = ? AND r.status = 'confirmed' \
             AND r.zomboid_username NOT IN ( \
                 SELECT zomboid_username FROM registrations WHERE season_id = ? \
             ) \
             ORDER BY ps.zombie_kills DESC \
             LIMIT ?",
        )
        .bind(new.as_i64())
        .bind(&now)
        .bind(&now)
        .bind(prev.as_i64())
        .bind(prev.as_i64())
        .bind(new.as_i64())
        .bind(count as i64)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    async fn get_confirmed_players(
        &self,
        season_id: SeasonId,
    ) -> AppResult<Vec<(String, String, String)>> {
        let rows: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT DISTINCT zomboid_username, zomboid_password, steam_id FROM registrations \
             WHERE season_id = ? AND status = 'confirmed'",
        )
        .bind(season_id.as_i64())
        .fetch_all(&self.pool)
        .await?;
        // Decrypt passwords (tuple index 1)
        Ok(rows
            .into_iter()
            .map(|(username, encrypted_pw, steam_id)| {
                let password = self.cipher.decrypt_or_plaintext(&encrypted_pw);
                (username, password, steam_id)
            })
            .collect())
    }

    async fn get_archived_seasons_for_purge(&self, keep: usize) -> AppResult<Vec<Season>> {
        let rows = sqlx::query_as::<_, SeasonRow>(
            "SELECT id, status, started_at, ends_at, save_path, created_at \
             FROM seasons WHERE status = 'archived' ORDER BY id DESC",
        )
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().skip(keep).map(Season::try_from).collect()
    }

    async fn update_season_save_path(&self, id: SeasonId, path: &str) -> AppResult<()> {
        sqlx::query("UPDATE seasons SET save_path = ? WHERE id = ?")
            .bind(path)
            .bind(id.as_i64())
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// PromoCodeRepo impl
// ---------------------------------------------------------------------------

impl PromoCodeRepo for SqliteRepo {
    async fn get_promo_code(&self, code: &str) -> AppResult<Option<PromoCode>> {
        sqlx::query_as::<_, PromoCodeRow>(
            "SELECT code, discount_percent, max_uses, \
             times_used, active, created_at, expires_at \
             FROM promo_codes WHERE code = ?",
        )
        .bind(code)
        .fetch_optional(&self.pool)
        .await?
        .map(PromoCode::try_from)
        .transpose()
    }

    async fn increment_promo_usage(&self, code: &str) -> AppResult<()> {
        sqlx::query("UPDATE promo_codes SET times_used = times_used + 1 WHERE code = ?")
            .bind(code)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn create_promo_code(&self, promo: &PromoCode) -> AppResult<()> {
        let expires_str = promo.expires_at.map(|dt| dt.to_rfc3339());

        sqlx::query(
            "INSERT INTO promo_codes \
             (code, discount_percent, max_uses, \
              times_used, active, created_at, expires_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&promo.code)
        .bind(promo.discount_percent.as_u8() as i32)
        .bind(promo.max_uses.map(|v| v as i32))
        .bind(promo.times_used as i32)
        .bind(i32::from(promo.active))
        .bind(promo.created_at.to_rfc3339())
        .bind(&expires_str)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn list_promo_codes(&self) -> AppResult<Vec<PromoCode>> {
        sqlx::query_as::<_, PromoCodeRow>(
            "SELECT code, discount_percent, max_uses, \
             times_used, active, created_at, expires_at \
             FROM promo_codes ORDER BY created_at DESC",
        )
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(PromoCode::try_from)
        .collect()
    }

    async fn deactivate_promo_code(&self, code: &str) -> AppResult<bool> {
        let result = sqlx::query("UPDATE promo_codes SET active = 0 WHERE code = ? AND active = 1")
            .bind(code)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}

// ---------------------------------------------------------------------------
// PlayerStatsRepo impl
// ---------------------------------------------------------------------------

impl PlayerStatsRepo for SqliteRepo {
    async fn upsert_player_stats(
        &self,
        season_id: SeasonId,
        steam_id: &SteamId,
        username: &ZomboidUsername,
        kills: ZombieKills,
        synced_at: DateTime<Utc>,
    ) -> AppResult<()> {
        let synced_str = synced_at.to_rfc3339();
        sqlx::query(
            "INSERT INTO player_stats (season_id, steam_id, zomboid_username, zombie_kills, last_synced_at) \
             VALUES (?, ?, ?, ?, ?) \
             ON CONFLICT(season_id, zomboid_username) DO UPDATE SET \
               steam_id = excluded.steam_id, \
               zombie_kills = excluded.zombie_kills, \
               last_synced_at = excluded.last_synced_at",
        )
        .bind(season_id.as_i64())
        .bind(steam_id.as_str())
        .bind(username.as_str())
        .bind(kills.as_u64() as i64)
        .bind(&synced_str)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn get_leaderboard(
        &self,
        season_id: SeasonId,
        limit: u32,
    ) -> AppResult<Vec<PlayerStats>> {
        sqlx::query_as::<_, PlayerStatsRow>(
            "SELECT season_id, steam_id, zomboid_username, zombie_kills, last_synced_at \
             FROM player_stats WHERE season_id = ? \
             ORDER BY zombie_kills DESC LIMIT ?",
        )
        .bind(season_id.as_i64())
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(PlayerStats::try_from)
        .collect()
    }

    async fn get_top_killers(
        &self,
        season_id: SeasonId,
        count: u32,
    ) -> AppResult<Vec<PlayerStats>> {
        self.get_leaderboard(season_id, count).await
    }
}

// ---------------------------------------------------------------------------
// SupplyDropRepo impl
// ---------------------------------------------------------------------------

const SD_COLS: &str = "id, season_id, poi_name, location_x, location_y, location_z, \
                        status, loot_tier, scheduled_at, announced_at, activated_at, \
                        claimed_by_steam_id, claimed_by_username, claimed_at, expires_at, created_at";

impl SupplyDropRepo for SqliteRepo {
    async fn create_supply_drop(&self, drop: &SupplyDrop) -> AppResult<()> {
        let announced_str = drop.announced_at.map(|dt| dt.to_rfc3339());
        let activated_str = drop.activated_at.map(|dt| dt.to_rfc3339());
        let claimed_str = drop.claimed_at.map(|dt| dt.to_rfc3339());

        sqlx::query(
            "INSERT INTO supply_drops \
             (id, season_id, poi_name, location_x, location_y, location_z, \
              status, loot_tier, scheduled_at, announced_at, activated_at, \
              claimed_by_steam_id, claimed_by_username, claimed_at, expires_at, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(drop.id.to_string())
        .bind(drop.season_id.as_i64())
        .bind(&drop.poi_name)
        .bind(drop.location_x)
        .bind(drop.location_y)
        .bind(drop.location_z)
        .bind(drop.status.as_db_str())
        .bind(&drop.loot_tier)
        .bind(drop.scheduled_at.to_rfc3339())
        .bind(&announced_str)
        .bind(&activated_str)
        .bind(&drop.claimed_by_steam_id)
        .bind(&drop.claimed_by_username)
        .bind(&claimed_str)
        .bind(drop.expires_at.to_rfc3339())
        .bind(drop.created_at.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_drops_by_status(&self, status: DropStatus) -> AppResult<Vec<SupplyDrop>> {
        let query = format!(
            "SELECT {SD_COLS} FROM supply_drops WHERE status = ? ORDER BY scheduled_at DESC"
        );
        sqlx::query_as::<_, SupplyDropRow>(&query)
            .bind(status.as_db_str())
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .map(SupplyDrop::try_from)
            .collect()
    }

    async fn get_drops_for_season(&self, season_id: SeasonId) -> AppResult<Vec<SupplyDrop>> {
        let query = format!(
            "SELECT {SD_COLS} FROM supply_drops WHERE season_id = ? ORDER BY scheduled_at DESC"
        );
        sqlx::query_as::<_, SupplyDropRow>(&query)
            .bind(season_id.as_i64())
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .map(SupplyDrop::try_from)
            .collect()
    }

    async fn update_drop_status(
        &self,
        id: Uuid,
        status: DropStatus,
        now: DateTime<Utc>,
    ) -> AppResult<()> {
        let now_str = now.to_rfc3339();
        let (extra_col, extra_val) = match status {
            DropStatus::Announced => (", announced_at = ?", Some(now_str.clone())),
            DropStatus::Active => (", activated_at = ?", Some(now_str.clone())),
            _ => ("", None),
        };

        let query = format!("UPDATE supply_drops SET status = ?{extra_col} WHERE id = ?");
        let mut q = sqlx::query(&query).bind(status.as_db_str());
        if let Some(val) = &extra_val {
            q = q.bind(val);
        }
        q.bind(id.to_string()).execute(&self.pool).await?;
        Ok(())
    }

    async fn claim_drop(
        &self,
        id: Uuid,
        steam_id: &str,
        username: &str,
        now: DateTime<Utc>,
    ) -> AppResult<()> {
        let now_str = now.to_rfc3339();
        sqlx::query(
            "UPDATE supply_drops SET status = 'claimed', \
             claimed_by_steam_id = ?, claimed_by_username = ?, claimed_at = ? \
             WHERE id = ? AND status != 'claimed'",
        )
        .bind(steam_id)
        .bind(username)
        .bind(&now_str)
        .bind(id.to_string())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn get_last_drop_time(&self, season_id: SeasonId) -> AppResult<Option<DateTime<Utc>>> {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT scheduled_at FROM supply_drops \
             WHERE season_id = ? ORDER BY scheduled_at DESC LIMIT 1",
        )
        .bind(season_id.as_i64())
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some((s,)) => Ok(Some(parse_datetime(&s)?)),
            None => Ok(None),
        }
    }

    async fn has_active_drop_at_poi(&self, season_id: SeasonId, poi_name: &str) -> AppResult<bool> {
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM supply_drops \
             WHERE season_id = ? AND poi_name = ? AND status IN ('scheduled','announced','active')",
        )
        .bind(season_id.as_i64())
        .bind(poi_name)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.0 > 0)
    }

    async fn get_stale_drops(&self, now: DateTime<Utc>) -> AppResult<Vec<SupplyDrop>> {
        let now_str = now.to_rfc3339();
        let query = format!(
            "SELECT {SD_COLS} FROM supply_drops \
             WHERE status IN ('scheduled','announced','active') AND expires_at <= ?"
        );
        sqlx::query_as::<_, SupplyDropRow>(&query)
            .bind(&now_str)
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .map(SupplyDrop::try_from)
            .collect()
    }

    async fn force_expire_at_poi(&self, season_id: SeasonId, poi_name: &str) -> AppResult<()> {
        sqlx::query(
            "UPDATE supply_drops SET status = 'expired' \
             WHERE season_id = ? AND poi_name = ? AND status IN ('scheduled','announced','active')",
        )
        .bind(season_id.as_i64())
        .bind(poi_name)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn get_recent_drops_for_season(
        &self,
        season_id: SeasonId,
        limit: u32,
    ) -> AppResult<Vec<SupplyDrop>> {
        let query = format!(
            "SELECT {SD_COLS} FROM supply_drops \
             WHERE season_id = ? ORDER BY scheduled_at DESC LIMIT ?"
        );
        sqlx::query_as::<_, SupplyDropRow>(&query)
            .bind(season_id.as_i64())
            .bind(limit as i64)
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .map(SupplyDrop::try_from)
            .collect()
    }

    async fn get_drop_by_id(&self, id: Uuid) -> AppResult<Option<SupplyDrop>> {
        let query = format!("SELECT {SD_COLS} FROM supply_drops WHERE id = ?");
        sqlx::query_as::<_, SupplyDropRow>(&query)
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await?
            .map(SupplyDrop::try_from)
            .transpose()
    }
}

// ---------------------------------------------------------------------------
// RewardRepo impl
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
impl RewardRepo for SqliteRepo {
    async fn upsert_account(&self, steam_id: &str, display_name: &str) -> AppResult<()> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO accounts (steam_id, display_name, balance, created_at) \
             VALUES (?, ?, 0, ?) \
             ON CONFLICT(steam_id) DO UPDATE SET display_name = excluded.display_name",
        )
        .bind(steam_id)
        .bind(display_name)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn insert_game_event(
        &self,
        id: &str,
        steam_id: &str,
        season_id: i64,
        character_id: Option<&str>,
        event_type: &str,
        payload: &str,
        idempotency_key: &str,
    ) -> AppResult<bool> {
        let now = Utc::now().to_rfc3339();
        let result = sqlx::query(
            "INSERT INTO game_events (id, steam_id, season_id, character_id, event_type, payload, idempotency_key, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?) \
             ON CONFLICT(idempotency_key) DO NOTHING",
        )
        .bind(id)
        .bind(steam_id)
        .bind(season_id)
        .bind(character_id)
        .bind(event_type)
        .bind(payload)
        .bind(idempotency_key)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    async fn create_character_snapshot(
        &self,
        id: &str,
        steam_id: &str,
        season_id: i64,
        username: &str,
        created_at_world_age: f64,
    ) -> AppResult<()> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO character_snapshots \
             (id, steam_id, season_id, zomboid_username, zombie_kills, game_days_survived, \
              created_at_world_age, online_minutes, p_kills, p_survival, created_at, updated_at) \
             VALUES (?, ?, ?, ?, 0, 0.0, ?, 0, 0.0, 0.0, ?, ?)",
        )
        .bind(id)
        .bind(steam_id)
        .bind(season_id)
        .bind(username)
        .bind(created_at_world_age)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn increment_character_kills(&self, character_id: &str) -> AppResult<()> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE character_snapshots SET zombie_kills = zombie_kills + 1, updated_at = ? WHERE id = ?",
        )
        .bind(&now)
        .bind(character_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn get_active_character(
        &self,
        steam_id: &str,
        season_id: i64,
    ) -> AppResult<Option<(String, f64, i64, f64)>> {
        let row: Option<(String, f64, i64, f64)> = sqlx::query_as(
            "SELECT id, created_at_world_age, online_minutes, hours_survived FROM character_snapshots \
             WHERE steam_id = ? AND season_id = ? AND died_at IS NULL \
             ORDER BY created_at DESC LIMIT 1",
        )
        .bind(steam_id)
        .bind(season_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    async fn freeze_character(
        &self,
        character_id: &str,
        died_at: &str,
        game_days: f64,
        p_kills_val: f64,
        p_survival_val: f64,
    ) -> AppResult<()> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE character_snapshots SET \
             died_at = ?, game_days_survived = ?, p_kills = ?, p_survival = ?, updated_at = ? \
             WHERE id = ?",
        )
        .bind(died_at)
        .bind(game_days)
        .bind(p_kills_val)
        .bind(p_survival_val)
        .bind(&now)
        .bind(character_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn add_character_online_minutes(
        &self,
        steam_id: &str,
        season_id: i64,
        minutes: i64,
    ) -> AppResult<()> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE character_snapshots \
             SET online_minutes = online_minutes + ?, updated_at = ? \
             WHERE steam_id = ? AND season_id = ? AND died_at IS NULL",
        )
        .bind(minutes)
        .bind(&now)
        .bind(steam_id)
        .bind(season_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn update_character_hours_survived(
        &self,
        steam_id: &str,
        season_id: i64,
        hours: f64,
    ) -> AppResult<()> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE character_snapshots \
             SET hours_survived = ?, updated_at = ? \
             WHERE steam_id = ? AND season_id = ? AND died_at IS NULL",
        )
        .bind(hours)
        .bind(&now)
        .bind(steam_id)
        .bind(season_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn record_online_minutes(
        &self,
        steam_id: &str,
        season_id: i64,
        date: &str,
        minutes: i64,
    ) -> AppResult<()> {
        sqlx::query(
            "INSERT INTO online_days (steam_id, season_id, date, minutes_online) \
             VALUES (?, ?, ?, ?) \
             ON CONFLICT(steam_id, season_id, date) DO UPDATE SET \
               minutes_online = minutes_online + excluded.minutes_online",
        )
        .bind(steam_id)
        .bind(season_id)
        .bind(date)
        .bind(minutes)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn get_scored_leaderboard(
        &self,
        season_id: i64,
        limit: u32,
    ) -> AppResult<Vec<ScoredLeaderboardRow>> {
        let rows = sqlx::query_as::<_, ScoredLeaderboardRow>(
            "WITH best_chars AS ( \
                SELECT steam_id, zomboid_username, p_kills, p_survival, zombie_kills, \
                       game_days_survived, online_minutes, hours_survived, died_at, \
                       ROW_NUMBER() OVER (PARTITION BY steam_id ORDER BY (p_kills + p_survival) DESC) as rn \
                FROM character_snapshots \
                WHERE season_id = ? \
            ), \
            online AS ( \
                SELECT steam_id, COUNT(*) as days_count \
                FROM online_days \
                WHERE season_id = ? AND minutes_online >= 30 \
                GROUP BY steam_id \
            ), \
            char_counts AS ( \
                SELECT steam_id, COUNT(*) as total \
                FROM character_snapshots \
                WHERE season_id = ? \
                GROUP BY steam_id \
            ) \
            SELECT bc.steam_id, bc.zomboid_username, \
                   bc.p_kills as best_p_kills, bc.p_survival as best_p_survival, \
                   bc.zombie_kills as best_kills, \
                   bc.game_days_survived as best_survival_days, \
                   COALESCE(o.days_count, 0) as online_days_count, \
                   COALESCE(cc.total, 0) as total_characters, \
                   bc.online_minutes as best_online_minutes, \
                   bc.hours_survived as best_hours_survived, \
                   CASE WHEN bc.died_at IS NULL THEN 1 ELSE 0 END as is_alive \
            FROM best_chars bc \
            LEFT JOIN online o ON o.steam_id = bc.steam_id \
            LEFT JOIN char_counts cc ON cc.steam_id = bc.steam_id \
            WHERE bc.rn = 1 \
            ORDER BY (10.0 * COALESCE(o.days_count, 0) + bc.p_kills + bc.p_survival) DESC \
            LIMIT ?",
        )
        .bind(season_id)
        .bind(season_id)
        .bind(season_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    async fn get_account(&self, steam_id: &str) -> AppResult<Option<AccountRow>> {
        let row = sqlx::query_as::<_, AccountRow>(
            "SELECT steam_id, display_name, balance FROM accounts WHERE steam_id = ?",
        )
        .bind(steam_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    async fn get_account_season_history(&self, steam_id: &str) -> AppResult<Vec<AccountSeasonRow>> {
        let rows = sqlx::query_as::<_, AccountSeasonRow>(
            "SELECT CAST(be.reference_id AS INTEGER) as season_id, be.amount as score, \
                    (SELECT COUNT(*) FROM character_snapshots cs \
                     WHERE cs.steam_id = be.steam_id \
                       AND cs.season_id = CAST(be.reference_id AS INTEGER)) as characters \
             FROM balance_entries be \
             WHERE be.steam_id = ? AND be.entry_type = 'season_score' \
             ORDER BY be.reference_id DESC",
        )
        .bind(steam_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    async fn has_daily_online_bonus(
        &self,
        steam_id: &str,
        season_id: i64,
        date: &str,
    ) -> AppResult<bool> {
        let row: Option<(i64,)> = sqlx::query_as(
            "SELECT minutes_online FROM online_days \
             WHERE steam_id = ? AND season_id = ? AND date = ?",
        )
        .bind(steam_id)
        .bind(season_id)
        .bind(date)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.is_some_and(|(mins,)| mins >= 30))
    }

    async fn freeze_alive_characters(
        &self,
        season_id: i64,
        current_world_age: f64,
    ) -> AppResult<u64> {
        // Get all alive characters for this season
        let rows: Vec<(String, i64, f64, f64)> = sqlx::query_as(
            "SELECT id, zombie_kills, created_at_world_age, hours_survived \
             FROM character_snapshots \
             WHERE season_id = ? AND died_at IS NULL",
        )
        .bind(season_id)
        .fetch_all(&self.pool)
        .await?;

        let now = Utc::now().to_rfc3339();
        let mut count = 0u64;
        for (id, kills, created_world_age, hours_survived) in &rows {
            // Prefer hours_survived from PZ native tracking, fall back to world-age delta
            let game_days = if *hours_survived > 0.0 {
                *hours_survived / 24.0
            } else {
                ((current_world_age - created_world_age) / 24.0).max(0.0)
            };
            let pk = crate::domain::p_kills(*kills as u64);
            let ps = crate::domain::p_survival(game_days);

            sqlx::query(
                "UPDATE character_snapshots \
                 SET died_at = ?, game_days_survived = ?, p_kills = ?, p_survival = ?, updated_at = ? \
                 WHERE id = ?",
            )
            .bind(&now)
            .bind(game_days)
            .bind(pk)
            .bind(ps)
            .bind(&now)
            .bind(id)
            .execute(&self.pool)
            .await?;
            count += 1;
        }

        Ok(count)
    }

    async fn credit_season_scores(&self, season_id: i64) -> AppResult<u64> {
        // Get scored leaderboard (all players, not limited)
        let rows = self.get_scored_leaderboard(season_id, 10000).await?;

        let now = Utc::now().to_rfc3339();
        let season_id_str = season_id.to_string();
        let mut count = 0u64;

        for row in &rows {
            let online_pts = crate::domain::p_online(row.online_days_count as u32);
            let score =
                crate::domain::season_score(online_pts, row.best_p_kills, row.best_p_survival);
            if score <= 0 {
                continue;
            }

            let entry_id = uuid::Uuid::new_v4().to_string();

            // Insert balance entry (idempotent via unique constraint if we add one,
            // but finalized flag prevents double-calls)
            sqlx::query(
                "INSERT INTO balance_entries (id, steam_id, amount, entry_type, reference_id, description, created_at) \
                 VALUES (?, ?, ?, 'season_score', ?, ?, ?)",
            )
            .bind(&entry_id)
            .bind(&row.steam_id)
            .bind(score)
            .bind(&season_id_str)
            .bind(format!("Season {} score", season_id))
            .bind(&now)
            .execute(&self.pool)
            .await?;

            // Update denormalized balance
            sqlx::query("UPDATE accounts SET balance = balance + ? WHERE steam_id = ?")
                .bind(score)
                .bind(&row.steam_id)
                .execute(&self.pool)
                .await?;

            count += 1;
        }

        Ok(count)
    }

    async fn finalize_season(&self, season_id: i64) -> AppResult<()> {
        sqlx::query("UPDATE seasons SET finalized = 1 WHERE id = ?")
            .bind(season_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn is_season_finalized(&self, season_id: i64) -> AppResult<bool> {
        let row: Option<(i64,)> = sqlx::query_as("SELECT finalized FROM seasons WHERE id = ?")
            .bind(season_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.is_some_and(|(f,)| f != 0))
    }
}
