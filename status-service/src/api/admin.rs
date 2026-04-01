use axum::extract::{FromRef, FromRequestParts, Path, State};
use axum::http::request::Parts;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{patch, post};
use axum::{Json, Router};
use chrono::Utc;
use uuid::Uuid;

use crate::api::models::{
    AddIncidentUpdateRequest, CreateIncidentRequest, CreateMaintenanceRequest, IncidentResponse,
    IncidentUpdateResponse, MaintenanceResponse, PatchIncidentRequest, PatchMaintenanceRequest,
    SetComponentOverrideRequest, SetPostmortemRequest,
};
use crate::api::public::maintenance_to_response;
use crate::db::StatusRepo;
use crate::domain::{
    ComponentStatus, IncidentSeverity, IncidentStatus, MaintenanceStatus, ScheduledMaintenance,
    StatusIncident, StatusIncidentUpdate, UpdateStatus,
};
use crate::error::{AppError, AppResult};
use crate::notifications::NotificationEvent;
use crate::AppState;

// ---------------------------------------------------------------------------
// Auth extractor
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
        .route(
            "/api/admin/incidents",
            post(create_incident).get(list_incidents),
        )
        .route(
            "/api/admin/incidents/{id}",
            patch(patch_incident).delete(delete_incident),
        )
        .route(
            "/api/admin/incidents/{id}/updates",
            post(add_incident_update),
        )
        .route("/api/admin/incidents/{id}/postmortem", post(set_postmortem))
        .route(
            "/api/admin/components/{id}/override",
            post(set_component_override),
        )
        .route(
            "/api/admin/maintenance",
            post(create_maintenance).get(list_all_maintenance),
        )
        .route("/api/admin/maintenance/{id}", patch(patch_maintenance))
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

async fn create_incident(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Json(req): Json<CreateIncidentRequest>,
) -> AppResult<Json<IncidentResponse>> {
    let severity = IncidentSeverity::try_from(req.severity.as_str())?;
    let now = Utc::now();

    let incident = StatusIncident {
        id: Uuid::new_v4(),
        title: req.title,
        status: IncidentStatus::Investigating,
        severity,
        auto_detected: false,
        created_at: now,
        resolved_at: None,
        postmortem_body: None,
        postmortem_published_at: None,
    };

    state.repo.create_incident(&incident).await?;

    // Create initial update
    let update = StatusIncidentUpdate {
        id: Uuid::new_v4(),
        incident_id: incident.id,
        status: UpdateStatus::Investigating,
        message: req.message.clone(),
        created_at: now,
    };
    state.repo.add_incident_update(&update).await?;

    // Link components
    state
        .repo
        .link_incident_components(incident.id, &req.component_ids)
        .await?;

    tracing::info!(
        incident_id = %incident.id,
        title = %incident.title,
        severity = %incident.severity,
        "admin_incident_created"
    );

    state
        .notification_sender
        .send(NotificationEvent::IncidentCreated {
            incident_id: incident.id,
        });

    Ok(Json(IncidentResponse {
        id: incident.id,
        title: incident.title,
        status: incident.status.as_db_str(),
        severity: incident.severity.as_db_str(),
        component_ids: req.component_ids,
        updates: vec![IncidentUpdateResponse {
            id: update.id,
            status: update.status.as_db_str(),
            message: req.message,
            created_at: now,
        }],
        created_at: incident.created_at,
        resolved_at: incident.resolved_at,
        postmortem_body: None,
        postmortem_published_at: None,
    }))
}

