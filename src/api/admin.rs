use axum::extract::rejection::QueryRejection;
use axum::extract::{FromRef, FromRequestParts, Path, Query, State};
use axum::http::request::Parts;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use chrono::Utc;

use crate::api::models::{
    CreatePromoRequest, ListDropsQuery, ListRegistrationsQuery, PromoCodeResponse,
    RegistrationAdminResponse, RotateResponse, SupplyDropResponse, TriggerDropRequest,
};
use crate::db::{PromoCodeRepo, RegistrationRepo, SeasonRepo, SupplyDropRepo};
use crate::domain::{DiscountPercent, DropStatus, PromoCode, RegStatus, SeasonId, SupplyDrop};
use crate::error::{AppError, AppResult};
use crate::services::{rotation, whitelist};
use crate::AppState;

// ---------------------------------------------------------------------------
// AdminAuth extractor
// ---------------------------------------------------------------------------

pub struct AdminAuth;

impl<S> FromRequestParts<S> for AdminAuth
where
    S: Send + Sync,
    AppState: axum::extract::FromRef<S>,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let app_state = AppState::from_ref(state);

        let auth_header = parts
            .headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .ok_or(AppError::Unauthorized)?;

        let token = auth_header
            .strip_prefix("Bearer ")
            .ok_or(AppError::Unauthorized)?;

        if token != &*app_state.admin_token {
            return Err(AppError::Unauthorized);
        }

        Ok(AdminAuth)
    }
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/admin/promo", post(create_promo).get(list_promos))
        .route("/api/admin/promo/{code}", delete(revoke_promo))
        .route("/api/admin/rotate", post(force_rotate))
        .route("/api/admin/registrations", get(list_registrations))
        .route("/api/admin/registrations/{id}", delete(revoke_registration))
        .route(
            "/api/admin/supply-drops",
            post(trigger_drop).get(list_drops),
        )
        .route("/api/admin/reload-lua", post(reload_lua))
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

async fn create_promo(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Json(req): Json<CreatePromoRequest>,
) -> AppResult<Json<PromoCodeResponse>> {
    let code = req.code.to_uppercase();
    if code.is_empty() || code.len() > 32 {
        return Err(AppError::BadRequest(
            "promo code must be 1-32 characters".into(),
        ));
    }
    if !code.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(AppError::BadRequest(
            "promo code must be alphanumeric or underscore".into(),
        ));
    }

    let discount = DiscountPercent::try_from(req.discount_percent)?;

    if state.repo.get_promo_code(&code).await?.is_some() {
        return Err(AppError::BadRequest("promo code already exists".into()));
    }

    let promo = PromoCode {
        code: code.clone(),
        discount_percent: discount,
        max_uses: req.max_uses,
        times_used: 0,
        active: true,
        created_at: Utc::now(),
        expires_at: req.expires_at,
    };

    state.repo.create_promo_code(&promo).await?;

    tracing::info!(
        promo_code = &code,
        discount_percent = discount.as_u8(),
        "admin_promo_created"
    );

    Ok(Json(PromoCodeResponse::from(&promo)))
}

