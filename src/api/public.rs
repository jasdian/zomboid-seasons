use axum::extract::{Path, Query, State};
use axum::http::header;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::Utc;
use tokio::io::BufReader;
use tokio_util::io::ReaderStream;
use uuid::Uuid;

use crate::api::models::{
    AccountResponse, AccountSeasonResponse, LeaderboardEntryResponse, LeaderboardQuery,
    LeaderboardResponse, RegisterRequest, RegisterResponse, RegistrationStatusResponse,
    SeasonInfoResponse, SeasonSummary, SupplyDropResponse,
};
use crate::db::{PromoCodeRepo, RegistrationRepo, RewardRepo, SeasonRepo, SupplyDropRepo};
use crate::domain::{
    compute_effective_price, determine_initial_status, validate_promo_usable, DiscountPercent,
    EthAddress, Registration, SeasonId, SeasonStatus, SteamId, Wei, ZomboidPassword,
    ZomboidUsername,
};
use crate::error::{AppError, AppResult};
use crate::services::whitelist;
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/season", get(get_current_season))
        .route("/api/seasons", get(list_seasons))
        .route("/api/register", post(register))
        .route("/api/register/{id}", get(registration_status))
        .route("/api/saves/{season_id}", get(download_save))
        .route("/api/leaderboard", get(get_leaderboard))
        .route("/api/account/{steam_id}", get(get_account))
        .route("/api/supply-drops", get(get_supply_drops))
        .route("/api/supply-drops/pois", get(get_pois))
        .route("/api/gametime", get(get_gametime))
}

async fn get_current_season(State(state): State<AppState>) -> AppResult<Json<SeasonInfoResponse>> {
    let season = state.repo.get_active_season().await?;
    let player_count = state.repo.get_player_count(season.id).await?;

    let game_version = {
        let v = &state.config.zomboid.game_version;
        if v.is_empty() {
            None
        } else {
            Some(v.clone())
        }
    };

    Ok(Json(SeasonInfoResponse {
        id: season.id,
        status: season.status,
        started_at: season.started_at,
        ends_at: season.ends_at,
        player_count,
        base_fee_wei: state.base_fee_wei.clone(),
        game_version,
    }))
}

async fn list_seasons(State(state): State<AppState>) -> AppResult<Json<Vec<SeasonSummary>>> {
    let seasons = state.repo.list_seasons().await?;
    let summaries = seasons
        .iter()
        .map(|s| SeasonSummary {
            id: s.id,
            status: s.status,
            started_at: s.started_at,
            ends_at: s.ends_at,
            has_save_download: s.save_path.as_ref().map(|p| !p.is_empty()).unwrap_or(false),
        })
        .collect();
    Ok(Json(summaries))
}

