use std::collections::BTreeMap;

use axum::extract::{Path, Query, State};
use axum::http::header;
use axum::response::Response;
use axum::routing::get;
use axum::{Json, Router};
use chrono::{Datelike, Months, Utc};
use uuid::Uuid;

use crate::api::models::{
    CalendarUptimeResponse, ComponentGroupResponse, ComponentStatusResponse,
    ComponentUptimeResponse, DayUptimeResponse, HistoryQuery, HistoryResponse, IncidentResponse,
    IncidentUpdateResponse, MaintenanceListResponse, MaintenanceResponse,
    MonthHistoryResponse, MonthUptimeResponse,
    StatusPageResponse, UptimeCalendarQuery, UptimeResponse,
};
use crate::db::StatusRepo;
use crate::domain::{
    derive_component_status, ComponentStatus, ScheduledMaintenance, StatusIncident,
    StatusIncidentUpdate,
};
use crate::error::{AppError, AppResult};
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/status", get(get_status))
        .route("/api/status/uptime", get(get_uptime))
        .route("/api/status/uptime/calendar", get(get_uptime_calendar))
        .route("/api/status/history", get(get_history))
        .route("/api/status/maintenance", get(get_maintenance))
        .route("/api/status/feed.atom", get(get_atom_feed))
        .route("/api/status/incidents/{id}", get(get_incident_detail))
}

async fn get_status(State(state): State<AppState>) -> AppResult<Json<StatusPageResponse>> {
    let components = state.repo.list_components().await?;
    let groups = state.repo.list_component_groups().await?;
    let active_incidents_with_comps = state.repo.list_active_incidents_with_components().await?;
    let past_incidents = state.repo.list_recent_resolved_incidents(15).await?;

    // Build component responses with derived status, tracking overall worst
    let mut overall = ComponentStatus::Operational;
    let mut comp_responses = Vec::with_capacity(components.len());
    for comp in &components {
        let comp_active: Vec<StatusIncident> = active_incidents_with_comps
            .iter()
            .filter(|(_, cids)| cids.contains(&comp.id))
            .map(|(inc, _)| inc.clone())
            .collect();
        let derived = derive_component_status(comp.status_override.as_ref(), &comp_active);
        if derived > overall {
            overall = derived;
        }
        comp_responses.push(ComponentStatusResponse {
            id: comp.id.clone(),
            name: comp.name.clone(),
            description: comp.description.clone(),
            status: derived.as_db_str(),
            group_id: comp.group_id.clone(),
        });
    }

    // Batch fetch updates + component_ids for all incidents
    let all_ids: Vec<Uuid> = active_incidents_with_comps
        .iter()
        .map(|(i, _)| i.id)
        .chain(past_incidents.iter().map(|i| i.id))
        .collect();
    let all_updates = state.repo.get_updates_for_incidents(&all_ids).await?;
    let all_comp_ids = state.repo.get_component_ids_for_incidents(&all_ids).await?;

    let active_responses: Vec<IncidentResponse> = active_incidents_with_comps
        .into_iter()
        .map(|(inc, _)| {
            let updates = all_updates.get(&inc.id).cloned().unwrap_or_default();
            let comp_ids = all_comp_ids.get(&inc.id).cloned().unwrap_or_default();
            incident_to_response(inc, comp_ids, updates)
        })
        .collect();

    let past_responses: Vec<IncidentResponse> = past_incidents
        .into_iter()
        .map(|inc| {
            let updates = all_updates.get(&inc.id).cloned().unwrap_or_default();
            let comp_ids = all_comp_ids.get(&inc.id).cloned().unwrap_or_default();
            incident_to_response(inc, comp_ids, updates)
        })
        .collect();

    Ok(Json(StatusPageResponse {
        overall_status: overall.as_db_str(),
        components: comp_responses,
        component_groups: groups
            .into_iter()
            .map(|g| ComponentGroupResponse {
                id: g.id,
                name: g.name,
            })
            .collect(),
        active_incidents: active_responses,
        past_incidents: past_responses,
    }))
}

// ---------------------------------------------------------------------------
// GET /api/status/uptime
// ---------------------------------------------------------------------------

#[derive(Default)]
struct UptimeAcc {
    days: Vec<DayUptimeResponse>,
    success: i64,
    total: i64,
}

