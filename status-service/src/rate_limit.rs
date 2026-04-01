use std::net::IpAddr;

use axum::extract::connect_info::ConnectInfo;
use tower_governor::key_extractor::KeyExtractor;
use tower_governor::GovernorError;

/// Extracts client IP from `X-Real-IP` header (set by nginx), falling back to peer socket.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RealIpKeyExtractor;

impl KeyExtractor for RealIpKeyExtractor {
    type Key = IpAddr;

    fn extract<T>(&self, req: &axum::http::Request<T>) -> Result<Self::Key, GovernorError> {
        let maybe_ip = req
            .headers()
            .get("x-real-ip")
            .and_then(|val| val.to_str().ok())
            .and_then(|s| s.trim().parse::<IpAddr>().ok());

        if let Some(ip) = maybe_ip {
            return Ok(ip);
        }

        req.extensions()
            .get::<ConnectInfo<std::net::SocketAddr>>()
            .map(|ci| ci.0.ip())
            .ok_or(GovernorError::UnableToExtractKey)
    }
}
