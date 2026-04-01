use std::collections::HashMap;
use std::future::Future;

use chrono::{DateTime, NaiveDateTime, Utc};
use sqlx::sqlite::SqlitePool;
use sqlx::FromRow;
use uuid::Uuid;

use crate::config::{ComponentConfig, ComponentGroupConfig};
use crate::domain::{
    ComponentGroup, ComponentStatus, IncidentSeverity, IncidentStatus, MaintenanceStatus,
    ScheduledMaintenance, StatusComponent, StatusIncident, StatusIncidentUpdate, UpdateStatus,
};
use crate::error::{AppError, AppResult};

// ---------------------------------------------------------------------------
// Row types
// ---------------------------------------------------------------------------

#[derive(Debug, FromRow)]
struct ComponentRow {
    id: String,
    name: String,
    description: String,
    position: i64,
    group_id: Option<String>,
    status_override: Option<String>,
    created_at: String,
}

#[derive(Debug, FromRow)]
struct ComponentGroupRow {
    id: String,
    name: String,
    position: i64,
}

#[derive(Debug, FromRow)]
struct IncidentRow {
    id: String,
    title: String,
    status: String,
    severity: String,
    auto_detected: i32,
    created_at: String,
    resolved_at: Option<String>,
    postmortem_body: Option<String>,
    postmortem_published_at: Option<String>,
}

#[derive(Debug, FromRow)]
struct IncidentWithComponentsRow {
    id: String,
    title: String,
    status: String,
    severity: String,
    auto_detected: i32,
    created_at: String,
    resolved_at: Option<String>,
    postmortem_body: Option<String>,
    postmortem_published_at: Option<String>,
    component_ids: Option<String>,
}

#[derive(Debug, FromRow)]
struct IncidentComponentRow {
    incident_id: String,
    component_id: String,
}

#[derive(Debug, FromRow)]
struct IncidentUpdateRow {
    id: String,
    incident_id: String,
    status: String,
    message: String,
    created_at: String,
}

#[derive(Debug, FromRow)]
struct DailyUptimeRow {
    component_id: String,
    date: String,
    total_checks: i64,
    successful_checks: i64,
    uptime_seconds: i64,
    worst_status: String,
    incident_count: i64,
}

#[derive(Debug, FromRow)]
struct MaintenanceRow {
    id: String,
    title: String,
    description: String,
    status: String,
    component_ids: String,
    scheduled_start: String,
    scheduled_end: String,
    actual_start: Option<String>,
    actual_end: Option<String>,
    auto_suppress_incidents: i32,
    created_at: String,
}

// ---------------------------------------------------------------------------
// Public DTO
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct DailyUptime {
    pub component_id: String,
    pub date: String,
    pub total_checks: i64,
    pub successful_checks: i64,
    pub uptime_seconds: i64,
    pub worst_status: String,
    pub incident_count: i64,
}

impl From<DailyUptimeRow> for DailyUptime {
    fn from(row: DailyUptimeRow) -> Self {
        DailyUptime {
            component_id: row.component_id,
            date: row.date,
            total_checks: row.total_checks,
            successful_checks: row.successful_checks,
            uptime_seconds: row.uptime_seconds,
            worst_status: row.worst_status,
            incident_count: row.incident_count,
        }
    }
}

// ---------------------------------------------------------------------------
// Datetime parsing
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

impl TryFrom<ComponentRow> for StatusComponent {
    type Error = AppError;

    fn try_from(row: ComponentRow) -> Result<Self, Self::Error> {
        Ok(StatusComponent {
            id: row.id,
            name: row.name,
            description: row.description,
            position: row.position,
            group_id: row.group_id,
            status_override: row
                .status_override
                .map(|s| ComponentStatus::try_from(s.as_str()))
                .transpose()?,
            created_at: parse_datetime(&row.created_at)?,
        })
    }
}

impl TryFrom<IncidentRow> for StatusIncident {
    type Error = AppError;

    fn try_from(row: IncidentRow) -> Result<Self, Self::Error> {
        Ok(StatusIncident {
            id: Uuid::parse_str(&row.id)
                .map_err(|e| AppError::BadRequest(format!("invalid uuid: {e}")))?,
            title: row.title,
            status: IncidentStatus::try_from(row.status.as_str())?,
            severity: IncidentSeverity::try_from(row.severity.as_str())?,
            auto_detected: row.auto_detected != 0,
            created_at: parse_datetime(&row.created_at)?,
            resolved_at: row.resolved_at.map(|s| parse_datetime(&s)).transpose()?,
            postmortem_body: row.postmortem_body,
            postmortem_published_at: row
                .postmortem_published_at
                .map(|s| parse_datetime(&s))
                .transpose()?,
        })
    }
}