async fn register(
    State(state): State<AppState>,
    Json(req): Json<RegisterRequest>,
) -> AppResult<Json<RegisterResponse>> {
    // 1. Validate username + password + steam_id
    let username = ZomboidUsername::try_from(req.zomboid_username)?;
    let password = ZomboidPassword::try_from(req.zomboid_password)?;
    let steam_id = SteamId::try_from(req.steam_id)?;

    // 2. Get active season
    let season = state.repo.get_active_season().await?;

    // 3. Check duplicate
    let existing = state
        .repo
        .find_registration_by_name_and_season(&username, season.id)
        .await?;

    if existing.is_some() {
        tracing::warn!(
            zomboid_username = username.as_str(),
            season_id = season.id.as_i64(),
            "duplicate_registration_attempt"
        );
        return Err(AppError::DuplicateRegistration);
    }

    // 4. Handle promo code
    let promo = if let Some(ref code) = req.promo_code {
        let p = state.repo.get_promo_code(code).await?.ok_or_else(|| {
            tracing::warn!(promo_code = code, "promo_not_found");
            AppError::InvalidPromoCode
        })?;

        if let Err(e) = validate_promo_usable(&p, Utc::now()) {
            match &e {
                AppError::PromoExpired => {
                    tracing::warn!(promo_code = code, "promo_expired");
                }
                AppError::PromoExhausted => {
                    tracing::warn!(promo_code = code, "promo_exhausted");
                }
                _ => {}
            }
            return Err(e);
        }

        state.repo.increment_promo_usage(code).await?;
        Some(p)
    } else {
        None
    };

    // 5. Count registrations for unique offset
    let offset = state.repo.count_season_registrations(season.id).await?;

    // 6. Compute effective price
    let base_fee: u128 = state
        .base_fee_wei
        .parse()
        .map_err(|e| AppError::Config(format!("invalid base_fee_wei: {e}")))?;

    let discount = promo
        .as_ref()
        .map(|p| p.discount_percent)
        .unwrap_or(DiscountPercent::try_from(0u8).expect("0 is valid"));
    let effective_price = compute_effective_price(base_fee, discount, offset);

    // 7. Determine initial status
    let amount_is_zero = effective_price == 0;
    let initial_status = determine_initial_status(amount_is_zero);
    let amount_wei = Wei::from_decimal(effective_price.to_string())?;

    // 8. Validate ETH address
    let eth_address = if amount_is_zero {
        match req.eth_address {
            Some(addr) if !addr.is_empty() => EthAddress::try_from(addr)?,
            _ => EthAddress::try_from("0x0000000000000000000000000000000000000000".to_string())?,
        }
    } else {
        let addr = req.eth_address.ok_or_else(|| {
            AppError::BadRequest("eth_address is required when payment is needed".into())
        })?;
        EthAddress::try_from(addr)?
    };

    // 9. Build and create registration
    let now = Utc::now();
    let reg = Registration {
        id: Uuid::new_v4(),
        season_id: season.id,
        zomboid_username: username.clone(),
        zomboid_password: password.clone(),
        steam_id,
        eth_address,
        promo_code: req.promo_code,
        tx_hash: None,
        status: initial_status,
        amount_wei: amount_wei.clone(),
        created_at: now,
        confirmed_at: if amount_is_zero { Some(now) } else { None },
    };
    state.repo.create_registration(&reg).await?;

    tracing::info!(
        registration_id = %reg.id,
        zomboid_username = reg.zomboid_username.as_str(),
        season_id = season.id.as_i64(),
        amount_wei = reg.amount_wei.as_str(),
        "registration_created"
    );

    // 10. If free (100% discount), whitelist immediately
    if amount_is_zero {
        whitelist::apply_whitelist_for_registration(
            &state.repo,
            &state.rcon_config,
            &state.config.zomboid,
            reg.zomboid_username.as_str(),
            reg.zomboid_password.as_str(),
            reg.steam_id.as_str(),
            season.id,
        )
        .await?;
    }

    // 11. Build response
    let deposit_address = if amount_is_zero {
        None
    } else {
        Some(state.deposit_address.clone())
    };
    let message = if amount_is_zero {
        "Registration confirmed! Save your password - you'll need it to log in to the server."
            .to_string()
    } else {
        format!(
            "Send exactly {} wei to {} to complete registration. Save your password!",
            reg.amount_wei, state.deposit_address
        )
    };

    Ok(Json(RegisterResponse {
        registration_id: reg.id,
        status: reg.status,
        amount_wei: reg.amount_wei,
        deposit_address,
        message,
    }))
}

async fn registration_status(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<RegistrationStatusResponse>> {
    let uuid = Uuid::parse_str(&id)
        .map_err(|e| AppError::BadRequest(format!("invalid registration id: {e}")))?;

    let reg = state
        .repo
        .get_registration_by_id(uuid)
        .await?
        .ok_or(AppError::NotFound)?;

    Ok(Json(RegistrationStatusResponse::from(&reg)))
}

async fn download_save(
    State(state): State<AppState>,
    Path(season_id): Path<i64>,
) -> AppResult<impl IntoResponse> {
    let season_id = SeasonId::new(season_id)?;
    let season = state
        .repo
        .get_season_by_id(season_id)
        .await?
        .ok_or(AppError::NotFound)?;

    if season.status != SeasonStatus::Archived {
        return Err(AppError::BadRequest(
            "save download only available for archived seasons".into(),
        ));
    }

    let save_path = season
        .save_path
        .as_ref()
        .filter(|p| !p.is_empty())
        .ok_or(AppError::NotFound)?;

    let path = std::path::Path::new(save_path);
    if !path.exists() {
        return Err(AppError::NotFound);
    }

    let file = tokio::fs::File::open(path).await?;
    let reader = BufReader::new(file);
    let stream = ReaderStream::new(reader);
    let body = axum::body::Body::from_stream(stream);

    let filename = format!("season_{}.tar.gz", season_id.as_i64());
    let headers = [
        (header::CONTENT_TYPE, "application/gzip".to_string()),
        (
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{filename}\""),
        ),
    ];

    Ok((headers, body))
}

