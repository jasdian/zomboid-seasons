use std::time::Duration;

use tokio::time::Instant;

use super::{Probe, ProbeResult};

pub struct RconProbe {
    host: String,
    port: u16,
    password: String,
    command: String,
    timeout: Duration,
}

impl RconProbe {
    pub fn new(
        host: String,
        port: u16,
        password: String,
        command: String,
        timeout: Duration,
    ) -> Self {
        Self {
            host,
            port,
            password,
            command,
            timeout,
        }
    }
}

impl Probe for RconProbe {
    fn check(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ProbeResult> + Send + '_>> {
        Box::pin(async move {
            let start = Instant::now();

            let url = format!("{}:{}", self.host, self.port);
            let password = self.password.clone();
            let command = self.command.clone();
            let timeout_secs = self.timeout.as_secs().max(1);

            let result = tokio::time::timeout(
                self.timeout,
                tokio::task::spawn_blocking(move || {
                    use rcon_client::{AuthRequest, RCONClient, RCONConfig, RCONRequest};

                    let config = RCONConfig {
                        url: url.clone(),
                        read_timeout: Some(timeout_secs),
                        write_timeout: Some(timeout_secs),
                    };

                    let mut client = RCONClient::new(config).map_err(|e| e.to_string())?;
                    client
                        .auth(AuthRequest::new(password))
                        .map_err(|e| e.to_string())?;
                    client
                        .execute(RCONRequest::new(command))
                        .map_err(|e| e.to_string())?;

                    Ok::<(), String>(())
                }),
            )
            .await;

            let elapsed_ms = start.elapsed().as_millis() as i64;

            match result {
                Ok(Ok(Ok(()))) => {
                    tracing::debug!(host = %self.host, port = %self.port, elapsed_ms, "rcon_probe_ok");
                    ProbeResult {
                        success: true,
                        response_ms: Some(elapsed_ms),
                        error_message: None,
                    }
                }
                Ok(Ok(Err(msg))) => {
                    tracing::warn!(host = %self.host, port = %self.port, error = %msg, "rcon_probe_error");
                    ProbeResult {
                        success: false,
                        response_ms: Some(elapsed_ms),
                        error_message: Some(msg),
                    }
                }
                Ok(Err(e)) => {
                    let msg = format!("spawn_blocking join error: {e}");
                    tracing::warn!(host = %self.host, port = %self.port, error = %msg, "rcon_probe_join_error");
                    ProbeResult {
                        success: false,
                        response_ms: Some(elapsed_ms),
                        error_message: Some(msg),
                    }
                }
                Err(_) => {
                    tracing::warn!(host = %self.host, port = %self.port, "rcon_probe_timeout");
                    ProbeResult {
                        success: false,
                        response_ms: Some(elapsed_ms),
                        error_message: Some("rcon probe timed out".into()),
                    }
                }
            }
        })
    }
}