async fn get_uptime(State(state): State<AppState>) -> AppResult<Json<UptimeResponse>> {
    let components = state.repo.list_components().await?;
    let active_incidents_with_comps = state.repo.list_active_incidents_with_components().await?;
    let now = Utc::now();
    let from_date = (now - chrono::Duration::days(30))
        .format("%Y-%m-%d")
        .to_string();
    let to_date = now.format("%Y-%m-%d").to_string();

    let all_uptime = state.repo.get_daily_uptime(&from_date, &to_date).await?;

    // Single accumulator map: component_id -> (days, success, total)
    let mut uptime_map: BTreeMap<String, UptimeAcc> = BTreeMap::new();
    for u in all_uptime {
        let acc = uptime_map.entry(u.component_id).or_default();
        acc.days.push(DayUptimeResponse {
            date: u.date,
            uptime_percent: uptime_pct(u.successful_checks, u.total_checks),
            worst_status: u.worst_status,
            incident_count: u.incident_count,
            total_checks: u.total_checks,
        });
        acc.success += u.successful_checks;
        acc.total += u.total_checks;
    }

    let mut comp_responses = Vec::with_capacity(components.len());
    for comp in &components {
        let comp_active: Vec<StatusIncident> = active_incidents_with_comps
            .iter()
            .filter(|(_, cids)| cids.contains(&comp.id))
            .map(|(inc, _)| inc.clone())
            .collect();
        let derived = derive_component_status(comp.status_override.as_ref(), &comp_active);

        let acc = uptime_map.remove(&comp.id).unwrap_or_default();

        comp_responses.push(ComponentUptimeResponse {
            id: comp.id.clone(),
            name: comp.name.clone(),
            current_status: derived.as_db_str(),
            uptime_percent_30d: uptime_pct(acc.success, acc.total),
            days: acc.days,
        });
    }

    Ok(Json(UptimeResponse {
        components: comp_responses,
    }))
}

// ---------------------------------------------------------------------------
// GET /api/status/uptime/calendar
// ---------------------------------------------------------------------------

async fn get_uptime_calendar(
    State(state): State<AppState>,
    Query(query): Query<UptimeCalendarQuery>,
) -> AppResult<Json<CalendarUptimeResponse>> {
    let component = state
        .repo
        .list_components()
        .await?
        .into_iter()
        .find(|c| c.id == query.component)
        .ok_or(AppError::NotFound)?;

    let mut months = Vec::new();

    for month_str in query.months.split(',').map(|s| s.trim()) {
        // Parse "2026-01" -> date range
        let from_date = format!("{month_str}-01");
        let first = chrono::NaiveDate::parse_from_str(&from_date, "%Y-%m-%d")
            .map_err(|e| AppError::BadRequest(format!("invalid month '{month_str}': {e}")))?;
        let next_month = first
            .checked_add_months(Months::new(1))
            .ok_or_else(|| AppError::BadRequest("date overflow".into()))?;
        let last = next_month - chrono::Duration::days(1);
        let to_date = last.format("%Y-%m-%d").to_string();

        let daily = state
            .repo
            .get_daily_uptime_for_component(&query.component, &from_date, &to_date)
            .await?;

        let (days, total_success, total_checks) = daily.iter().fold(
            (Vec::with_capacity(daily.len()), 0i64, 0i64),
            |(mut days, ts, tc), u| {
                days.push(DayUptimeResponse {
                    date: u.date.clone(),
                    uptime_percent: uptime_pct(u.successful_checks, u.total_checks),
                    worst_status: u.worst_status.clone(),
                    incident_count: u.incident_count,
                    total_checks: u.total_checks,
                });
                (days, ts + u.successful_checks, tc + u.total_checks)
            },
        );

        months.push(MonthUptimeResponse {
            month: month_str.to_string(),
            uptime_percent: uptime_pct(total_success, total_checks),
            days,
        });
    }

    Ok(Json(CalendarUptimeResponse {
        component_id: component.id,
        component_name: component.name,
        months,
    }))
}

// ---------------------------------------------------------------------------
// GET /api/status/history
// ---------------------------------------------------------------------------

