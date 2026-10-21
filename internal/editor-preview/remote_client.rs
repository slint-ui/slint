// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::time::Duration;

use futures_util::{Sink, SinkExt as _};
use i_slint_live_preview::protocol::PreviewToLspMessage;
use tokio_tungstenite_wasm::Message;

pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
pub const PING_INTERVAL: Duration = Duration::from_secs(5);
pub const PONG_TIMEOUT: Duration = Duration::from_secs(15);
pub const CLOSE_TIMEOUT: Duration = Duration::from_secs(1);

pub fn is_allowed_message(message: &PreviewToLspMessage) -> bool {
    matches!(
        message,
        PreviewToLspMessage::Diagnostics { .. }
            | PreviewToLspMessage::DebugMessage { .. }
            | PreviewToLspMessage::RequestState { .. }
    )
}

pub async fn close(socket: &mut (impl Sink<Message> + Unpin)) {
    let _ = tokio::time::timeout(CLOSE_TIMEOUT, socket.close()).await;
}

#[derive(Debug, PartialEq, Eq)]
pub enum HostPortError {
    Empty,
    BareIpv6,
    ExtraColons,
    MissingPort,
    InvalidPort,
    InvalidHost,
}

impl HostPortError {
    pub fn message(&self) -> &'static str {
        match self {
            HostPortError::Empty => "Enter a host and port",
            HostPortError::BareIpv6 => "IPv6 addresses must be bracketed, e.g. [::1]:1234",
            HostPortError::ExtraColons => "Too many `:` characters; expected host:port",
            HostPortError::MissingPort => "Missing `:port`",
            HostPortError::InvalidPort => "Port must be a number between 1 and 65535",
            HostPortError::InvalidHost => "Host is empty",
        }
    }
}

/// Parse `host:port` or `[ipv6]:port` into a `(host, port)` pair.
///
/// `std::net::SocketAddr::from_str` rejects anything that isn't an IP, and
/// `url::Url::parse` collapses every rejection into a single `ParseError`.
/// We want to accept a hostname like `phone.local:1234` and surface typed
/// errors verbatim in the dialog, so we parse by hand.
pub fn parse_host_port(input: &str) -> Result<(String, u16), HostPortError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(HostPortError::Empty);
    }
    if let Some(rest) = trimmed.strip_prefix('[') {
        let end = rest.find(']').ok_or(HostPortError::MissingPort)?;
        let host = &rest[..end];
        if host.is_empty() {
            return Err(HostPortError::InvalidHost);
        }
        let after = &rest[end + 1..];
        let port_str = after.strip_prefix(':').ok_or(HostPortError::MissingPort)?;
        let port = port_str.parse::<u16>().map_err(|_| HostPortError::InvalidPort)?;
        if port == 0 {
            return Err(HostPortError::InvalidPort);
        }
        return Ok((format!("[{host}]"), port));
    }
    // Bracketed IPv6 is the only legitimate way to have more than one `:`.
    // Bare `::1` etc. is a common typo and gets its own dedicated message;
    // anything else with extra colons (`host:8080:9000`) is ExtraColons.
    let colon_count = trimmed.bytes().filter(|&b| b == b':').count();
    if colon_count == 0 {
        return Err(HostPortError::MissingPort);
    }
    if colon_count > 1 {
        if trimmed.contains("::") {
            return Err(HostPortError::BareIpv6);
        }
        return Err(HostPortError::ExtraColons);
    }
    let (host, port_str) = trimmed.split_once(':').unwrap();
    if host.is_empty() {
        return Err(HostPortError::InvalidHost);
    }
    let port = port_str.parse::<u16>().map_err(|_| HostPortError::InvalidPort)?;
    if port == 0 {
        return Err(HostPortError::InvalidPort);
    }
    Ok((host.to_owned(), port))
}

#[cfg(test)]
mod parse_host_port_tests {
    use super::*;

    #[test]
    fn rejects_empty() {
        assert_eq!(parse_host_port(""), Err(HostPortError::Empty));
        assert_eq!(parse_host_port("   "), Err(HostPortError::Empty));
    }

    #[test]
    fn rejects_bare_ipv6() {
        assert_eq!(parse_host_port("::1"), Err(HostPortError::BareIpv6));
        assert_eq!(parse_host_port("fe80::1"), Err(HostPortError::BareIpv6));
    }

    #[test]
    fn rejects_extra_colons() {
        assert_eq!(parse_host_port("example.com:8080:9000"), Err(HostPortError::ExtraColons));
    }

    #[test]
    fn rejects_invalid_port() {
        assert_eq!(parse_host_port("host:0"), Err(HostPortError::InvalidPort));
        assert_eq!(parse_host_port("host:99999"), Err(HostPortError::InvalidPort));
        assert_eq!(parse_host_port("host:abc"), Err(HostPortError::InvalidPort));
    }

    #[test]
    fn rejects_missing_port() {
        assert_eq!(parse_host_port("host"), Err(HostPortError::MissingPort));
        assert_eq!(parse_host_port("[::1]"), Err(HostPortError::MissingPort));
    }

    #[test]
    fn rejects_empty_host() {
        assert_eq!(parse_host_port(":1234"), Err(HostPortError::InvalidHost));
        assert_eq!(parse_host_port("[]:1234"), Err(HostPortError::InvalidHost));
    }

    #[test]
    fn extra_colons_without_ipv6_is_extra_colons() {
        assert_eq!(parse_host_port("localhost:8080:9000"), Err(HostPortError::ExtraColons));
    }

    #[test]
    fn accepts_ipv4_host_port() {
        assert_eq!(parse_host_port("192.168.1.42:9000"), Ok(("192.168.1.42".to_owned(), 9000)));
    }

    #[test]
    fn accepts_hostname_port() {
        assert_eq!(parse_host_port("phone.local:1234"), Ok(("phone.local".to_owned(), 1234)));
    }

    #[test]
    fn accepts_bracketed_ipv6() {
        assert_eq!(parse_host_port("[::1]:1234"), Ok(("[::1]".to_owned(), 1234)));
        assert_eq!(parse_host_port("[fe80::1]:9000"), Ok(("[fe80::1]".to_owned(), 9000)));
    }
}