impl TryFrom<IncidentUpdateRow> for StatusIncidentUpdate {
    type Error = AppError;

    fn try_from(row: IncidentUpdateRow) -> Result<Self, Self::Error> {
        Ok(StatusIncidentUpdate {
            id: Uuid::parse_str(&row.id)
                .map_err(|e| AppError::BadRequest(format!("invalid uuid: {e}")))?,
            incident_id: Uuid::parse_str(&row.incident_id)
                .map_err(|e| AppError::BadRequest(format!("invalid uuid: {e}")))?,
            status: UpdateStatus::try_from(row.status.as_str())?,
            message: row.message,
            created_at: parse_datetime(&row.created_at)?,
        })
    }
}

impl TryFrom<MaintenanceRow> for ScheduledMaintenance {
    type Error = AppError;

    fn try_from(row: MaintenanceRow) -> Result<Self, Self::Error> {
        let component_ids: Vec<String> = serde_json::from_str(&row.component_ids)
            .map_err(|e| AppError::BadRequest(format!("invalid component_ids JSON: {e}")))?;
        Ok(ScheduledMaintenance {
            id: Uuid::parse_str(&row.id)
                .map_err(|e| AppError::BadRequest(format!("invalid uuid: {e}")))?,
            title: row.title,
            description: row.description,
            status: MaintenanceStatus::try_from(row.status.as_str())?,
            component_ids,
            scheduled_start: parse_datetime(&row.scheduled_start)?,
            scheduled_end: parse_datetime(&row.scheduled_end)?,
            actual_start: row.actual_start.map(|s| parse_datetime(&s)).transpose()?,
            actual_end: row.actual_end.map(|s| parse_datetime(&s)).transpose()?,
            auto_suppress_incidents: row.auto_suppress_incidents != 0,
            created_at: parse_datetime(&row.created_at)?,
        })
    }
}

// ---------------------------------------------------------------------------
// StatusRepo trait
// ---------------------------------------------------------------------------

pub trait StatusRepo {
    fn list_components(&self) -> impl Future<Output = AppResult<Vec<StatusComponent>>> + Send;
    fn set_component_override(
        &self,
        id: &str,
        status: Option<ComponentStatus>,
    ) -> impl Future<Output = AppResult<()>> + Send;
    fn create_incident(
        &self,
        incident: &StatusIncident,
    ) -> impl Future<Output = AppResult<()>> + Send;
    fn get_incident_by_id(
        &self,
        id: Uuid,
    ) -> impl Future<Output = AppResult<Option<StatusIncident>>> + Send;
    fn list_active_incidents(&self) -> impl Future<Output = AppResult<Vec<StatusIncident>>> + Send;
    fn list_recent_incidents(
        &self,
        days: i64,
    ) -> impl Future<Output = AppResult<Vec<StatusIncident>>> + Send;
    fn list_all_incidents(&self) -> impl Future<Output = AppResult<Vec<StatusIncident>>> + Send;
    fn update_incident_status(
        &self,
        id: Uuid,
        status: IncidentStatus,
        resolved_at: Option<DateTime<Utc>>,
    ) -> impl Future<Output = AppResult<()>> + Send;
    fn delete_incident(&self, id: Uuid) -> impl Future<Output = AppResult<()>> + Send;
    fn add_incident_update(
        &self,
        update: &StatusIncidentUpdate,
    ) -> impl Future<Output = AppResult<()>> + Send;
    fn get_updates_for_incident(
        &self,
        incident_id: Uuid,
    ) -> impl Future<Output = AppResult<Vec<StatusIncidentUpdate>>> + Send;
    fn link_incident_components(
        &self,
        incident_id: Uuid,
        component_ids: &[String],
    ) -> impl Future<Output = AppResult<()>> + Send;
    fn get_component_ids_for_incident(
        &self,
        incident_id: Uuid,
    ) -> impl Future<Output = AppResult<Vec<String>>> + Send;
    fn get_active_incidents_for_component(
        &self,
        component_id: &str,
    ) -> impl Future<Output = AppResult<Vec<StatusIncident>>> + Send;
    fn upsert_component(
        &self,
        component: &ComponentConfig,
    ) -> impl Future<Output = AppResult<()>> + Send;
    fn upsert_component_group(
        &self,
        group: &ComponentGroupConfig,
    ) -> impl Future<Output = AppResult<()>> + Send;
    fn list_component_groups(&self) -> impl Future<Output = AppResult<Vec<ComponentGroup>>> + Send;