async fn list_promos(
    _auth: AdminAuth,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<PromoCodeResponse>>> {
    let promos = state.repo.list_promo_codes().await?;
    let responses: Vec<PromoCodeResponse> = promos.iter().map(PromoCodeResponse::from).collect();
    Ok(Json(responses))
}

async fn revoke_promo(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(code): Path<String>,
) -> AppResult<impl IntoResponse> {
    let deactivated = state.repo.deactivate_promo_code(&code).await?;
    if !deactivated {
        return Err(AppError::NotFound);
    }

    tracing::info!(promo_code = &code, "admin_promo_revoked");

    Ok(StatusCode::NO_CONTENT)
}

async fn force_rotate(
    _auth: AdminAuth,
    State(state): State<AppState>,
) -> AppResult<Json<RotateResponse>> {
    tracing::warn!("admin_force_rotation");

    rotation::rotate_season(&state.repo, &state.config, &state.rcon_config, true).await?;

    Ok(Json(RotateResponse { status: "rotated" }))
}

async fn list_registrations(
    _auth: AdminAuth,
    State(state): State<AppState>,
    query: Result<Query<ListRegistrationsQuery>, QueryRejection>,
) -> AppResult<Json<Vec<RegistrationAdminResponse>>> {
    let query = query.map(|Query(q)| q).unwrap_or(ListRegistrationsQuery {
        season_id: None,
        status: None,
    });

    let registrations = if let Some(sid) = query.season_id {
        let season_id = SeasonId::new(sid)?;
        state.repo.list_registrations_for_season(season_id).await?
    } else {
        state.repo.list_all_registrations().await?
    };

    let mut responses: Vec<RegistrationAdminResponse> = registrations
        .iter()
        .map(RegistrationAdminResponse::from)
        .collect();

    if let Some(ref status_filter) = query.status {
        responses.retain(|r| r.status.as_db_str() == status_filter);
    }

    Ok(Json(responses))
}

async fn revoke_registration(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> AppResult<impl IntoResponse> {
    let reg = state
        .repo
        .get_registration_by_id(id)
        .await?
        .ok_or(AppError::NotFound)?;

    if reg.status == RegStatus::Expired {
        return Err(AppError::BadRequest("registration already expired".into()));
    }

    let name = reg.zomboid_username.as_str().to_string();
    let steam_id = reg.steam_id.as_ref().map(|s| s.as_str().to_string());
    let season_id = reg.season_id;

    state.repo.expire_registration(id).await?;

    whitelist::remove_player(&state.rcon_config, &state.config.zomboid, &name, steam_id.as_deref())
        .await;

    tracing::info!(
        registration_id = %id,
        zomboid_username = name,
        season_id = season_id.as_i64(),
        "admin_registration_revoked"
    );

    Ok(StatusCode::NO_CONTENT)
}

async fn reload_lua(
    _auth: AdminAuth,
    State(state): State<AppState>,
) -> AppResult<Json<serde_json::Value>> {
    let response = crate::services::rcon::reload_lua(&state.rcon_config).await?;
    tracing::info!(rcon_response = %response, "admin_lua_reload");
    Ok(Json(
        serde_json::json!({ "status": "reloaded", "rcon_response": response }),
    ))
}

async fn trigger_drop(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Json(req): Json<TriggerDropRequest>,
) -> AppResult<Json<SupplyDropResponse>> {
    let poi = state
        .config
        .supply_drops
        .pois
        .iter()
        .find(|p| p.name == req.poi_name)
        .ok_or_else(|| AppError::BadRequest(format!("unknown POI: {}", req.poi_name)))?
        .clone();

    let season = state.repo.get_active_season().await?;

    if state
        .repo
        .has_active_drop_at_poi(season.id, &poi.name)
        .await?
    {
        return Err(AppError::BadRequest(format!(
            "active drop already exists at {}",
            poi.name
        )));
    }

    let now = Utc::now();
    let sd_config = &state.config.supply_drops;
    let expire_hours = sd_config.expire_hours.min(sd_config.max_interval_hours);
    let expires_at = now + chrono::Duration::seconds((expire_hours * 3600.0) as i64);

    let loot_tier = req
        .loot_tier
        .unwrap_or_else(|| state.config.supply_drops.random_loot_tier());

    let drop = SupplyDrop {
        id: uuid::Uuid::new_v4(),
        season_id: season.id,
        poi_name: poi.name,
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

    state.repo.create_supply_drop(&drop).await?;

    tracing::info!(
        drop_id = %drop.id,
        poi = %drop.poi_name,
        "admin_supply_drop_triggered"
    );

    Ok(Json(SupplyDropResponse::from(&drop)))
}

async fn list_drops(
    _auth: AdminAuth,
    State(state): State<AppState>,
    query: Result<Query<ListDropsQuery>, QueryRejection>,
) -> AppResult<Json<Vec<SupplyDropResponse>>> {
    let query = query
        .map(|Query(q)| q)
        .unwrap_or(ListDropsQuery { season_id: None });

    let season_id = match query.season_id {
        Some(id) => SeasonId::new(id)?,
        None => state.repo.get_active_season().await?.id,
    };

    let drops = state.repo.get_drops_for_season(season_id).await?;
    let responses: Vec<SupplyDropResponse> = drops.iter().map(SupplyDropResponse::from).collect();
    Ok(Json(responses))
}
