use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Request types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct CreateIncidentRequest {
    pub title: String,
    pub severity: String,
    pub component_ids: Vec<String>,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct AddIncidentUpdateRequest {
    pub status: String,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct SetComponentOverrideRequest {
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PatchIncidentRequest {
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateMaintenanceRequest {
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub component_ids: Vec<String>,
    pub scheduled_start: DateTime<Utc>,
    pub scheduled_end: DateTime<Utc>,
    #[serde(default = "default_true")]
    pub auto_suppress_incidents: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize)]
pub struct PatchMaintenanceRequest {
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub struct SetPostmortemRequest {
    pub body: String,
}

// ---------------------------------------------------------------------------
// Query param types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct UptimeCalendarQuery {
    pub component: String,
    pub months: String, // comma-separated: "2026-01,2026-02"
}

#[derive(Debug, Deserialize)]
pub struct HistoryQuery {
    pub from: Option<String>,
    pub to: Option<String>,
    pub components: Option<String>, // comma-separated
    pub expand_month: Option<String>,
    #[serde(default = "default_preview_count")]
    pub preview_count: usize,
}

fn default_preview_count() -> usize {
    3
}

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct StatusPageResponse {
    pub overall_status: &'static str,
    pub components: Vec<ComponentStatusResponse>,
    pub component_groups: Vec<ComponentGroupResponse>,
    pub active_incidents: Vec<IncidentResponse>,
    pub past_incidents: Vec<IncidentResponse>,
}

#[derive(Debug, Serialize)]
pub struct ComponentStatusResponse {
    pub id: String,
    pub name: String,
    pub description: String,
    pub status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ComponentGroupResponse {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct IncidentResponse {
    pub id: Uuid,
    pub title: String,
    pub status: &'static str,
    pub severity: &'static str,
    pub component_ids: Vec<String>,
    pub updates: Vec<IncidentUpdateResponse>,
    pub created_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub postmortem_body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub postmortem_published_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct IncidentUpdateResponse {
    pub id: Uuid,
    pub status: &'static str,
    pub message: String,
    pub created_at: DateTime<Utc>,
}

// Uptime

#[derive(Debug, Serialize)]
pub struct UptimeResponse {
    pub components: Vec<ComponentUptimeResponse>,
}

#[derive(Debug, Serialize)]
pub struct ComponentUptimeResponse {
    pub id: String,
    pub name: String,
    pub current_status: &'static str,
    pub uptime_percent_30d: f64,
    pub days: Vec<DayUptimeResponse>,
}

#[derive(Debug, Serialize)]
pub struct DayUptimeResponse {
    pub date: String,
    pub uptime_percent: f64,
    pub worst_status: String,
    pub incident_count: i64,
    pub total_checks: i64,
}

// Calendar

#[derive(Debug, Serialize)]
pub struct CalendarUptimeResponse {
    pub component_id: String,
    pub component_name: String,
    pub months: Vec<MonthUptimeResponse>,
}

#[derive(Debug, Serialize)]
pub struct MonthUptimeResponse {
    pub month: String,
    pub uptime_percent: f64,
    pub days: Vec<DayUptimeResponse>,
}

// History

#[derive(Debug, Serialize)]
pub struct HistoryResponse {
    pub from: String,
    pub to: String,
    pub months: Vec<MonthHistoryResponse>,
}

#[derive(Debug, Serialize)]
pub struct MonthHistoryResponse {
    pub month: String,
    pub total_incidents: usize,
    pub incidents: Vec<IncidentResponse>,
}

// Maintenance

#[derive(Debug, Serialize)]
pub struct MaintenanceListResponse {
    pub active: Vec<MaintenanceResponse>,
    pub completed: Vec<MaintenanceResponse>,
}

#[derive(Debug, Serialize)]
pub struct MaintenanceResponse {
    pub id: Uuid,
    pub title: String,
    pub description: String,
    pub status: &'static str,
    pub component_ids: Vec<String>,
    pub scheduled_start: DateTime<Utc>,
    pub scheduled_end: DateTime<Utc>,
    pub actual_start: Option<DateTime<Utc>>,
    pub actual_end: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}
