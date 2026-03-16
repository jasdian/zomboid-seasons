use std::time::Duration;

use tokio::time::Instant;

use super::{Probe, ProbeResult};

pub struct HttpProbe {
    client: reqwest::Client,
    url: String,
    method: String,
    expected_status: u16,
    timeout: Duration,
}

impl HttpProbe {
    pub fn new(
        client: reqwest::Client,
        url: String,
        method: String,
        expected_status: u16,
        timeout: Duration,
    ) -> Self {
        Self {
            client,
            url,
            method,
            expected_status,
            timeout,
        }
    }
}

impl Probe for HttpProbe {
    fn check(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ProbeResult> + Send + '_>> {
        Box::pin(async move {
            let start = Instant::now();

            let request = match self.method.to_uppercase().as_str() {
                "HEAD" => self.client.head(&self.url),
                _ => self.client.get(&self.url),
            };

            let result = tokio::time::timeout(self.timeout, request.send()).await;

            let elapsed_ms = start.elapsed().as_millis() as i64;

            match result {
                Ok(Ok(response)) => {
                    let status = response.status().as_u16();
                    if status == self.expected_status {
                        tracing::debug!(url = %self.url, status, elapsed_ms, "http_probe_ok");
                        ProbeResult {
                            success: true,
                            response_ms: Some(elapsed_ms),
                            error_message: None,
                        }
                    } else {
                        let msg = format!("expected status {}, got {status}", self.expected_status);
                        tracing::warn!(url = %self.url, %msg, "http_probe_status_mismatch");
                        ProbeResult {
                            success: false,
                            response_ms: Some(elapsed_ms),
                            error_message: Some(msg),
                        }
                    }
                }
                Ok(Err(e)) => {
                    let msg = format!("request error: {e}");
                    tracing::warn!(url = %self.url, error = %e, "http_probe_error");
                    ProbeResult {
                        success: false,
                        response_ms: Some(elapsed_ms),
                        error_message: Some(msg),
                    }
                }
                Err(_) => {
                    tracing::warn!(url = %self.url, "http_probe_timeout");
                    ProbeResult {
                        success: false,
                        response_ms: Some(elapsed_ms),
                        error_message: Some("timeout".into()),
                    }
                }
            }
        })
    }
}