async fn get_leaderboard(
    State(state): State<AppState>,
    Query(query): Query<LeaderboardQuery>,
) -> AppResult<Json<LeaderboardResponse>> {
    let limit = query
        .limit
        .unwrap_or(state.config.leaderboard.leaderboard_size)
        .min(100);

    let season_id = match query.season_id {
        Some(id) => SeasonId::new(id)?,
        None => state.repo.get_active_season().await?.id,
    };

    let rows = state
        .repo
        .get_scored_leaderboard(season_id.as_i64(), limit)
        .await?;

    let entries = rows
        .iter()
        .enumerate()
        .map(|(i, row)| {
            // For alive characters, compute live scores from current stats
            let (pk, ps, survival_days) = if row.is_alive != 0 {
                let days = if row.best_hours_survived > 0.0 {
                    row.best_hours_survived / 24.0
                } else {
                    row.best_survival_days
                };
                (
                    crate::domain::p_kills(row.best_kills as u64),
                    crate::domain::p_survival(days),
                    days,
                )
            } else {
                (
                    row.best_p_kills,
                    row.best_p_survival,
                    row.best_survival_days,
                )
            };
            let p_online_val = crate::domain::p_online(row.online_days_count as u32);
            let score = crate::domain::season_score(p_online_val, pk, ps);
            LeaderboardEntryResponse {
                rank: (i as u32) + 1,
                zomboid_username: row.zomboid_username.clone(),
                season_score: score,
                p_online: p_online_val,
                p_kills: pk,
                p_survival: ps,
                best_kills: row.best_kills as u64,
                best_survival_days: survival_days,
                days_logged: row.online_days_count as u32,
                characters: row.total_characters as u32,
                zombie_kills: row.best_kills as u64,
            }
        })
        .collect();

    Ok(Json(LeaderboardResponse {
        season_id,
        entries,
        fetched_at: Utc::now(),
    }))
}

async fn get_account(
    State(state): State<AppState>,
    Path(steam_id): Path<String>,
) -> AppResult<Json<AccountResponse>> {
    let account = state
        .repo
        .get_account(&steam_id)
        .await?
        .ok_or(AppError::NotFound)?;

    let history = state.repo.get_account_season_history(&steam_id).await?;

    // Check daily online bonus for active season
    let daily_online_bonus = match state.repo.get_active_season().await {
        Ok(active) => {
            let today = Utc::now().format("%Y-%m-%d").to_string();
            state
                .repo
                .has_daily_online_bonus(&steam_id, active.id.as_i64(), &today)
                .await
                .unwrap_or(false)
        }
        Err(_) => false,
    };

    let seasons = history
        .into_iter()
        .map(|row| AccountSeasonResponse {
            season_id: row.season_id,
            score: row.score,
            characters: row.characters as u32,
        })
        .collect();

    Ok(Json(AccountResponse {
        steam_id: account.steam_id,
        balance: account.balance,
        daily_online_bonus,
        seasons,
    }))
}

async fn get_pois(State(state): State<AppState>) -> Json<Vec<String>> {
    Json(
        state
            .config
            .supply_drops
            .pois
            .iter()
            .map(|p| p.name.clone())
            .collect(),
    )
}

async fn get_supply_drops(
    State(state): State<AppState>,
) -> AppResult<Json<Vec<SupplyDropResponse>>> {
    let season = state.repo.get_active_season().await?;
    let drops = state.repo.get_recent_drops_for_season(season.id, 5).await?;
    let responses: Vec<SupplyDropResponse> = drops.iter().map(SupplyDropResponse::from).collect();
    Ok(Json(responses))
}

async fn get_gametime(State(state): State<AppState>) -> AppResult<Json<serde_json::Value>> {
    let guard = state.gametime_cache.read().await;
    match &*guard {
        Some(gt) => Ok(Json(serde_json::json!({
            "month": gt.month,
            "day": gt.day,
            "hour": gt.hour,
            "minute": gt.minute,
            "world_age_hours": gt.world_age_hours,
        }))),
        None => Ok(Json(serde_json::json!({
            "month": null,
            "day": null,
            "hour": null,
            "minute": null,
            "world_age_hours": null,
        }))),
    }
}