async fn get_history(
    State(state): State<AppState>,
    Query(query): Query<HistoryQuery>,
) -> AppResult<Json<HistoryResponse>> {
    let now = Utc::now();

    // Defaults: from = 3 months ago (first of that month), to = end of current month
    let from = query.from.clone().unwrap_or_else(|| {
        let three_months_ago = now
            .date_naive()
            .checked_sub_months(Months::new(3))
            .unwrap_or(now.date_naive());
        format!(
            "{}-{:02}-01",
            three_months_ago.year(),
            three_months_ago.month()
        )
    });
    let to = query.to.clone().unwrap_or_else(|| {
        let next_month = now
            .date_naive()
            .checked_add_months(Months::new(1))
            .unwrap_or(now.date_naive());
        format!("{}-{:02}-01", next_month.year(), next_month.month())
    });

    // Parse from as first of month, to as first of next month after `to`
    let from_date = format!("{from}-01");
    let to_first = chrono::NaiveDate::parse_from_str(&format!("{to}-01"), "%Y-%m-%d")
        .map_err(|e| AppError::BadRequest(format!("invalid 'to' month: {e}")))?;
    let to_date = to_first
        .checked_add_months(Months::new(1))
        .ok_or_else(|| AppError::BadRequest("date overflow".into()))?
        .format("%Y-%m-%d")
        .to_string();

    let component_ids: Option<Vec<String>> = query
        .components
        .as_ref()
        .map(|s| s.split(',').map(|c| c.trim().to_string()).collect());

    let incidents = state
        .repo
        .list_incidents_in_range(&from_date, &to_date, component_ids.as_deref())
        .await?;

    // Batch fetch updates + component_ids
    let ids: Vec<Uuid> = incidents.iter().map(|i| i.id).collect();
    let all_updates = state.repo.get_updates_for_incidents(&ids).await?;
    let all_comp_ids = state.repo.get_component_ids_for_incidents(&ids).await?;

    // Group by month (YYYY-MM)
    let mut month_map: BTreeMap<String, Vec<IncidentResponse>> = BTreeMap::new();
    for inc in incidents {
        let month_key = format!("{}-{:02}", inc.created_at.year(), inc.created_at.month());
        let updates = all_updates.get(&inc.id).cloned().unwrap_or_default();
        let comp_ids = all_comp_ids.get(&inc.id).cloned().unwrap_or_default();
        month_map
            .entry(month_key)
            .or_default()
            .push(incident_to_response(inc, comp_ids, updates));
    }

    let expand_month = query.expand_month.as_deref();
    let preview_count = query.preview_count;

    // BTreeMap.into_iter().rev() gives descending order — no sort needed
    let months_resp: Vec<MonthHistoryResponse> = month_map
        .into_iter()
        .rev()
        .map(|(month, mut incidents_list)| {
            let total = incidents_list.len();
            if expand_month != Some(month.as_str()) && incidents_list.len() > preview_count {
                incidents_list.truncate(preview_count);
            }
            MonthHistoryResponse {
                month,
                total_incidents: total,
                incidents: incidents_list,
            }
        })
        .collect();

    Ok(Json(HistoryResponse {
        from,
        to,
        months: months_resp,
    }))
}

// ---------------------------------------------------------------------------
// GET /api/status/maintenance
// ---------------------------------------------------------------------------

async fn get_maintenance(
    State(state): State<AppState>,
) -> AppResult<Json<MaintenanceListResponse>> {
    let upcoming = state.repo.list_upcoming_maintenances().await?;
    let active = state.repo.list_active_maintenances().await?;
    let completed = state.repo.list_recent_completed_maintenances(15).await?;

    let mut active_responses: Vec<MaintenanceResponse> = active
        .into_iter()
        .chain(upcoming)
        .map(maintenance_to_response)
        .collect();

    // Sort by scheduled_start
    active_responses.sort_by_key(|r| r.scheduled_start);

    let completed_responses: Vec<MaintenanceResponse> = completed
        .into_iter()
        .map(maintenance_to_response)
        .collect();

    Ok(Json(MaintenanceListResponse {
        active: active_responses,
        completed: completed_responses,
    }))
}

// ---------------------------------------------------------------------------
// GET /api/status/feed.atom
// ---------------------------------------------------------------------------

