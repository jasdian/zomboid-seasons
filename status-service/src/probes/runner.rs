use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::config::{ComponentConfig, ProbeConfig};
use crate::db::{SqliteRepo, StatusRepo};
use crate::domain::{
    IncidentSeverity, IncidentStatus, StatusIncident, StatusIncidentUpdate, UpdateStatus,
};
use crate::notifications::{NotificationEvent, NotificationSender};

use super::http::HttpProbe;
use super::json_rpc::JsonRpcProbe;
use super::rcon::RconProbe;
use super::tcp::TcpProbe;
use super::Probe;

/// Spawn one probe task per component that has a probe config.
pub fn spawn_probe_tasks(
    components: &[ComponentConfig],
    repo: SqliteRepo,
    reqwest_client: reqwest::Client,
    notification_sender: NotificationSender,
) -> Vec<JoinHandle<()>> {
    let mut handles = Vec::new();

    for comp in components {
        let Some(ref probe_config) = comp.probe else {
            continue;
        };

        let probe: Box<dyn Probe> = match probe_config {
            ProbeConfig::Http(c) => Box::new(HttpProbe::new(
                reqwest_client.clone(),
                c.url.clone(),
                c.method.clone(),
                c.expected_status,
                c.common.timeout(),
            )),
            ProbeConfig::Tcp(c) => {
                Box::new(TcpProbe::new(c.host.clone(), c.port, c.common.timeout()))
            }
            ProbeConfig::Rcon(c) => {
                let password = match c.resolve_password() {
                    Ok(pw) => pw,
                    Err(e) => {
                        tracing::error!(component = %comp.id, error = %e, "skipping rcon probe");
                        continue;
                    }
                };
                Box::new(RconProbe::new(
                    c.host.clone(),
                    c.port,
                    password,
                    c.command.clone(),
                    c.common.timeout(),
                ))
            }
            ProbeConfig::JsonRpc(c) => Box::new(JsonRpcProbe::new(
                reqwest_client.clone(),
                c.url.clone(),
                c.method.clone(),
                c.common.timeout(),
            )),
        };

        let common = probe_config.common().clone();
        let component_id = comp.id.clone();
        let component_name = comp.name.clone();
        let repo = repo.clone();
        let notification_sender = notification_sender.clone();

        let handle = tokio::spawn(async move {
            probe_loop(probe, &component_id, &component_name, &common, repo, notification_sender)
                .await;
        });

        handles.push(handle);
    }

    handles
}

async fn probe_loop(
    probe: Box<dyn Probe>,
    component_id: &str,
    component_name: &str,
    common: &crate::config::ProbeCommonConfig,
    repo: SqliteRepo,
    notification_sender: NotificationSender,
) {
    let interval = common.interval();
    let failure_threshold = common.failure_threshold;
    let recovery_threshold = common.recovery_threshold;

    let mut consecutive_failures: u32 = 0;
    let mut consecutive_successes: u32 = 0;

    // Crash recovery: check if there's an existing unresolved auto-incident
    let mut active_auto_incident_id: Option<Uuid> =
        match repo.find_active_auto_incident(component_id).await {
            Ok(id) => {
                if id.is_some() {
                    tracing::info!(
                        component = %component_id,
                        incident_id = ?id,
                        "recovered active auto-incident from db"
                    );
                }
                id
            }
            Err(e) => {
                tracing::error!(
                    component = %component_id,
                    error = %e,
                    "failed to check for active auto-incident"
                );
                None
            }
        };

    loop {
        tokio::time::sleep(interval).await;

        let result = probe.check().await;

        // Persist probe result
        if let Err(e) = repo
            .insert_probe_result(
                component_id,
                result.success,
                result.response_ms,
                result.error_message.as_deref(),
            )
            .await
        {
            tracing::error!(
                component = %component_id,
                error = %e,
                "failed to insert probe result"
            );
        }

        if result.success {
            consecutive_failures = 0;
            consecutive_successes += 1;

            // Auto-resolve if recovery threshold met and there's an active auto-incident
            if consecutive_successes >= recovery_threshold {
                if let Some(incident_id) = active_auto_incident_id {
                    auto_resolve_incident(&repo, incident_id, component_id).await;
                    active_auto_incident_id = None;
                    notification_sender
                        .send(NotificationEvent::IncidentResolved { incident_id });
                }
            }
        } else {
            consecutive_successes = 0;
            consecutive_failures += 1;

            if consecutive_failures >= failure_threshold && active_auto_incident_id.is_none() {
                // Check if component is in maintenance — suppress auto-creation
                let in_maintenance = repo
                    .is_component_in_maintenance(component_id)
                    .await
                    .unwrap_or(false);

                if !in_maintenance {
                    let severity = escalate_severity(consecutive_failures, failure_threshold);
                    active_auto_incident_id =
                        auto_create_incident(&repo, component_id, component_name, severity).await;
                    if let Some(incident_id) = active_auto_incident_id {
                        notification_sender
                            .send(NotificationEvent::IncidentCreated { incident_id });
                    }
                }
            } else if let Some(incident_id) = active_auto_incident_id {
                // Escalate severity if failures keep increasing
                let new_severity = escalate_severity(consecutive_failures, failure_threshold);
                if let Err(e) = repo
                    .update_incident_severity(incident_id, new_severity)
                    .await
                {
                    tracing::error!(
                        component = %component_id,
                        incident_id = %incident_id,
                        error = %e,
                        "failed to escalate incident severity"
                    );
                } else {
                    notification_sender.send(
                        NotificationEvent::IncidentSeverityEscalated {
                            incident_id,
                            new_severity,
                        },
                    );
                }
            }
        }
    }
}

