use std::time::Duration;

use serde::Deserialize;

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Deserialize)]
pub struct StatusConfig {
    pub server: ServerConfig,
    pub admin: AdminConfig,
    pub database: DatabaseConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub component_groups: Vec<ComponentGroupConfig>,
    #[serde(default)]
    pub components: Vec<ComponentConfig>,
    #[serde(default)]
    pub notifications: NotificationsConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct NotificationsConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_queue_capacity")]
    pub queue_capacity: usize,
    #[serde(default)]
    pub subscribers: Vec<SubscriberConfig>,
}

fn default_queue_capacity() -> usize {
    64
}

#[derive(Debug, Clone, Deserialize)]
pub struct SubscriberConfig {
    #[serde(rename = "type")]
    pub subscriber_type: String,
    pub endpoint: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub bind: String,
    pub static_dir: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AdminConfig {
    pub token: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct LoggingConfig {
    #[serde(default = "default_level")]
    pub level: String,
    #[serde(default = "default_format")]
    pub format: String,
}

fn default_level() -> String {
    "info".into()
}
fn default_format() -> String {
    "json".into()
}

#[derive(Debug, Clone, Deserialize)]
pub struct ComponentGroupConfig {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub position: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ComponentConfig {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub position: i64,
    #[serde(default)]
    pub group_id: Option<String>,
    pub probe: Option<ProbeConfig>,
}

// ---------------------------------------------------------------------------
// Probe configuration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProbeConfig {
    Http(HttpProbeConfig),
    Tcp(TcpProbeConfig),
    Rcon(RconProbeConfig),
    JsonRpc(JsonRpcProbeConfig),
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProbeCommonConfig {
    #[serde(default = "default_interval_seconds")]
    pub interval_seconds: u64,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default = "default_failure_threshold")]
    pub failure_threshold: u32,
    #[serde(default = "default_recovery_threshold")]
    pub recovery_threshold: u32,
}

fn default_interval_seconds() -> u64 {
    30
}
fn default_timeout_ms() -> u64 {
    5000
}
fn default_failure_threshold() -> u32 {
    3
}
fn default_recovery_threshold() -> u32 {
    2
}

impl ProbeCommonConfig {
    pub fn timeout(&self) -> Duration {
        Duration::from_millis(self.timeout_ms)
    }

    pub fn interval(&self) -> Duration {
        Duration::from_secs(self.interval_seconds)
    }
}

impl ProbeConfig {
    pub fn common(&self) -> &ProbeCommonConfig {
        match self {
            ProbeConfig::Http(c) => &c.common,
            ProbeConfig::Tcp(c) => &c.common,
            ProbeConfig::Rcon(c) => &c.common,
            ProbeConfig::JsonRpc(c) => &c.common,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct HttpProbeConfig {
    pub url: String,
    #[serde(default = "default_http_method")]
    pub method: String,
    #[serde(default = "default_expected_status")]
    pub expected_status: u16,
    #[serde(flatten)]
    pub common: ProbeCommonConfig,
}

fn default_http_method() -> String {
    "GET".into()
}
fn default_expected_status() -> u16 {
    200
}

#[derive(Debug, Clone, Deserialize)]
pub struct TcpProbeConfig {
    pub host: String,
    pub port: u16,
    #[serde(flatten)]
    pub common: ProbeCommonConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RconProbeConfig {
    pub host: String,
    pub port: u16,
    /// Inline password. Mutually exclusive with `password_file`.
    #[serde(default)]
    pub password: Option<String>,
    /// Path to file containing the password (trimmed on read). Takes precedence over `password`.
    pub password_file: Option<String>,
    #[serde(default = "default_rcon_command")]
    pub command: String,
    #[serde(flatten)]
    pub common: ProbeCommonConfig,
}

impl RconProbeConfig {
    /// Resolve the RCON password from `password_file` (if set) or `password`.
    pub fn resolve_password(&self) -> AppResult<String> {
        if let Some(ref path) = self.password_file {
            std::fs::read_to_string(path)
                .map(|s| s.trim().to_string())
                .map_err(|e| {
                    AppError::Config(format!("cannot read rcon password file {path}: {e}"))
                })
        } else if let Some(ref pw) = self.password {
            Ok(pw.clone())
        } else {
            Err(AppError::Config(
                "rcon probe requires either 'password' or 'password_file'".into(),
            ))
        }
    }
}

fn default_rcon_command() -> String {
    "players".into()
}

#[derive(Debug, Clone, Deserialize)]
pub struct JsonRpcProbeConfig {
    pub url: String,
    pub method: String,
    #[serde(flatten)]
    pub common: ProbeCommonConfig,
}

impl StatusConfig {
    pub fn from_file(path: &str) -> AppResult<Self> {
        let raw = std::fs::read_to_string(path)
            .map_err(|e| AppError::Config(format!("cannot read {path}: {e}")))?;
        let content = expand_env_vars(&raw);
        toml::from_str(&content).map_err(|e| AppError::Config(format!("invalid config: {e}")))
    }
}

/// Replace `${VAR_NAME}` patterns with the corresponding environment variable value.
/// Unknown variables are replaced with an empty string.
pub fn expand_env_vars(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '$' && chars.peek() == Some(&'{') {
            chars.next(); // consume '{'
            let mut var_name = String::new();
            for ch in chars.by_ref() {
                if ch == '}' {
                    break;
                }
                var_name.push(ch);
            }
            match std::env::var(&var_name) {
                Ok(val) => result.push_str(&val),
                Err(_) => tracing::warn!(var = %var_name, "env var not found, using empty string"),
            }
        } else {
            result.push(c);
        }
    }
    result
}
