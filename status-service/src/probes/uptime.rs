use std::time::Duration;

use tokio::task::JoinHandle;

use crate::db::{SqliteRepo, StatusRepo};

pub fn spawn_uptime_aggregator(repo: SqliteRepo) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(300)).await;
            if let Err(e) = repo.aggregate_daily_uptime().await {
                tracing::error!(error = %e, "uptime_aggregation_failed");
            }
        }
    })
}
