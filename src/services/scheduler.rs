use std::sync::Arc;

use chrono::{Datelike, Timelike};

use crate::config::AppConfig;
use crate::db::SqliteRepo;
use crate::services::rcon::RconConfig;
use crate::services::rotation;

pub fn spawn_scheduler(
    repo: SqliteRepo,
    config: Arc<AppConfig>,
    rcon_config: Arc<RconConfig>,
) -> tokio::task::JoinHandle<()> {
    let target_month = config.schedule.rotation_month;
    let target_day = config.schedule.rotation_day_of_month;
    let target_hour = config.schedule.rotation_hour;

    tracing::info!(
        rotation_month = target_month,
        rotation_day = target_day,
        rotation_hour = target_hour,
        "scheduler_started"
    );

    tokio::spawn(async move {
        let mut last_rotation_date: Option<chrono::NaiveDate> = None;

        loop {
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;

            let now = chrono::Utc::now();
            let today = now.date_naive();

            if now.month() == u32::from(target_month)
                && now.day() == u32::from(target_day)
                && now.hour() == u32::from(target_hour)
                && last_rotation_date != Some(today)
            {
                tracing::info!("scheduler_triggering_rotation");
                last_rotation_date = Some(today);

                match rotation::rotate_season(&repo, &config, &rcon_config, false).await {
                    Ok(()) => {
                        tracing::info!("scheduler_rotation_success");
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "scheduler_rotation_failed");
                    }
                }
            }
        }
    })
}
