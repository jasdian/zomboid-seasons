use std::time::Duration;

use tokio::time::Instant;

use super::{Probe, ProbeResult};

pub struct JsonRpcProbe {
    client: reqwest::Client,
    url: String,
    method: String,
    timeout: Duration,
}

impl JsonRpcProbe {
    pub fn new(client: reqwest::Client, url: String, method: String, timeout: Duration) -> Self {
        Self {
            client,
            url,
            method,
            timeout,
        }
    }
}

impl Probe for JsonRpcProbe {
    fn check(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ProbeResult> + Send + '_>> {
        Box::pin(async move {
            let start = Instant::now();

            let body = serde_json::json!({
                "jsonrpc": "2.0",
                "method": &self.method,
                "params": [],
                "id": 1
            });

            let result =
                tokio::time::timeout(self.timeout, self.client.post(&self.url).json(&body).send())
                    .await;

            let elapsed_ms = start.elapsed().as_millis() as i64;

            match result {
                Ok(Ok(response)) => match response.json::<serde_json::Value>().await {
                    Ok(json) => {
                        if json.get("error").is_some() {
                            let msg = format!(
                                "JSON-RPC error: {}",
                                json.get("error").unwrap_or(&serde_json::Value::Null)
                            );
                            tracing::warn!(url = %self.url, %msg, "json_rpc_probe_error_response");
                            ProbeResult {
                                success: false,
                                response_ms: Some(elapsed_ms),
                                error_message: Some(msg),
                            }
                        } else if json.get("result").is_some() {
                            tracing::debug!(url = %self.url, elapsed_ms, "json_rpc_probe_ok");
                            ProbeResult {
                                success: true,
                                response_ms: Some(elapsed_ms),
                                error_message: None,
                            }
                        } else {
                            let msg =
                                "JSON-RPC response missing both 'result' and 'error'".to_string();
                            tracing::warn!(url = %self.url, %msg, "json_rpc_probe_invalid_response");
                            ProbeResult {
                                success: false,
                                response_ms: Some(elapsed_ms),
                                error_message: Some(msg),
                            }
                        }
                    }
                    Err(e) => {
                        let msg = format!("failed to parse response: {e}");
                        tracing::warn!(url = %self.url, error = %e, "json_rpc_probe_parse_error");
                        ProbeResult {
                            success: false,
                            response_ms: Some(elapsed_ms),
                            error_message: Some(msg),
                        }
                    }
                },
                Ok(Err(e)) => {
                    let msg = format!("request error: {e}");
                    tracing::warn!(url = %self.url, error = %e, "json_rpc_probe_request_error");
                    ProbeResult {
                        success: false,
                        response_ms: Some(elapsed_ms),
                        error_message: Some(msg),
                    }
                }
                Err(_) => {
                    tracing::warn!(url = %self.url, "json_rpc_probe_timeout");
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
