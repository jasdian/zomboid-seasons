pub mod api;
pub mod config;
pub mod db;
pub mod domain;
pub mod error;
pub mod notifications;
pub mod probes;
pub mod rate_limit;

use std::path::PathBuf;
use std::sync::Arc;

use crate::config::StatusConfig;
use crate::db::SqliteRepo;
use crate::notifications::NotificationSender;

#[derive(Clone)]
pub struct AppState {
    pub repo: SqliteRepo,
    pub admin_token: Arc<str>,
    pub static_dir: PathBuf,
    pub config: Arc<StatusConfig>,
    pub notification_sender: NotificationSender,
}