async fn list_incidents(
    _auth: AdminAuth,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<IncidentResponse>>> {
    let incidents = state.repo.list_all_incidents().await?;
    let ids: Vec<Uuid> = incidents.iter().map(|i| i.id).collect();
    let all_updates = state.repo.get_updates_for_incidents(&ids).await?;
    let all_comp_ids = state.repo.get_component_ids_for_incidents(&ids).await?;

    let responses: Vec<IncidentResponse> = incidents
        .into_iter()
        .map(|inc| {
            let updates = all_updates.get(&inc.id).cloned().unwrap_or_default();
            let comp_ids = all_comp_ids.get(&inc.id).cloned().unwrap_or_default();
            IncidentResponse {
                id: inc.id,
                title: inc.title,
                status: inc.status.as_db_str(),
                severity: inc.severity.as_db_str(),
                component_ids: comp_ids,
                updates: updates
                    .into_iter()
                    .map(|u| IncidentUpdateResponse {
                        id: u.id,
                        status: u.status.as_db_str(),
                        message: u.message,
                        created_at: u.created_at,
                    })
                    .collect(),
                created_at: inc.created_at,
                resolved_at: inc.resolved_at,
                postmortem_body: inc.postmortem_body,
                postmortem_published_at: inc.postmortem_published_at,
            }
        })
        .collect();
    Ok(Json(responses))
}

async fn patch_incident(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<PatchIncidentRequest>,
) -> AppResult<impl IntoResponse> {
    let uuid = Uuid::parse_str(&id)
        .map_err(|e| AppError::BadRequest(format!("invalid incident id: {e}")))?;

    state
        .repo
        .get_incident_by_id(uuid)
        .await?
        .ok_or(AppError::NotFound)?;

    let status = IncidentStatus::try_from(req.status.as_str())?;
    let resolved_at = if status == IncidentStatus::Resolved {
        Some(Utc::now())
    } else {
        None
    };

    state
        .repo
        .update_incident_status(uuid, status, resolved_at)
        .await?;

    tracing::info!(
        incident_id = %uuid,
        new_status = %status,
        "admin_incident_patched"
    );

    Ok(StatusCode::NO_CONTENT)
}

async fn delete_incident(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    let uuid = Uuid::parse_str(&id)
        .map_err(|e| AppError::BadRequest(format!("invalid incident id: {e}")))?;

    state
        .repo
        .get_incident_by_id(uuid)
        .await?
        .ok_or(AppError::NotFound)?;

    state.repo.delete_incident(uuid).await?;

    tracing::info!(incident_id = %uuid, "admin_incident_deleted");

    Ok(StatusCode::NO_CONTENT)
}

async fn set_postmortem(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<SetPostmortemRequest>,
) -> AppResult<impl IntoResponse> {
    let uuid = Uuid::parse_str(&id)
        .map_err(|e| AppError::BadRequest(format!("invalid incident id: {e}")))?;
    let incident = state
        .repo
        .get_incident_by_id(uuid)
        .await?
        .ok_or(AppError::NotFound)?;
    if incident.status != IncidentStatus::Resolved {
        return Err(AppError::BadRequest(
            "postmortem can only be added to resolved incidents".into(),
        ));
    }
    state.repo.set_postmortem(uuid, &req.body).await?;
    tracing::info!(incident_id = %uuid, "admin_postmortem_published");
    state
        .notification_sender
        .send(NotificationEvent::PostmortemPublished { incident_id: uuid });
    Ok(StatusCode::NO_CONTENT)
}

async fn add_incident_update(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<AddIncidentUpdateRequest>,
) -> AppResult<Json<IncidentUpdateResponse>> {
    let incident_id = Uuid::parse_str(&id)
        .map_err(|e| AppError::BadRequest(format!("invalid incident id: {e}")))?;

    state
        .repo
        .get_incident_by_id(incident_id)
        .await?
        .ok_or(AppError::NotFound)?;

    let update_status = UpdateStatus::try_from(req.status.as_str())?;
    let now = Utc::now();

    let update = StatusIncidentUpdate {
        id: Uuid::new_v4(),
        incident_id,
        status: update_status,
        message: req.message.clone(),
        created_at: now,
    };
    state.repo.add_incident_update(&update).await?;

    // Sync the parent incident status with the update status
    let incident_status = match update_status {
        UpdateStatus::Investigating => Some(IncidentStatus::Investigating),
        UpdateStatus::Identified => Some(IncidentStatus::Identified),
        UpdateStatus::Monitoring => Some(IncidentStatus::Monitoring),
        UpdateStatus::Resolved => Some(IncidentStatus::Resolved),
        UpdateStatus::Update => None, // informational update, no status change
    };
    if let Some(status) = incident_status {
        let resolved_at = if status == IncidentStatus::Resolved {
            Some(now)
        } else {
            None
        };
        state
            .repo
            .update_incident_status(incident_id, status, resolved_at)
            .await?;
    }

    tracing::info!(
        incident_id = %incident_id,
        update_id = %update.id,
        status = %update_status,
        "admin_incident_update_added"
    );

    if update_status == UpdateStatus::Resolved {
        state
            .notification_sender
            .send(NotificationEvent::IncidentResolved { incident_id });
    } else {
        state
            .notification_sender
            .send(NotificationEvent::IncidentUpdated {
                incident_id,
                message: req.message.clone(),
            });
    }

    Ok(Json(IncidentUpdateResponse {
        id: update.id,
        status: update_status.as_db_str(),
        message: req.message,
        created_at: now,
    }))
}

async fn set_component_override(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<SetComponentOverrideRequest>,
) -> AppResult<impl IntoResponse> {
    let status = req
        .status
        .map(|s| ComponentStatus::try_from(s.as_str()))
        .transpose()?;

    state.repo.set_component_override(&id, status).await?;

    tracing::info!(
        component_id = %id,
        override_status = ?status,
        "admin_component_override_set"
    );

    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------
// Maintenance handlers
// ---------------------------------------------------------------------------

async fn create_maintenance(
    _auth: AdminAuth,
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
        "admin_maintenance_created"
    );

    state
        .notification_sender
        .send(NotificationEvent::MaintenanceScheduled {
            maintenance_id: m.id,
        });

    Ok(Json(maintenance_to_response(m)))
}

async fn patch_maintenance(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<PatchMaintenanceRequest>,
) -> AppResult<impl IntoResponse> {
    let uuid = Uuid::parse_str(&id)
        .map_err(|e| AppError::BadRequest(format!("invalid maintenance id: {e}")))?;

    let existing = state
        .repo
        .get_maintenance_by_id(uuid)
        .await?
        .ok_or(AppError::NotFound)?;

    let new_status = MaintenanceStatus::try_from(req.status.as_str())?;

    // Validate transition
    match (existing.status, new_status) {
        (MaintenanceStatus::Scheduled, MaintenanceStatus::InProgress) => {}
        (MaintenanceStatus::InProgress, MaintenanceStatus::Completed) => {}
        (from, to) => {
            return Err(AppError::BadRequest(format!(
                "invalid maintenance status transition: {from} -> {to}"
            )));
        }
    }

    let now = Utc::now();
    let actual_start = if new_status == MaintenanceStatus::InProgress {
        Some(now)
    } else {
        None
    };
    let actual_end = if new_status == MaintenanceStatus::Completed {
        Some(now)
    } else {
        None
    };

    state
        .repo
        .update_maintenance_status(uuid, new_status, actual_start, actual_end)
        .await?;

    // When manually starting, if scheduled_end is already past, push it forward
    // so the maintenance watcher doesn't immediately auto-complete it.
    if new_status == MaintenanceStatus::InProgress && existing.scheduled_end <= now {
        let duration = existing
            .scheduled_end
            .signed_duration_since(existing.scheduled_start);
        let min_duration = chrono::Duration::minutes(30);
        let effective_duration = if duration > min_duration {
            duration
        } else {
            min_duration
        };
        let new_end = now + effective_duration;
        state
            .repo
            .update_maintenance_scheduled_end(uuid, new_end)
            .await?;
        tracing::info!(
            maintenance_id = %uuid,
            new_scheduled_end = %new_end,
            "adjusted scheduled_end for manually started maintenance"
        );
    }

    tracing::info!(
        maintenance_id = %uuid,
        new_status = %new_status,
        "admin_maintenance_patched"
    );

    match new_status {
        MaintenanceStatus::InProgress => {
            state
                .notification_sender
                .send(NotificationEvent::MaintenanceStarted {
                    maintenance_id: uuid,
                });
        }
        MaintenanceStatus::Completed => {
            state
                .notification_sender
                .send(NotificationEvent::MaintenanceCompleted {
                    maintenance_id: uuid,
                });
        }
        _ => {}
    }

    Ok(StatusCode::NO_CONTENT)
}

async fn list_all_maintenance(
    _auth: AdminAuth,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<MaintenanceResponse>>> {
    let all = state.repo.list_all_maintenances().await?;
    let responses: Vec<MaintenanceResponse> =
        all.into_iter().map(maintenance_to_response).collect();
    Ok(Json(responses))
}
