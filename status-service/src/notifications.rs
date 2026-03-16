use std::collections::HashMap;

use chrono::Utc;
use reqwest::Client;
use serde_json::{json, Value};
use tokio::sync::mpsc;
use tokio::time::Instant;
use uuid::Uuid;

use crate::config::{expand_env_vars, StatusConfig, SubscriberConfig};
use crate::db::{SqliteRepo, StatusRepo};
use crate::domain::{IncidentSeverity, ScheduledMaintenance, StatusIncident};

// ---------------------------------------------------------------------------
// Event type
// ---------------------------------------------------------------------------

pub enum NotificationEvent {
    IncidentCreated { incident_id: Uuid },
    IncidentUpdated { incident_id: Uuid, message: String },
    IncidentResolved { incident_id: Uuid },
    IncidentSeverityEscalated { incident_id: Uuid, new_severity: IncidentSeverity },
    PostmortemPublished { incident_id: Uuid },
    MaintenanceScheduled { maintenance_id: Uuid },
    MaintenanceStarted { maintenance_id: Uuid },
    MaintenanceCompleted { maintenance_id: Uuid },
}

impl NotificationEvent {
    fn event_name(&self) -> &'static str {
        match self {
            NotificationEvent::IncidentCreated { .. } => "incident.created",
            NotificationEvent::IncidentUpdated { .. } => "incident.updated",
            NotificationEvent::IncidentResolved { .. } => "incident.resolved",
            NotificationEvent::IncidentSeverityEscalated { .. } => "incident.severity_escalated",
            NotificationEvent::PostmortemPublished { .. } => "postmortem.published",
            NotificationEvent::MaintenanceScheduled { .. } => "maintenance.scheduled",
            NotificationEvent::MaintenanceStarted { .. } => "maintenance.started",
            NotificationEvent::MaintenanceCompleted { .. } => "maintenance.completed",
        }
    }
}

// ---------------------------------------------------------------------------
// Sender (fire-and-forget handle)
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct NotificationSender {
    tx: mpsc::Sender<NotificationEvent>,
}

impl NotificationSender {
    /// Queue an event for delivery. Never blocks; silently drops if the
    /// channel is full (events are best-effort).
    pub fn send(&self, event: NotificationEvent) {
        let _ = self.tx.try_send(event);
    }
}

// ---------------------------------------------------------------------------
// Spawn
// ---------------------------------------------------------------------------

pub fn spawn(
    config_path: String,
    repo: SqliteRepo,
    client: Client,
    queue_capacity: usize,
) -> NotificationSender {
    let (tx, rx) = mpsc::channel(queue_capacity);
    tokio::spawn(worker(config_path, repo, client, rx));
    NotificationSender { tx }
}

// ---------------------------------------------------------------------------
// Worker
// ---------------------------------------------------------------------------

async fn worker(
    config_path: String,
    repo: SqliteRepo,
    client: Client,
    mut rx: mpsc::Receiver<NotificationEvent>,
) {
    let mut rate_limits: HashMap<String, Instant> = HashMap::new();

    while let Some(event) = rx.recv().await {
        let subscribers = match load_subscribers(&config_path) {
            Ok(subs) => subs,
            Err(e) => {
                tracing::error!(error = %e, "failed to load notification subscribers");
                continue;
            }
        };

        if subscribers.is_empty() {
            continue;
        }

        let (discord_payload, webhook_payload) = match build_payloads(&event, &repo).await {
            Ok(payloads) => payloads,
            Err(e) => {
                tracing::error!(error = %e, event = event.event_name(), "failed to build notification payloads");
                continue;
            }
        };

        for sub in &subscribers {
            // Rate-limit: 400ms minimum gap per endpoint
            if let Some(last) = rate_limits.get(&sub.endpoint) {
                let elapsed = last.elapsed();
                if elapsed < std::time::Duration::from_millis(400) {
                    tokio::time::sleep(std::time::Duration::from_millis(400) - elapsed).await;
                }
            }

            let payload = match sub.subscriber_type.as_str() {
                "discord" => &discord_payload,
                _ => &webhook_payload,
            };

            match send_with_retry(&client, &sub.endpoint, payload, 3).await {
                Ok(status) => {
                    if status == 404 {
                        tracing::error!(
                            endpoint = %sub.endpoint,
                            "discord webhook returned 404 — endpoint may be invalid"
                        );
                    } else {
                        tracing::debug!(
                            endpoint = %sub.endpoint,
                            status,
                            event = event.event_name(),
                            "notification delivered"
                        );
                    }
                }
                Err(e) => {
                    tracing::error!(
                        endpoint = %sub.endpoint,
                        error = %e,
                        event = event.event_name(),
                        "notification delivery failed"
                    );
                }
            }

            rate_limits.insert(sub.endpoint.clone(), Instant::now());
        }
    }

    tracing::info!("notification worker stopped");
}