    // Probe results
    fn insert_probe_result(
        &self,
        component_id: &str,
        success: bool,
        response_ms: Option<i64>,
        error_message: Option<&str>,
    ) -> impl Future<Output = AppResult<()>> + Send;
    fn delete_old_probe_results(&self, days: i64) -> impl Future<Output = AppResult<()>> + Send;

    // Auto-incident support
    fn find_active_auto_incident(
        &self,
        component_id: &str,
    ) -> impl Future<Output = AppResult<Option<Uuid>>> + Send;
    fn update_incident_severity(
        &self,
        id: Uuid,
        severity: IncidentSeverity,
    ) -> impl Future<Output = AppResult<()>> + Send;

    // Uptime
    fn aggregate_daily_uptime(&self) -> impl Future<Output = AppResult<()>> + Send;
    fn get_daily_uptime(
        &self,
        from_date: &str,
        to_date: &str,
    ) -> impl Future<Output = AppResult<Vec<DailyUptime>>> + Send;
    fn get_daily_uptime_for_component(
        &self,
        component_id: &str,
        from_date: &str,
        to_date: &str,
    ) -> impl Future<Output = AppResult<Vec<DailyUptime>>> + Send;

    // Incident history
    fn list_incidents_in_range(
        &self,
        from: &str,
        to: &str,
        component_ids: Option<&[String]>,
    ) -> impl Future<Output = AppResult<Vec<StatusIncident>>> + Send;

    // Batch queries
    fn get_updates_for_incidents(
        &self,
        ids: &[Uuid],
    ) -> impl Future<Output = AppResult<HashMap<Uuid, Vec<StatusIncidentUpdate>>>> + Send;
    fn get_component_ids_for_incidents(
        &self,
        ids: &[Uuid],
    ) -> impl Future<Output = AppResult<HashMap<Uuid, Vec<String>>>> + Send;
    fn list_active_incidents_with_components(
        &self,
    ) -> impl Future<Output = AppResult<Vec<(StatusIncident, Vec<String>)>>> + Send;
    fn list_recent_resolved_incidents(
        &self,
        days: i64,
    ) -> impl Future<Output = AppResult<Vec<StatusIncident>>> + Send;

    // Postmortem
    fn set_postmortem(&self, id: Uuid, body: &str) -> impl Future<Output = AppResult<()>> + Send;

    // Maintenance
    fn create_maintenance(
        &self,
        m: &ScheduledMaintenance,
    ) -> impl Future<Output = AppResult<()>> + Send;
    fn get_maintenance_by_id(
        &self,
        id: Uuid,
    ) -> impl Future<Output = AppResult<Option<ScheduledMaintenance>>> + Send;
    fn update_maintenance_status(
        &self,
        id: Uuid,
        status: MaintenanceStatus,
        actual_start: Option<DateTime<Utc>>,
        actual_end: Option<DateTime<Utc>>,
    ) -> impl Future<Output = AppResult<()>> + Send;
    fn list_active_maintenances(
        &self,
    ) -> impl Future<Output = AppResult<Vec<ScheduledMaintenance>>> + Send;
    fn list_upcoming_maintenances(
        &self,
    ) -> impl Future<Output = AppResult<Vec<ScheduledMaintenance>>> + Send;
    fn is_component_in_maintenance(
        &self,
        component_id: &str,
    ) -> impl Future<Output = AppResult<bool>> + Send;
    fn list_all_maintenances(
        &self,
    ) -> impl Future<Output = AppResult<Vec<ScheduledMaintenance>>> + Send;
    fn list_recent_completed_maintenances(
        &self,
        days: i64,
    ) -> impl Future<Output = AppResult<Vec<ScheduledMaintenance>>> + Send;
    fn update_maintenance_scheduled_end(
        &self,
        id: Uuid,
        scheduled_end: DateTime<Utc>,
    ) -> impl Future<Output = AppResult<()>> + Send;
}

// ---------------------------------------------------------------------------
// SqliteRepo
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct SqliteRepo {
    pool: SqlitePool,
}

impl SqliteRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn run_migrations(&self) -> AppResult<()> {
        sqlx::migrate!("./migrations")
            .run(&self.pool)
            .await
            .map_err(|e| AppError::Config(format!("migration failed: {e}")))?;
        Ok(())
    }
}

