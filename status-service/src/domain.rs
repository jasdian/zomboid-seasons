use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AppError;

// ---------------------------------------------------------------------------
// Status page enums
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComponentStatus {
    Operational,
    Degraded,
    PartialOutage,
    MajorOutage,
    Maintenance,
}

impl ComponentStatus {
    pub fn as_db_str(self) -> &'static str {
        match self {
            ComponentStatus::Operational => "operational",
            ComponentStatus::Degraded => "degraded",
            ComponentStatus::PartialOutage => "partial_outage",
            ComponentStatus::MajorOutage => "major_outage",
            ComponentStatus::Maintenance => "maintenance",
        }
    }

    /// Numeric severity for ordering. Higher = worse.
    fn severity_ord(self) -> u8 {
        match self {
            ComponentStatus::Operational => 0,
            ComponentStatus::Degraded => 1,
            ComponentStatus::PartialOutage => 2,
            ComponentStatus::MajorOutage => 3,
            ComponentStatus::Maintenance => 3, // treat as equal to MajorOutage for ordering
        }
    }
}

impl PartialOrd for ComponentStatus {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ComponentStatus {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.severity_ord().cmp(&other.severity_ord())
    }
}

impl TryFrom<&str> for ComponentStatus {
    type Error = AppError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "operational" => Ok(ComponentStatus::Operational),
            "degraded" => Ok(ComponentStatus::Degraded),
            "partial_outage" => Ok(ComponentStatus::PartialOutage),
            "major_outage" => Ok(ComponentStatus::MajorOutage),
            "maintenance" => Ok(ComponentStatus::Maintenance),
            other => Err(AppError::BadRequest(format!(
                "unknown component status: {other}"
            ))),
        }
    }
}

impl std::fmt::Display for ComponentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_db_str())
    }
}

// ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IncidentStatus {
    Investigating,
    Identified,
    Monitoring,
    Resolved,
}

impl IncidentStatus {
    pub fn as_db_str(self) -> &'static str {
        match self {
            IncidentStatus::Investigating => "investigating",
            IncidentStatus::Identified => "identified",
            IncidentStatus::Monitoring => "monitoring",
            IncidentStatus::Resolved => "resolved",
        }
    }
}

impl TryFrom<&str> for IncidentStatus {
    type Error = AppError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "investigating" => Ok(IncidentStatus::Investigating),
            "identified" => Ok(IncidentStatus::Identified),
            "monitoring" => Ok(IncidentStatus::Monitoring),
            "resolved" => Ok(IncidentStatus::Resolved),
            other => Err(AppError::BadRequest(format!(
                "unknown incident status: {other}"
            ))),
        }
    }
}

impl std::fmt::Display for IncidentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_db_str())
    }
}

// ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IncidentSeverity {
    Minor,
    Major,
    Critical,
}

impl IncidentSeverity {
    pub fn as_db_str(self) -> &'static str {
        match self {
            IncidentSeverity::Minor => "minor",
            IncidentSeverity::Major => "major",
            IncidentSeverity::Critical => "critical",
        }
    }
}

impl TryFrom<&str> for IncidentSeverity {
    type Error = AppError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "minor" => Ok(IncidentSeverity::Minor),
            "major" => Ok(IncidentSeverity::Major),
            "critical" => Ok(IncidentSeverity::Critical),
            other => Err(AppError::BadRequest(format!(
                "unknown incident severity: {other}"
            ))),
        }
    }
}

impl std::fmt::Display for IncidentSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_db_str())
    }
}

// ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateStatus {
    Investigating,
    Identified,
    Monitoring,
    Update,
    Resolved,
}

impl UpdateStatus {
    pub fn as_db_str(self) -> &'static str {
        match self {
            UpdateStatus::Investigating => "investigating",
            UpdateStatus::Identified => "identified",
            UpdateStatus::Monitoring => "monitoring",
            UpdateStatus::Update => "update",
            UpdateStatus::Resolved => "resolved",
        }
    }
}

impl TryFrom<&str> for UpdateStatus {
    type Error = AppError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "investigating" => Ok(UpdateStatus::Investigating),
            "identified" => Ok(UpdateStatus::Identified),
            "monitoring" => Ok(UpdateStatus::Monitoring),
            "update" => Ok(UpdateStatus::Update),
            "resolved" => Ok(UpdateStatus::Resolved),
            other => Err(AppError::BadRequest(format!(
                "unknown update status: {other}"
            ))),
        }
    }
}

impl std::fmt::Display for UpdateStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_db_str())
    }
}

// ---------------------------------------------------------------------------
// Status page domain structs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct StatusComponent {
    pub id: String,
    pub name: String,
    pub description: String,
    pub position: i64,
    pub group_id: Option<String>,
    pub status_override: Option<ComponentStatus>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ComponentGroup {
    pub id: String,
    pub name: String,
    pub position: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct StatusIncident {
    pub id: Uuid,
    pub title: String,
    pub status: IncidentStatus,
    pub severity: IncidentSeverity,
    pub auto_detected: bool,
    pub created_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub postmortem_body: Option<String>,
    pub postmortem_published_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StatusIncidentUpdate {
    pub id: Uuid,
    pub incident_id: Uuid,
    pub status: UpdateStatus,
    pub message: String,
    pub created_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Status page pure domain functions
// ---------------------------------------------------------------------------

pub fn derive_component_status(
    override_status: Option<&ComponentStatus>,
    active_incidents: &[StatusIncident],
) -> ComponentStatus {
    if let Some(s) = override_status {
        return *s;
    }
    active_incidents
        .iter()
        .map(|inc| match inc.severity {
            IncidentSeverity::Critical => ComponentStatus::MajorOutage,
            IncidentSeverity::Major => ComponentStatus::PartialOutage,
            IncidentSeverity::Minor => ComponentStatus::Degraded,
        })
        .max()
        .unwrap_or(ComponentStatus::Operational)
}

// ---------------------------------------------------------------------------
// Maintenance
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaintenanceStatus {
    Scheduled,
    InProgress,
    Completed,
}

impl MaintenanceStatus {
    pub fn as_db_str(self) -> &'static str {
        match self {
            MaintenanceStatus::Scheduled => "scheduled",
            MaintenanceStatus::InProgress => "in_progress",
            MaintenanceStatus::Completed => "completed",
        }
    }
}

impl TryFrom<&str> for MaintenanceStatus {
    type Error = AppError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "scheduled" => Ok(MaintenanceStatus::Scheduled),
            "in_progress" => Ok(MaintenanceStatus::InProgress),
            "completed" => Ok(MaintenanceStatus::Completed),
            other => Err(AppError::BadRequest(format!(
                "unknown maintenance status: {other}"
            ))),
        }
    }
}

impl std::fmt::Display for MaintenanceStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_db_str())
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ScheduledMaintenance {
    pub id: Uuid,
    pub title: String,
    pub description: String,
    pub status: MaintenanceStatus,
    pub component_ids: Vec<String>,
    pub scheduled_start: DateTime<Utc>,
    pub scheduled_end: DateTime<Utc>,
    pub actual_start: Option<DateTime<Utc>>,
    pub actual_end: Option<DateTime<Utc>>,
    pub auto_suppress_incidents: bool,
    pub created_at: DateTime<Utc>,
}