// ---------------------------------------------------------------------------
// Config hot-reload
// ---------------------------------------------------------------------------

fn load_subscribers(config_path: &str) -> Result<Vec<SubscriberConfig>, String> {
    let raw = std::fs::read_to_string(config_path)
        .map_err(|e| format!("cannot read {config_path}: {e}"))?;
    let content = expand_env_vars(&raw);
    let config: StatusConfig =
        toml::from_str(&content).map_err(|e| format!("invalid config: {e}"))?;
    if !config.notifications.enabled {
        return Ok(Vec::new());
    }
    Ok(config.notifications.subscribers)
}

// ---------------------------------------------------------------------------
// Payload building
// ---------------------------------------------------------------------------

async fn build_payloads(
    event: &NotificationEvent,
    repo: &SqliteRepo,
) -> Result<(Value, Value), String> {
    match event {
        NotificationEvent::IncidentCreated { incident_id } => {
            let incident = get_incident(repo, *incident_id).await?;
            let components = get_component_ids(repo, *incident_id).await?;
            let discord = json!({ "embeds": [format_incident_created(&incident, &components)] });
            let webhook = json!({
                "event": "incident.created",
                "timestamp": Utc::now().to_rfc3339(),
                "data": incident_data(&incident, Some(&components)),
            });
            Ok((discord, webhook))
        }
        NotificationEvent::IncidentUpdated { incident_id, message } => {
            let incident = get_incident(repo, *incident_id).await?;
            let discord = json!({ "embeds": [format_incident_updated(&incident, message)] });
            let webhook = json!({
                "event": "incident.updated",
                "timestamp": Utc::now().to_rfc3339(),
                "data": {
                    "incident": incident_data(&incident, None),
                    "message": message,
                },
            });
            Ok((discord, webhook))
        }
        NotificationEvent::IncidentResolved { incident_id } => {
            let incident = get_incident(repo, *incident_id).await?;
            let components = get_component_ids(repo, *incident_id).await?;
            let duration = incident
                .resolved_at
                .map(|r| r.signed_duration_since(incident.created_at));
            let discord = json!({ "embeds": [format_incident_resolved(&incident, &components, duration)] });
            let webhook = json!({
                "event": "incident.resolved",
                "timestamp": Utc::now().to_rfc3339(),
                "data": incident_data(&incident, Some(&components)),
            });
            Ok((discord, webhook))
        }
        NotificationEvent::IncidentSeverityEscalated { incident_id, .. } => {
            let incident = get_incident(repo, *incident_id).await?;
            let discord = json!({ "embeds": [format_severity_escalated(&incident)] });
            let webhook = json!({
                "event": "incident.severity_escalated",
                "timestamp": Utc::now().to_rfc3339(),
                "data": incident_data(&incident, None),
            });
            Ok((discord, webhook))
        }
        NotificationEvent::PostmortemPublished { incident_id } => {
            let incident = get_incident(repo, *incident_id).await?;
            let discord = json!({ "embeds": [format_postmortem(&incident)] });
            let webhook = json!({
                "event": "postmortem.published",
                "timestamp": Utc::now().to_rfc3339(),
                "data": incident_data(&incident, None),
            });
            Ok((discord, webhook))
        }
        NotificationEvent::MaintenanceScheduled { maintenance_id } => {
            let m = get_maintenance(repo, *maintenance_id).await?;
            let discord = json!({ "embeds": [format_maintenance(&m, "Scheduled")] });
            let webhook = json!({
                "event": "maintenance.scheduled",
                "timestamp": Utc::now().to_rfc3339(),
                "data": maintenance_data(&m),
            });
            Ok((discord, webhook))
        }
        NotificationEvent::MaintenanceStarted { maintenance_id } => {
            let m = get_maintenance(repo, *maintenance_id).await?;
            let discord = json!({ "embeds": [format_maintenance(&m, "Started")] });
            let webhook = json!({
                "event": "maintenance.started",
                "timestamp": Utc::now().to_rfc3339(),
                "data": maintenance_data(&m),
            });
            Ok((discord, webhook))
        }
        NotificationEvent::MaintenanceCompleted { maintenance_id } => {
            let m = get_maintenance(repo, *maintenance_id).await?;
            let discord = json!({ "embeds": [format_maintenance(&m, "Completed")] });
            let webhook = json!({
                "event": "maintenance.completed",
                "timestamp": Utc::now().to_rfc3339(),
                "data": maintenance_data(&m),
            });
            Ok((discord, webhook))
        }
    }
}

