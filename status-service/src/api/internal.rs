use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use chrono::Utc;
use uuid::Uuid;

use crate::api::models::{CreateMaintenanceRequest, MaintenanceResponse};
use crate::api::public::maintenance_to_response;
use crate::db::StatusRepo;
use crate::domain::{MaintenanceStatus, ScheduledMaintenance};
use crate::error::AppResult;
use crate::AppState;

/// Internal routes — no authentication. Only reachable within the Docker network
/// (not exposed via reverse proxy). Used by the main backend to announce maintenance.
pub fn router() -> Router<AppState> {
    Router::new().route("/internal/maintenance", post(create_internal_maintenance))
}

async fn create_internal_maintenance(
    State(state): State<AppState>,
    Json(req): Json<CreateMaintenanceRequest>,
) -> AppResult<Json<MaintenanceResponse>> {
    let now = Utc::now();
    let m = ScheduledMaintenance {
        id: Uuid::new_v4(),
        title: req.title,
        description: req.description,
        status: MaintenanceStatus::Scheduled,
        component_ids: req.component_ids,
        scheduled_start: req.scheduled_start,
        scheduled_end: req.scheduled_end,
        actual_start: None,
        actual_end: None,
        auto_suppress_incidents: req.auto_suppress_incidents,
        created_at: now,
    };

    state.repo.create_maintenance(&m).await?;

    tracing::info!(
        maintenance_id = %m.id,
        title = %m.title,
        "internal_maintenance_created"
    );

    Ok(Json(maintenance_to_response(m)))
}
