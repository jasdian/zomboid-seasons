use std::time::Duration;

use tokio::task::JoinHandle;

use crate::db::{SqliteRepo, StatusRepo};
use crate::domain::{ComponentStatus, MaintenanceStatus, ScheduledMaintenance};
use crate::notifications::{NotificationEvent, NotificationSender};

pub fn spawn_maintenance_watcher(
    repo: SqliteRepo,
    notification_sender: NotificationSender,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
            let now = chrono::Utc::now();

            // Transition scheduled -> in_progress when scheduled_start has passed
            match repo.list_upcoming_maintenances().await {
                Ok(upcoming) => {
                    for m in &upcoming {
                        transition_to_in_progress(&repo, m, now, &notification_sender).await;
                    }
                }
                Err(e) => {
                    tracing::error!(error = %e, "failed to list upcoming maintenances");
                }
            }

            // Transition in_progress -> completed when scheduled_end has passed
            match repo.list_active_maintenances().await {
                Ok(active) => {
                    for m in &active {
                        transition_to_completed(&repo, m, now, &notification_sender).await;
                    }
                }
                Err(e) => {
                    tracing::error!(error = %e, "failed to list active maintenances");
                }
            }
        }
    })
}

async fn transition_to_in_progress(
    repo: &SqliteRepo,
    m: &ScheduledMaintenance,
    now: chrono::DateTime<chrono::Utc>,
    notification_sender: &NotificationSender,
) {
    if m.scheduled_start > now {
        return;
    }

    tracing::info!(
        maintenance_id = %m.id,
        title = %m.title,
        "transitioning maintenance to in_progress"
    );

    if let Err(e) = repo
        .update_maintenance_status(m.id, MaintenanceStatus::InProgress, Some(now), None)
        .await
    {
        tracing::error!(error = %e, "failed to transition maintenance");
        return;
    }

    // Set component status_override to maintenance
    for cid in &m.component_ids {
        if let Err(e) = repo
            .set_component_override(cid, Some(ComponentStatus::Maintenance))
            .await
        {
            tracing::error!(component = %cid, error = %e, "failed to set maintenance override");
        }
    }

    notification_sender.send(NotificationEvent::MaintenanceStarted {
        maintenance_id: m.id,
    });
}

async fn transition_to_completed(
    repo: &SqliteRepo,
    m: &ScheduledMaintenance,
    now: chrono::DateTime<chrono::Utc>,
    notification_sender: &NotificationSender,
) {
    if m.scheduled_end > now {
        return;
    }

    tracing::info!(
        maintenance_id = %m.id,
        title = %m.title,
        "transitioning maintenance to completed"
    );

    if let Err(e) = repo
        .update_maintenance_status(
            m.id,
            MaintenanceStatus::Completed,
            m.actual_start,
            Some(now),
        )
        .await
    {
        tracing::error!(error = %e, "failed to complete maintenance");
        return;
    }

    // Clear component status_override
    for cid in &m.component_ids {
        if let Err(e) = repo.set_component_override(cid, None).await {
            tracing::error!(component = %cid, error = %e, "failed to clear maintenance override");
        }
    }

    notification_sender.send(NotificationEvent::MaintenanceCompleted {
        maintenance_id: m.id,
    });
}