async fn get_atom_feed(State(state): State<AppState>) -> AppResult<Response> {
    let incidents = state.repo.list_recent_incidents(90).await?;
    let maintenances = state.repo.list_active_maintenances().await?;
    let upcoming = state.repo.list_upcoming_maintenances().await?;

    let ids: Vec<Uuid> = incidents.iter().map(|i| i.id).collect();
    let all_updates = state.repo.get_updates_for_incidents(&ids).await?;
    let all_comp_ids = state.repo.get_component_ids_for_incidents(&ids).await?;

    let incident_responses: Vec<IncidentResponse> = incidents
        .into_iter()
        .map(|inc| {
            let updates = all_updates.get(&inc.id).cloned().unwrap_or_default();
            let comp_ids = all_comp_ids.get(&inc.id).cloned().unwrap_or_default();
            incident_to_response(inc, comp_ids, updates)
        })
        .collect();

    let all_maintenances: Vec<MaintenanceResponse> = maintenances
        .into_iter()
        .chain(upcoming)
        .map(maintenance_to_response)
        .collect();

    let xml = build_atom_feed(&incident_responses, &all_maintenances);

    Response::builder()
        .header(header::CONTENT_TYPE, "text/xml; charset=utf-8")
        .body(xml.into())
        .map_err(|e| AppError::BadRequest(format!("failed to build response: {e}")))
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn build_atom_feed(incidents: &[IncidentResponse], maintenances: &[MaintenanceResponse]) -> String {
    let now = Utc::now().to_rfc3339();
    let mut entries = String::new();

    for inc in incidents {
        let last_update = inc.updates.last();
        let updated = last_update
            .map(|u| u.created_at.to_rfc3339())
            .unwrap_or_else(|| inc.created_at.to_rfc3339());
        let last_msg = last_update
            .map(|u| xml_escape(&u.message))
            .unwrap_or_default();
        let components = inc.component_ids.join(", ");
        let content = format!(
            "Status: {} | Severity: {} | Components: {} | {}",
            inc.status,
            inc.severity,
            if components.is_empty() {
                "none"
            } else {
                &components
            },
            last_msg
        );

        entries.push_str(&format!(
            "  <entry>\n    <title>{}</title>\n    <id>urn:uuid:{}</id>\n    <published>{}</published>\n    <updated>{}</updated>\n    <content type=\"text\">{}</content>\n  </entry>\n",
            xml_escape(&inc.title),
            inc.id,
            inc.created_at.to_rfc3339(),
            updated,
            xml_escape(&content),
        ));
    }

    for m in maintenances {
        let content = format!(
            "Status: {} | Scheduled: {} to {} | Components: {} | {}",
            m.status,
            m.scheduled_start.to_rfc3339(),
            m.scheduled_end.to_rfc3339(),
            m.component_ids.join(", "),
            xml_escape(&m.description),
        );

        entries.push_str(&format!(
            "  <entry>\n    <title>Maintenance: {}</title>\n    <id>urn:uuid:{}</id>\n    <published>{}</published>\n    <updated>{}</updated>\n    <content type=\"text\">{}</content>\n  </entry>\n",
            xml_escape(&m.title),
            m.id,
            m.created_at.to_rfc3339(),
            m.created_at.to_rfc3339(),
            xml_escape(&content),
        ));
    }

    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<feed xmlns=\"http://www.w3.org/2005/Atom\">\n  <title>Zomboid Seasons Status</title>\n  <id>https://status.example.com/</id>\n  <link href=\"https://status.example.com/\" rel=\"alternate\"/>\n  <link href=\"https://status.example.com/api/status/feed.atom\" rel=\"self\"/>\n  <updated>{now}</updated>\n{entries}</feed>\n"
    )
}

// ---------------------------------------------------------------------------
// GET /api/status/incidents/{id}
// ---------------------------------------------------------------------------

async fn get_incident_detail(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<IncidentResponse>> {
    let uuid = Uuid::parse_str(&id)
        .map_err(|e| AppError::BadRequest(format!("invalid incident id: {e}")))?;

    let incident = state
        .repo
        .get_incident_by_id(uuid)
        .await?
        .ok_or(AppError::NotFound)?;

    let (updates, comp_ids) = tokio::join!(
        state.repo.get_updates_for_incident(uuid),
        state.repo.get_component_ids_for_incident(uuid)
    );

    Ok(Json(incident_to_response(incident, comp_ids?, updates?)))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn incident_to_response(
    inc: StatusIncident,
    comp_ids: Vec<String>,
    updates: Vec<StatusIncidentUpdate>,
) -> IncidentResponse {
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
}

pub fn maintenance_to_response(m: ScheduledMaintenance) -> MaintenanceResponse {
    MaintenanceResponse {
        id: m.id,
        title: m.title,
        description: m.description,
        status: m.status.as_db_str(),
        component_ids: m.component_ids,
        scheduled_start: m.scheduled_start,
        scheduled_end: m.scheduled_end,
        actual_start: m.actual_start,
        actual_end: m.actual_end,
        created_at: m.created_at,
    }
}

fn uptime_pct(successful: i64, total: i64) -> f64 {
    if total > 0 {
        (successful as f64 / total as f64) * 100.0
    } else {
        100.0
    }
}
