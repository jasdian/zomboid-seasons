use std::net::IpAddr;

use axum::extract::connect_info::ConnectInfo;
use tower_governor::key_extractor::KeyExtractor;
use tower_governor::GovernorError;

/// Extracts client IP from the `X-Real-IP` header (set by nginx to `$remote_addr`,
/// which overwrites any client-supplied value), falling back to the peer socket address.
///
/// Unlike `SmartIpKeyExtractor`, this does NOT trust `X-Forwarded-For` which can be
/// spoofed when nginx uses `$proxy_add_x_forwarded_for` (appends rather than overwrites).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RealIpKeyExtractor;

impl KeyExtractor for RealIpKeyExtractor {
    type Key = IpAddr;

    fn extract<T>(&self, req: &axum::http::Request<T>) -> Result<Self::Key, GovernorError> {
        // Try X-Real-IP first (trustworthy — nginx overwrites it with $remote_addr)
        let maybe_ip = req
            .headers()
            .get("x-real-ip")
            .and_then(|val| val.to_str().ok())
            .and_then(|s| s.trim().parse::<IpAddr>().ok());

        if let Some(ip) = maybe_ip {
            return Ok(ip);
        }

        // Fallback to peer socket address
        req.extensions()
            .get::<ConnectInfo<std::net::SocketAddr>>()
            .map(|ci| ci.0.ip())
            .ok_or(GovernorError::UnableToExtractKey)
    }
}