impl StatusRepo for SqliteRepo {
    async fn list_components(&self) -> AppResult<Vec<StatusComponent>> {
        sqlx::query_as::<_, ComponentRow>(
            "SELECT id, name, description, position, group_id, status_override, created_at \
             FROM components ORDER BY position ASC",
        )
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(StatusComponent::try_from)
        .collect()
    }

    async fn set_component_override(
        &self,
        id: &str,
        status: Option<ComponentStatus>,
    ) -> AppResult<()> {
        let status_str = status.map(|s| s.as_db_str().to_string());
        sqlx::query("UPDATE components SET status_override = ? WHERE id = ?")
            .bind(&status_str)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn create_incident(&self, incident: &StatusIncident) -> AppResult<()> {
        let resolved_str = incident.resolved_at.map(|dt| dt.to_rfc3339());
        sqlx::query(
            "INSERT INTO incidents (id, title, status, severity, auto_detected, created_at, resolved_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(incident.id.to_string())
        .bind(&incident.title)
        .bind(incident.status.as_db_str())
        .bind(incident.severity.as_db_str())
        .bind(incident.auto_detected as i32)
        .bind(incident.created_at.to_rfc3339())
        .bind(&resolved_str)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn get_incident_by_id(&self, id: Uuid) -> AppResult<Option<StatusIncident>> {
        sqlx::query_as::<_, IncidentRow>(
            "SELECT id, title, status, severity, auto_detected, created_at, resolved_at, \
             postmortem_body, postmortem_published_at \
             FROM incidents WHERE id = ?",
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?
        .map(StatusIncident::try_from)
        .transpose()
    }

    async fn list_active_incidents(&self) -> AppResult<Vec<StatusIncident>> {
        sqlx::query_as::<_, IncidentRow>(
            "SELECT id, title, status, severity, auto_detected, created_at, resolved_at, \
             postmortem_body, postmortem_published_at \
             FROM incidents WHERE status != 'resolved' \
             ORDER BY created_at DESC",
        )
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(StatusIncident::try_from)
        .collect()
    }

    async fn list_recent_incidents(&self, days: i64) -> AppResult<Vec<StatusIncident>> {
        sqlx::query_as::<_, IncidentRow>(
            "SELECT id, title, status, severity, auto_detected, created_at, resolved_at, \
             postmortem_body, postmortem_published_at \
             FROM incidents \
             WHERE created_at >= datetime('now', ? || ' days') \
             ORDER BY created_at DESC",
        )
        .bind(-days)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(StatusIncident::try_from)
        .collect()
    }

    async fn list_all_incidents(&self) -> AppResult<Vec<StatusIncident>> {
        sqlx::query_as::<_, IncidentRow>(
            "SELECT id, title, status, severity, auto_detected, created_at, resolved_at, \
             postmortem_body, postmortem_published_at \
             FROM incidents ORDER BY created_at DESC",
        )
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(StatusIncident::try_from)
        .collect()
    }

    async fn update_incident_status(
        &self,
        id: Uuid,
        status: IncidentStatus,
        resolved_at: Option<DateTime<Utc>>,
    ) -> AppResult<()> {
        let resolved_str = resolved_at.map(|dt| dt.to_rfc3339());
        sqlx::query("UPDATE incidents SET status = ?, resolved_at = ? WHERE id = ?")
            .bind(status.as_db_str())
            .bind(&resolved_str)
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn delete_incident(&self, id: Uuid) -> AppResult<()> {
        sqlx::query("DELETE FROM incidents WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn add_incident_update(&self, update: &StatusIncidentUpdate) -> AppResult<()> {
        sqlx::query(
            "INSERT INTO incident_updates (id, incident_id, status, message, created_at) \
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(update.id.to_string())
        .bind(update.incident_id.to_string())
        .bind(update.status.as_db_str())
        .bind(&update.message)
        .bind(update.created_at.to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn get_updates_for_incident(
        &self,
        incident_id: Uuid,
    ) -> AppResult<Vec<StatusIncidentUpdate>> {
        sqlx::query_as::<_, IncidentUpdateRow>(
            "SELECT id, incident_id, status, message, created_at \
             FROM incident_updates WHERE incident_id = ? \
             ORDER BY created_at ASC",
        )
        .bind(incident_id.to_string())
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(StatusIncidentUpdate::try_from)
        .collect()
    }

    async fn link_incident_components(
        &self,
        incident_id: Uuid,
        component_ids: &[String],
    ) -> AppResult<()> {
        let id_str = incident_id.to_string();
        for cid in component_ids {
            sqlx::query(
                "INSERT INTO incident_components (incident_id, component_id) \
                 VALUES (?, ?)",
            )
            .bind(&id_str)
            .bind(cid)
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    async fn get_component_ids_for_incident(&self, incident_id: Uuid) -> AppResult<Vec<String>> {
        let rows: Vec<(String,)> =
            sqlx::query_as("SELECT component_id FROM incident_components WHERE incident_id = ?")
                .bind(incident_id.to_string())
                .fetch_all(&self.pool)
                .await?;
        Ok(rows.into_iter().map(|(cid,)| cid).collect())
    }

    async fn get_active_incidents_for_component(
        &self,
        component_id: &str,
    ) -> AppResult<Vec<StatusIncident>> {
        sqlx::query_as::<_, IncidentRow>(
            "SELECT i.id, i.title, i.status, i.severity, i.auto_detected, i.created_at, i.resolved_at, \
             i.postmortem_body, i.postmortem_published_at \
             FROM incidents i \
             INNER JOIN incident_components ic ON ic.incident_id = i.id \
             WHERE ic.component_id = ? AND i.status != 'resolved' \
             ORDER BY i.created_at DESC",
        )
        .bind(component_id)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(StatusIncident::try_from)
        .collect()
    }

    async fn upsert_component(&self, component: &ComponentConfig) -> AppResult<()> {
        sqlx::query(
            "INSERT INTO components (id, name, description, position, group_id) VALUES (?, ?, ?, ?, ?) \
             ON CONFLICT(id) DO UPDATE SET name = excluded.name, \
             description = excluded.description, position = excluded.position, \
             group_id = excluded.group_id",
        )
        .bind(&component.id)
        .bind(&component.name)
        .bind(&component.description)
        .bind(component.position)
        .bind(&component.group_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn upsert_component_group(&self, group: &ComponentGroupConfig) -> AppResult<()> {
        sqlx::query(
            "INSERT INTO component_groups (id, name, position) VALUES (?, ?, ?) \
             ON CONFLICT(id) DO UPDATE SET name = excluded.name, position = excluded.position",
        )
        .bind(&group.id)
        .bind(&group.name)
        .bind(group.position)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn list_component_groups(&self) -> AppResult<Vec<ComponentGroup>> {
        sqlx::query_as::<_, ComponentGroupRow>(
            "SELECT id, name, position FROM component_groups ORDER BY position ASC",
        )
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|row| {
            Ok(ComponentGroup {
                id: row.id,
                name: row.name,
                position: row.position,
            })
        })
        .collect()
    }

    // -- Probe results --

    async fn insert_probe_result(
        &self,
        component_id: &str,
        success: bool,
        response_ms: Option<i64>,
        error_message: Option<&str>,
    ) -> AppResult<()> {
        sqlx::query(
            "INSERT INTO probe_results (component_id, success, response_ms, error_message, checked_at) \
             VALUES (?, ?, ?, ?, datetime('now'))",
        )
        .bind(component_id)
        .bind(success as i32)
        .bind(response_ms)
        .bind(error_message)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn delete_old_probe_results(&self, days: i64) -> AppResult<()> {
        sqlx::query(
            "DELETE FROM probe_results WHERE checked_at < datetime('now', '-' || ? || ' days')",
        )
        .bind(days)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    // -- Auto-incident support --

    async fn find_active_auto_incident(&self, component_id: &str) -> AppResult<Option<Uuid>> {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT i.id FROM incidents i \
             INNER JOIN incident_components ic ON ic.incident_id = i.id \
             WHERE ic.component_id = ? AND i.status != 'resolved' AND i.auto_detected = 1 \
             ORDER BY i.created_at DESC LIMIT 1",
        )
        .bind(component_id)
        .fetch_optional(&self.pool)
        .await?;

        row.map(|(id,)| {
            Uuid::parse_str(&id).map_err(|e| AppError::BadRequest(format!("invalid uuid: {e}")))
        })
        .transpose()
    }

    async fn update_incident_severity(
        &self,
        id: Uuid,
        severity: IncidentSeverity,
    ) -> AppResult<()> {
        sqlx::query("UPDATE incidents SET severity = ? WHERE id = ?")
            .bind(severity.as_db_str())
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // -- Uptime --

    async fn aggregate_daily_uptime(&self) -> AppResult<()> {
        sqlx::query(
            "INSERT INTO component_daily_uptime (component_id, date, total_checks, successful_checks, uptime_seconds) \
             SELECT component_id, date(checked_at), COUNT(*), SUM(success), \
                    CAST(86400.0 * SUM(success) / MAX(COUNT(*), 1) AS INTEGER) \
             FROM probe_results WHERE date(checked_at) = date('now') \
             GROUP BY component_id \
             ON CONFLICT(component_id, date) DO UPDATE SET \
                 total_checks = excluded.total_checks, \
                 successful_checks = excluded.successful_checks, \
                 uptime_seconds = excluded.uptime_seconds",
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn get_daily_uptime(
        &self,
        from_date: &str,
        to_date: &str,
    ) -> AppResult<Vec<DailyUptime>> {
        Ok(sqlx::query_as::<_, DailyUptimeRow>(
            "SELECT component_id, date, total_checks, successful_checks, uptime_seconds, worst_status, incident_count \
             FROM component_daily_uptime \
             WHERE date >= ? AND date <= ? \
             ORDER BY component_id, date ASC",
        )
        .bind(from_date)
        .bind(to_date)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(DailyUptime::from)
        .collect())
    }

    async fn get_daily_uptime_for_component(
        &self,
        component_id: &str,
        from_date: &str,
        to_date: &str,
    ) -> AppResult<Vec<DailyUptime>> {
        Ok(sqlx::query_as::<_, DailyUptimeRow>(
            "SELECT component_id, date, total_checks, successful_checks, uptime_seconds, worst_status, incident_count \
             FROM component_daily_uptime \
             WHERE component_id = ? AND date >= ? AND date <= ? \
             ORDER BY date ASC",
        )
        .bind(component_id)
        .bind(from_date)
        .bind(to_date)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(DailyUptime::from)
        .collect())
    }

    // -- Incident history --

    async fn list_incidents_in_range(
        &self,
        from: &str,
        to: &str,
        component_ids: Option<&[String]>,
    ) -> AppResult<Vec<StatusIncident>> {
        match component_ids {
            None => sqlx::query_as::<_, IncidentRow>(
                "SELECT id, title, status, severity, auto_detected, created_at, resolved_at, \
                     postmortem_body, postmortem_published_at \
                     FROM incidents \
                     WHERE created_at >= ? AND created_at < ? \
                     ORDER BY created_at DESC",
            )
            .bind(from)
            .bind(to)
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .map(StatusIncident::try_from)
            .collect(),
            Some([]) => Ok(Vec::new()),
            Some(ids) => {
                // Build dynamic query for IN clause since sqlx doesn't support
                // binding slices. Use placeholders + positional binds.
                let placeholders: Vec<&str> = ids.iter().map(|_| "?").collect();
                let in_clause = placeholders.join(", ");
                let sql = format!(
                    "SELECT DISTINCT i.id, i.title, i.status, i.severity, i.auto_detected, i.created_at, i.resolved_at, \
                     i.postmortem_body, i.postmortem_published_at \
                     FROM incidents i \
                     INNER JOIN incident_components ic ON ic.incident_id = i.id \
                     WHERE i.created_at >= ? AND i.created_at < ? \
                     AND ic.component_id IN ({in_clause}) \
                     ORDER BY i.created_at DESC"
                );
                let mut query = sqlx::query_as::<_, IncidentRow>(&sql).bind(from).bind(to);
                for id in ids {
                    query = query.bind(id);
                }
                query
                    .fetch_all(&self.pool)
                    .await?
                    .into_iter()
                    .map(StatusIncident::try_from)
                    .collect()
            }
        }
    }

    // -- Batch queries --

    async fn get_updates_for_incidents(
        &self,
        ids: &[Uuid],
    ) -> AppResult<HashMap<Uuid, Vec<StatusIncidentUpdate>>> {
        if ids.is_empty() {
            return Ok(HashMap::new());
        }
        let placeholders: Vec<&str> = ids.iter().map(|_| "?").collect();
        let in_clause = placeholders.join(", ");
        let sql = format!(
            "SELECT id, incident_id, status, message, created_at \
             FROM incident_updates WHERE incident_id IN ({in_clause}) \
             ORDER BY incident_id, created_at ASC"
        );
        let mut query = sqlx::query_as::<_, IncidentUpdateRow>(&sql);
        for id in ids {
            query = query.bind(id.to_string());
        }
        let rows = query.fetch_all(&self.pool).await?;
        let mut map: HashMap<Uuid, Vec<StatusIncidentUpdate>> = HashMap::new();
        for row in rows {
            let update = StatusIncidentUpdate::try_from(row)?;
            map.entry(update.incident_id).or_default().push(update);
        }
        Ok(map)
    }

    async fn get_component_ids_for_incidents(
        &self,
        ids: &[Uuid],
    ) -> AppResult<HashMap<Uuid, Vec<String>>> {
        if ids.is_empty() {
            return Ok(HashMap::new());
        }
        let placeholders: Vec<&str> = ids.iter().map(|_| "?").collect();
        let in_clause = placeholders.join(", ");
        let sql = format!(
            "SELECT incident_id, component_id \
             FROM incident_components WHERE incident_id IN ({in_clause})"
        );
        let mut query = sqlx::query_as::<_, IncidentComponentRow>(&sql);
        for id in ids {
            query = query.bind(id.to_string());
        }
        let rows = query.fetch_all(&self.pool).await?;
        let mut map: HashMap<Uuid, Vec<String>> = HashMap::new();
        for row in rows {
            let incident_id = Uuid::parse_str(&row.incident_id)
                .map_err(|e| AppError::BadRequest(format!("invalid uuid: {e}")))?;
            map.entry(incident_id).or_default().push(row.component_id);
        }
        Ok(map)
    }

    async fn list_active_incidents_with_components(
        &self,
    ) -> AppResult<Vec<(StatusIncident, Vec<String>)>> {
        let rows = sqlx::query_as::<_, IncidentWithComponentsRow>(
            "SELECT i.id, i.title, i.status, i.severity, i.auto_detected, i.created_at, i.resolved_at, \
             i.postmortem_body, i.postmortem_published_at, \
             GROUP_CONCAT(ic.component_id) as component_ids \
             FROM incidents i \
             LEFT JOIN incident_components ic ON ic.incident_id = i.id \
             WHERE i.status != 'resolved' \
             GROUP BY i.id \
             ORDER BY i.created_at DESC",
        )
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|row| {
                let comp_ids: Vec<String> = row
                    .component_ids
                    .as_deref()
                    .filter(|s| !s.is_empty())
                    .map(|s| s.split(',').map(String::from).collect())
                    .unwrap_or_default();
                let incident = StatusIncident::try_from(IncidentRow {
                    id: row.id,
                    title: row.title,
                    status: row.status,
                    severity: row.severity,
                    auto_detected: row.auto_detected,
                    created_at: row.created_at,
                    resolved_at: row.resolved_at,
                    postmortem_body: row.postmortem_body,
                    postmortem_published_at: row.postmortem_published_at,
                })?;
                Ok((incident, comp_ids))
            })
            .collect()
    }

    async fn list_recent_resolved_incidents(&self, days: i64) -> AppResult<Vec<StatusIncident>> {
        sqlx::query_as::<_, IncidentRow>(
            "SELECT id, title, status, severity, auto_detected, created_at, resolved_at, \
             postmortem_body, postmortem_published_at \
             FROM incidents \
             WHERE status = 'resolved' AND resolved_at >= datetime('now', ? || ' days') \
             ORDER BY created_at DESC",
        )
        .bind(-days)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(StatusIncident::try_from)
        .collect()
    }

    // -- Postmortem --

    async fn set_postmortem(&self, id: Uuid, body: &str) -> AppResult<()> {
        sqlx::query(
            "UPDATE incidents SET postmortem_body = ?, postmortem_published_at = datetime('now') WHERE id = ?",
        )
        .bind(body)
        .bind(id.to_string())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    // -- Maintenance --

    async fn create_maintenance(&self, m: &ScheduledMaintenance) -> AppResult<()> {
        let component_ids_json = serde_json::to_string(&m.component_ids)
            .map_err(|e| AppError::BadRequest(format!("failed to serialize component_ids: {e}")))?;
        sqlx::query(
            "INSERT INTO scheduled_maintenances \
             (id, title, description, status, component_ids, scheduled_start, scheduled_end, \
              actual_start, actual_end, auto_suppress_incidents, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(m.id.to_string())
        .bind(&m.title)
        .bind(&m.description)
        .bind(m.status.as_db_str())
        .bind(&component_ids_json)
        .bind(m.scheduled_start.to_rfc3339())
        .bind(m.scheduled_end.to_rfc3339())
        .bind(m.actual_start.map(|dt| dt.to_rfc3339()))
        .bind(m.actual_end.map(|dt| dt.to_rfc3339()))
        .bind(m.auto_suppress_incidents as i32)
        .bind(m.created_at.to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn get_maintenance_by_id(&self, id: Uuid) -> AppResult<Option<ScheduledMaintenance>> {
        sqlx::query_as::<_, MaintenanceRow>(
            "SELECT id, title, description, status, component_ids, \
             scheduled_start, scheduled_end, actual_start, actual_end, \
             auto_suppress_incidents, created_at \
             FROM scheduled_maintenances WHERE id = ?",
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?
        .map(ScheduledMaintenance::try_from)
        .transpose()
    }

    async fn update_maintenance_status(
        &self,
        id: Uuid,
        status: MaintenanceStatus,
        actual_start: Option<DateTime<Utc>>,
        actual_end: Option<DateTime<Utc>>,
    ) -> AppResult<()> {
        sqlx::query(
            "UPDATE scheduled_maintenances SET status = ?, actual_start = COALESCE(?, actual_start), actual_end = COALESCE(?, actual_end) WHERE id = ?",
        )
        .bind(status.as_db_str())
        .bind(actual_start.map(|dt| dt.to_rfc3339()))
        .bind(actual_end.map(|dt| dt.to_rfc3339()))
        .bind(id.to_string())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn list_active_maintenances(&self) -> AppResult<Vec<ScheduledMaintenance>> {
        sqlx::query_as::<_, MaintenanceRow>(
            "SELECT id, title, description, status, component_ids, \
             scheduled_start, scheduled_end, actual_start, actual_end, \
             auto_suppress_incidents, created_at \
             FROM scheduled_maintenances WHERE status = 'in_progress' \
             ORDER BY scheduled_start ASC",
        )
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(ScheduledMaintenance::try_from)
        .collect()
    }

    async fn list_upcoming_maintenances(&self) -> AppResult<Vec<ScheduledMaintenance>> {
        sqlx::query_as::<_, MaintenanceRow>(
            "SELECT id, title, description, status, component_ids, \
             scheduled_start, scheduled_end, actual_start, actual_end, \
             auto_suppress_incidents, created_at \
             FROM scheduled_maintenances WHERE status = 'scheduled' \
             ORDER BY scheduled_start ASC",
        )
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(ScheduledMaintenance::try_from)
        .collect()
    }

    async fn is_component_in_maintenance(&self, component_id: &str) -> AppResult<bool> {
        let rows = sqlx::query_as::<_, MaintenanceRow>(
            "SELECT id, title, description, status, component_ids, \
             scheduled_start, scheduled_end, actual_start, actual_end, \
             auto_suppress_incidents, created_at \
             FROM scheduled_maintenances \
             WHERE status IN ('scheduled', 'in_progress') \
             AND datetime('now') >= datetime(scheduled_start) \
             AND datetime('now') <= datetime(scheduled_end)",
        )
        .fetch_all(&self.pool)
        .await?;

        for row in rows {
            let ids: Vec<String> = serde_json::from_str(&row.component_ids).unwrap_or_else(|e| {
                tracing::warn!(maintenance_id = %row.id, error = %e, "invalid component_ids json");
                Vec::new()
            });
            if ids.iter().any(|id| id == component_id) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    async fn list_all_maintenances(&self) -> AppResult<Vec<ScheduledMaintenance>> {
        sqlx::query_as::<_, MaintenanceRow>(
            "SELECT id, title, description, status, component_ids, \
             scheduled_start, scheduled_end, actual_start, actual_end, \
             auto_suppress_incidents, created_at \
             FROM scheduled_maintenances \
             ORDER BY created_at DESC",
        )
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(ScheduledMaintenance::try_from)
        .collect()
    }

    async fn list_recent_completed_maintenances(
        &self,
        days: i64,
    ) -> AppResult<Vec<ScheduledMaintenance>> {
        let cutoff = (Utc::now() - chrono::Duration::days(days)).to_rfc3339();
        sqlx::query_as::<_, MaintenanceRow>(
            "SELECT id, title, description, status, component_ids, \
             scheduled_start, scheduled_end, actual_start, actual_end, \
             auto_suppress_incidents, created_at \
             FROM scheduled_maintenances \
             WHERE status = 'completed' AND scheduled_end >= ? \
             ORDER BY scheduled_end DESC",
        )
        .bind(&cutoff)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(ScheduledMaintenance::try_from)
        .collect()
    }

    async fn update_maintenance_scheduled_end(
        &self,
        id: Uuid,
        scheduled_end: DateTime<Utc>,
    ) -> AppResult<()> {
        sqlx::query("UPDATE scheduled_maintenances SET scheduled_end = ? WHERE id = ?")
            .bind(scheduled_end.to_rfc3339())
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
