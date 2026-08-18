// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Runtime event protocol between a live-preview application and Springboard.

use serde::{Deserialize, Serialize};
use std::io::{BufRead as _, BufReader, Write as _};
use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::time::Duration;

/// Environment variable containing Springboard's loopback control address.
pub const ENDPOINT_ENVIRONMENT_VARIABLE: &str = "SLINT_SPRINGBOARD_ENDPOINT";

/// Environment variable containing the token for the current project session.
pub const TOKEN_ENVIRONMENT_VARIABLE: &str = "SLINT_SPRINGBOARD_TOKEN";

/// Current version of the Rust application runtime protocol.
pub const PROTOCOL_VERSION: u32 = 1;

const IO_TIMEOUT: Duration = Duration::from_secs(1);

/// An outcome reported by a Rust live-preview application.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "event")]
pub enum RuntimeEvent {
    /// The initial live component is ready.
    Ready { hot_reload_paths: Vec<PathBuf> },
    /// An implementation-only change was applied.
    Reloaded { hot_reload_paths: Vec<PathBuf> },
    /// A candidate component did not compile.
    CompileError { hot_reload_paths: Vec<PathBuf> },
    /// The generated Rust interface changed.
    RebuildRequired { diff: String, hot_reload_paths: Vec<PathBuf> },
    /// The application is exiting.
    Exiting,
}

impl RuntimeEvent {
    /// Return the live compiler's current source and resource graph.
    pub fn hot_reload_paths(&self) -> Option<&[PathBuf]> {
        match self {
            Self::Ready { hot_reload_paths }
            | Self::Reloaded { hot_reload_paths }
            | Self::CompileError { hot_reload_paths }
            | Self::RebuildRequired { hot_reload_paths, .. } => Some(hot_reload_paths),
            Self::Exiting => None,
        }
    }

    /// Return whether the application requested a Cargo rebuild.
    pub fn requires_rebuild(&self) -> bool {
        matches!(self, Self::RebuildRequired { .. })
    }
}

/// A token-authenticated event sent to Springboard.
#[derive(Debug, Deserialize, Serialize)]
pub struct RuntimeEventEnvelope {
    pub protocol_version: u32,
    pub token: String,
    #[serde(flatten)]
    pub event: RuntimeEvent,
}

/// Springboard's acknowledgement for one runtime event.
#[derive(Debug, Deserialize, Serialize)]
pub struct RuntimeEventAcknowledgement {
    pub protocol_version: u32,
    pub accepted: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// A synchronous reporter suitable for use on the Slint event-loop thread.
pub struct SpringboardRuntimeReporter {
    reader: BufReader<TcpStream>,
    writer: TcpStream,
    token: String,
}

impl SpringboardRuntimeReporter {
    /// Connect to the Springboard session described by the process environment.
    pub fn from_environment() -> Result<Option<Self>, String> {
        Self::from_environment_values(
            std::env::var(ENDPOINT_ENVIRONMENT_VARIABLE).ok(),
            std::env::var(TOKEN_ENVIRONMENT_VARIABLE).ok(),
        )
    }

    fn from_environment_values(
        endpoint: Option<String>,
        token: Option<String>,
    ) -> Result<Option<Self>, String> {
        match (endpoint, token) {
            (None, None) => Ok(None),
            (Some(_), None) => Err(format!(
                "{TOKEN_ENVIRONMENT_VARIABLE} is missing while {ENDPOINT_ENVIRONMENT_VARIABLE} is set"
            )),
            (None, Some(_)) => Err(format!(
                "{ENDPOINT_ENVIRONMENT_VARIABLE} is missing while {TOKEN_ENVIRONMENT_VARIABLE} is set"
            )),
            (Some(endpoint), Some(token)) => {
                let endpoint = endpoint.parse().map_err(|error| {
                    format!("invalid {ENDPOINT_ENVIRONMENT_VARIABLE} value '{endpoint}': {error}")
                })?;
                Self::connect(endpoint, token).map(Some).map_err(|error| {
                    format!("cannot connect to Springboard at {endpoint}: {error}")
                })
            }
        }
    }

    /// Connect to an explicit Springboard control address.
    pub fn connect(endpoint: SocketAddr, token: impl Into<String>) -> std::io::Result<Self> {
        if !endpoint.ip().is_loopback() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Springboard runtime endpoints must use a loopback address",
            ));
        }
        let writer = TcpStream::connect_timeout(&endpoint, IO_TIMEOUT)?;
        writer.set_nodelay(true)?;
        writer.set_read_timeout(Some(IO_TIMEOUT))?;
        writer.set_write_timeout(Some(IO_TIMEOUT))?;
        let reader = BufReader::new(writer.try_clone()?);
        Ok(Self { reader, writer, token: token.into() })
    }

    /// Report one event and wait for Springboard to acknowledge it.
    pub fn report(&mut self, event: RuntimeEvent) -> std::io::Result<()> {
        let envelope = RuntimeEventEnvelope {
            protocol_version: PROTOCOL_VERSION,
            token: self.token.clone(),
            event,
        };
        serde_json::to_writer(&mut self.writer, &envelope).map_err(invalid_data)?;
        self.writer.write_all(b"\n")?;
        self.writer.flush()?;

        let mut response = String::new();
        if self.reader.read_line(&mut response)? == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "Springboard closed the runtime connection",
            ));
        }
        let acknowledgement: RuntimeEventAcknowledgement =
            serde_json::from_str(&response).map_err(invalid_data)?;
        if acknowledgement.protocol_version != PROTOCOL_VERSION {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Springboard returned an incompatible runtime protocol version",
            ));
        }
        if !acknowledgement.accepted {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                acknowledgement.error.unwrap_or_else(|| "Springboard rejected the event".into()),
            ));
        }
        Ok(())
    }
}

fn invalid_data(error: impl std::fmt::Display) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_application_outside_springboard_has_no_reporter() {
        assert!(SpringboardRuntimeReporter::from_environment_values(None, None).unwrap().is_none());
    }

    #[test]
    fn partial_environment_is_rejected() {
        let error = SpringboardRuntimeReporter::from_environment_values(
            Some("127.0.0.1:1234".into()),
            None,
        )
        .err()
        .unwrap();
        assert!(error.contains(TOKEN_ENVIRONMENT_VARIABLE));
    }

    #[test]
    fn non_loopback_endpoints_are_rejected() {
        let error = SpringboardRuntimeReporter::connect("192.0.2.1:1234".parse().unwrap(), "token")
            .err()
            .unwrap();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    }
}