// ---------------------------------------------------------------------------
// DB helpers (map AppError to String for the notification context)
// ---------------------------------------------------------------------------

async fn get_incident(repo: &SqliteRepo, id: Uuid) -> Result<StatusIncident, String> {
    repo.get_incident_by_id(id)
        .await
        .map_err(|e| format!("db error fetching incident {id}: {e}"))?
        .ok_or_else(|| format!("incident {id} not found"))
}

async fn get_component_ids(repo: &SqliteRepo, incident_id: Uuid) -> Result<Vec<String>, String> {
    repo.get_component_ids_for_incident(incident_id)
        .await
        .map_err(|e| format!("db error fetching components for incident {incident_id}: {e}"))
}

async fn get_maintenance(repo: &SqliteRepo, id: Uuid) -> Result<ScheduledMaintenance, String> {
    repo.get_maintenance_by_id(id)
        .await
        .map_err(|e| format!("db error fetching maintenance {id}: {e}"))?
        .ok_or_else(|| format!("maintenance {id} not found"))
}

// ---------------------------------------------------------------------------
// JSON data helpers for webhook payloads
// ---------------------------------------------------------------------------

fn incident_data(incident: &StatusIncident, components: Option<&[String]>) -> Value {
    let mut data = json!({
        "id": incident.id.to_string(),
        "title": incident.title,
        "status": incident.status.as_db_str(),
        "severity": incident.severity.as_db_str(),
        "created_at": incident.created_at.to_rfc3339(),
    });
    if let Some(resolved) = incident.resolved_at {
        data["resolved_at"] = json!(resolved.to_rfc3339());
    }
    if let Some(comps) = components {
        data["component_ids"] = json!(comps);
    }
    data
}

fn maintenance_data(m: &ScheduledMaintenance) -> Value {
    json!({
        "id": m.id.to_string(),
        "title": m.title,
        "description": m.description,
        "status": m.status.as_db_str(),
        "component_ids": m.component_ids,
        "scheduled_start": m.scheduled_start.to_rfc3339(),
        "scheduled_end": m.scheduled_end.to_rfc3339(),
    })
}

// ---------------------------------------------------------------------------
// Discord embed formatters
// ---------------------------------------------------------------------------

fn severity_color(severity: IncidentSeverity) -> u32 {
    match severity {
        IncidentSeverity::Critical => 0xE04040,
        IncidentSeverity::Major => 0xF0A020,
        IncidentSeverity::Minor => 0xFFFF00,
    }
}

fn severity_emoji(severity: IncidentSeverity) -> &'static str {
    match severity {
        IncidentSeverity::Critical => "\u{1F534}",  // red circle
        IncidentSeverity::Major => "\u{1F7E0}",     // orange circle
        IncidentSeverity::Minor => "\u{1F7E1}",     // yellow circle
    }
}

fn format_incident_created(incident: &StatusIncident, components: &[String]) -> Value {
    let mut fields = vec![
        json!({ "name": "Severity", "value": incident.severity.as_db_str(), "inline": true }),
        json!({ "name": "Status", "value": incident.status.as_db_str(), "inline": true }),
    ];
    if !components.is_empty() {
        fields.push(json!({
            "name": "Components",
            "value": components.join(", "),
            "inline": false,
        }));
    }
    json!({
        "title": format!("{} Incident: {}", severity_emoji(incident.severity), incident.title),
        "color": severity_color(incident.severity),
        "fields": fields,
        "footer": { "text": incident.created_at.to_rfc3339() },
    })
}

fn format_incident_updated(incident: &StatusIncident, message: &str) -> Value {
    json!({
        "title": format!("\u{1F4CB} Update: {}", incident.title),
        "color": 0x3B82F6,
        "fields": [
            { "name": "Status", "value": incident.status.as_db_str(), "inline": true },
            { "name": "Message", "value": truncate(message, 1024), "inline": false },
        ],
        "footer": { "text": Utc::now().to_rfc3339() },
    })
}

