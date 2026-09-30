pub mod http;
pub mod json_rpc;
pub mod maintenance;
pub mod rcon;
pub mod runner;
pub mod tcp;
pub mod uptime;

pub use maintenance::spawn_maintenance_watcher;
pub use runner::spawn_probe_tasks;
pub use uptime::spawn_uptime_aggregator;

use std::future::Future;
use std::pin::Pin;

use tokio::task::JoinHandle;

use crate::db::{SqliteRepo, StatusRepo};

pub struct ProbeResult {
    pub success: bool,
    pub response_ms: Option<i64>,
    pub error_message: Option<String>,
}

pub trait Probe: Send + Sync {
    fn check(&self) -> Pin<Box<dyn Future<Output = ProbeResult> + Send + '_>>;
}

/// Spawn a background task that prunes probe results older than 7 days every hour.
pub fn spawn_probe_cleanup(repo: SqliteRepo) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
            if let Err(e) = repo.delete_old_probe_results(7).await {
                tracing::error!(error = %e, "probe_cleanup_failed");
            }
        }
    })
}
