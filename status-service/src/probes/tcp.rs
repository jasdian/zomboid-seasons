use std::time::Duration;

use tokio::net::TcpStream;
use tokio::time::Instant;

use super::{Probe, ProbeResult};

pub struct TcpProbe {
    addr: String,
    timeout: Duration,
}

impl TcpProbe {
    pub fn new(host: String, port: u16, timeout: Duration) -> Self {
        Self {
            addr: format!("{host}:{port}"),
            timeout,
        }
    }
}

impl Probe for TcpProbe {
    fn check(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ProbeResult> + Send + '_>> {
        Box::pin(async move {
            let start = Instant::now();

            let result = tokio::time::timeout(self.timeout, TcpStream::connect(&self.addr)).await;

            let elapsed_ms = start.elapsed().as_millis() as i64;

            match result {
                Ok(Ok(_stream)) => {
                    tracing::debug!(addr = %self.addr, elapsed_ms, "tcp_probe_ok");
                    ProbeResult {
                        success: true,
                        response_ms: Some(elapsed_ms),
                        error_message: None,
                    }
                }
                Ok(Err(e)) => {
                    let msg = format!("connection error: {e}");
                    tracing::warn!(addr = %self.addr, error = %e, "tcp_probe_error");
                    ProbeResult {
                        success: false,
                        response_ms: Some(elapsed_ms),
                        error_message: Some(msg),
                    }
                }
                Err(_) => {
                    tracing::warn!(addr = %self.addr, "tcp_probe_timeout");
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