fn format_incident_resolved(
    incident: &StatusIncident,
    components: &[String],
    duration: Option<chrono::Duration>,
) -> Value {
    let mut fields = Vec::new();
    if let Some(d) = duration {
        fields.push(json!({
            "name": "Duration", "value": format_duration(d), "inline": true
        }));
    }
    if !components.is_empty() {
        fields.push(json!({
            "name": "Components", "value": components.join(", "), "inline": false
        }));
    }
    json!({
        "title": format!("\u{2705} Resolved: {}", incident.title),
        "color": 0x48C78E,
        "fields": fields,
        "footer": { "text": Utc::now().to_rfc3339() },
    })
}

fn format_severity_escalated(incident: &StatusIncident) -> Value {
    json!({
        "title": format!("\u{26A0}\u{FE0F} Escalated: {}", incident.title),
        "color": 0xE04040,
        "fields": [
            { "name": "New Severity", "value": incident.severity.as_db_str(), "inline": true },
        ],
        "footer": { "text": Utc::now().to_rfc3339() },
    })
}

fn format_postmortem(incident: &StatusIncident) -> Value {
    let description = incident
        .postmortem_body
        .as_deref()
        .map(|b| truncate(b, 2048))
        .unwrap_or_default();
    json!({
        "title": format!("\u{1F4DD} Postmortem: {}", incident.title),
        "color": 0x2D6A4F,
        "description": description,
        "footer": { "text": Utc::now().to_rfc3339() },
    })
}

fn format_maintenance(m: &ScheduledMaintenance, event_type: &str) -> Value {
    let mut fields = vec![
        json!({ "name": "Start", "value": m.scheduled_start.to_rfc3339(), "inline": true }),
        json!({ "name": "End", "value": m.scheduled_end.to_rfc3339(), "inline": true }),
    ];
    if !m.component_ids.is_empty() {
        fields.push(json!({
            "name": "Components",
            "value": m.component_ids.join(", "),
            "inline": false,
        }));
    }
    json!({
        "title": format!("\u{1F527} {event_type}: {}", m.title),
        "color": 0x60A5FA,
        "fields": fields,
        "footer": { "text": Utc::now().to_rfc3339() },
    })
}

// ---------------------------------------------------------------------------
// HTTP delivery with retry
// ---------------------------------------------------------------------------

async fn send_with_retry(
    client: &Client,
    url: &str,
    payload: &Value,
    max_attempts: u32,
) -> Result<u16, String> {
    let mut delay = std::time::Duration::from_secs(1);

    for attempt in 1..=max_attempts {
        let result = client.post(url).json(payload).send().await;

        match result {
            Ok(resp) => {
                let status = resp.status().as_u16();
                if (200..300).contains(&status) {
                    return Ok(status);
                }
                if status == 404 {
                    return Ok(404);
                }
                // Retry on 429 (rate limited) and 5xx (server error)
                if (status == 429 || status >= 500) && attempt < max_attempts {
                    tracing::warn!(
                        url,
                        status,
                        attempt,
                        "notification delivery failed, retrying"
                    );
                    tokio::time::sleep(delay).await;
                    delay *= 2;
                    continue;
                }
                return Err(format!("HTTP {status} from {url} after {attempt} attempts"));
            }
            Err(e) => {
                if attempt < max_attempts {
                    tracing::warn!(url, error = %e, attempt, "notification request failed, retrying");
                    tokio::time::sleep(delay).await;
                    delay *= 2;
                    continue;
                }
                return Err(format!("request to {url} failed after {max_attempts} attempts: {e}"));
            }
        }
    }

    Err(format!("exhausted {max_attempts} attempts for {url}"))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    let truncated: String = s.chars().take(max_chars.saturating_sub(3)).collect();
    format!("{truncated}...")
}

fn format_duration(d: chrono::Duration) -> String {
    let total_minutes = d.num_minutes();
    if total_minutes < 1 {
        return format!("{}s", d.num_seconds());
    }
    let hours = total_minutes / 60;
    let minutes = total_minutes % 60;
    if hours > 0 {
        format!("{hours}h {minutes}m")
    } else {
        format!("{minutes}m")
    }
}