fn escalate_severity(failures: u32, threshold: u32) -> IncidentSeverity {
    if failures >= threshold * 4 {
        IncidentSeverity::Critical
    } else if failures >= threshold * 2 {
        IncidentSeverity::Major
    } else {
        IncidentSeverity::Minor
    }
}

async fn auto_create_incident(
    repo: &SqliteRepo,
    component_id: &str,
    component_name: &str,
    severity: IncidentSeverity,
) -> Option<Uuid> {
    let now = chrono::Utc::now();
    let incident_id = Uuid::new_v4();

    let incident = StatusIncident {
        id: incident_id,
        title: format!("{component_name} health check failing"),
        status: IncidentStatus::Investigating,
        severity,
        auto_detected: true,
        created_at: now,
        resolved_at: None,
        postmortem_body: None,
        postmortem_published_at: None,
    };

    if let Err(e) = repo.create_incident(&incident).await {
        tracing::error!(
            component = %component_id,
            error = %e,
            "failed to auto-create incident"
        );
        return None;
    }

    if let Err(e) = repo
        .link_incident_components(incident_id, &[component_id.to_string()])
        .await
    {
        tracing::error!(
            component = %component_id,
            error = %e,
            "failed to link auto-incident to component"
        );
    }

    // Add initial update
    let update = StatusIncidentUpdate {
        id: Uuid::new_v4(),
        incident_id,
        status: UpdateStatus::Investigating,
        message: format!(
            "Auto-detected: {component_name} health check failures exceeded threshold"
        ),
        created_at: now,
    };
    if let Err(e) = repo.add_incident_update(&update).await {
        tracing::error!(error = %e, "failed to add auto-incident update");
    }

    tracing::warn!(
        component = %component_id,
        incident_id = %incident_id,
        severity = %severity,
        "auto_incident_created"
    );

    Some(incident_id)
}

async fn auto_resolve_incident(repo: &SqliteRepo, incident_id: Uuid, component_id: &str) {
    // Check if incident was manually modified (auto_detected set to false means admin took over)
    match repo.get_incident_by_id(incident_id).await {
        Ok(Some(incident)) if !incident.auto_detected => {
            tracing::info!(
                component = %component_id,
                incident_id = %incident_id,
                "skipping auto-resolve: incident was manually modified"
            );
            return;
        }
        Ok(None) => return, // already deleted
        Err(e) => {
            tracing::error!(error = %e, "failed to check incident for auto-resolve");
            return;
        }
        _ => {}
    }

    let now = chrono::Utc::now();

    // Add resolved update
    let update = StatusIncidentUpdate {
        id: Uuid::new_v4(),
        incident_id,
        status: UpdateStatus::Resolved,
        message: "Auto-resolved: health check recovered".into(),
        created_at: now,
    };
    if let Err(e) = repo.add_incident_update(&update).await {
        tracing::error!(error = %e, "failed to add auto-resolve update");
    }

    if let Err(e) = repo
        .update_incident_status(incident_id, IncidentStatus::Resolved, Some(now))
        .await
    {
        tracing::error!(
            component = %component_id,
            incident_id = %incident_id,
            error = %e,
            "failed to auto-resolve incident"
        );
        return;
    }

    tracing::info!(
        component = %component_id,
        incident_id = %incident_id,
        "auto_incident_resolved"
    );
}
