// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell:ignore dialable undialable

//! Remote-preview discovery, run inside the preview process.
//!
//! The mDNS daemon is created on first use, so the OS firewall prompt only
//! fires when the user opens the Remote Preview dialog.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use i_slint_live_preview::protocol::{
    PROTOCOL_SUBPROTOCOL, PreviewToLspMessage, SERVICE_TYPE, SLINT_VERSION, TXT_PROTOCOLS_KEY,
    TXT_SLINT_VERSION_KEY,
};
use slint::{Model, ModelRc, SharedString, VecModel};

use crate::common;
use crate::preview::ui::{Api, AppWindow, RemoteConnectionState, RemoteViewerInfo};

/// Register the Remote Preview callbacks on the window's `Api` global.
pub fn setup(app_window: &AppWindow, to_lsp: &Rc<dyn common::PreviewToLsp>) {
    let api = app_window.api();
    api.set_remote_discovered_viewers(ModelRc::new(VecModel::<RemoteViewerInfo>::default()));

    let api_weak = app_window.api_weak();
    api.on_remote_start_discovery(move || {
        let api_weak = api_weak.clone();
        crate::preview::PREVIEW_STATE.with_borrow(|preview_state| {
            preview_state.remote_discovery.start(api_weak);
        });
    });
    api.on_remote_stop_discovery(|| {
        crate::preview::PREVIEW_STATE.with_borrow(|preview_state| {
            preview_state.remote_discovery.stop();
        });
    });

    let lsp = to_lsp.clone();
    let api_weak_for_port = app_window.api_weak();
    api.on_remote_connect(move |addresses, port| {
        let Ok(port) =
            u16::try_from(port).map_err(|_| ()).and_then(|p| if p == 0 { Err(()) } else { Ok(p) })
        else {
            tracing::warn!("Discovered viewer has invalid port {port}; not connecting");
            report_manual_entry_error(&api_weak_for_port, "Discovered viewer has an invalid port");
            return;
        };
        let addresses = addresses.iter().map(|a| a.to_string()).collect::<Vec<String>>();
        if let Err(err) = lsp.send(&PreviewToLspMessage::ConnectRemote { addresses, port }) {
            tracing::error!("Failed sending ConnectRemote to LSP: {err}");
        }
    });

    let lsp = to_lsp.clone();
    let api_weak_for_validate = app_window.api_weak();
    api.on_remote_connect_manual(move |host_port| match parse_host_port(host_port.as_str()) {
        Ok((host, port)) => {
            if let Err(err) =
                lsp.send(&PreviewToLspMessage::ConnectRemote { addresses: vec![host], port })
            {
                tracing::error!("Failed sending ConnectRemote to LSP: {err}");
            }
        }
        Err(parse_err) => {
            tracing::warn!(
                "Invalid manual remote-preview address {host_port:?}: {}",
                parse_err.message()
            );
            report_manual_entry_error(&api_weak_for_validate, parse_err.message());
        }
    });

    let lsp = to_lsp.clone();
    api.on_remote_disconnect(move || {
        if let Err(err) = lsp.send(&PreviewToLspMessage::DisconnectRemote) {
            tracing::error!("Failed sending DisconnectRemote to LSP: {err}");
        }
    });
}

/// Surface a manual-entry / discovery error in the dialog. Skipped when
/// a remote connection is currently live so a typo doesn't lie about a
/// peer that's still receiving traffic.
fn report_manual_entry_error(api_weak: &slint::Weak<Api<'static>>, message: &str) {
    let message = message.to_owned();
    let _ = api_weak.upgrade_in_event_loop(move |api| {
        if api.get_remote_connection_state() == RemoteConnectionState::Connected {
            return;
        }
        api.set_remote_connection_state(RemoteConnectionState::Failed);
        api.set_remote_connection_target(SharedString::default());
        api.set_remote_connection_error(message.into());
    });
}

#[derive(Debug, PartialEq, Eq)]
enum HostPortError {
    Empty,
    BareIpv6,
    ExtraColons,
    MissingPort,
    InvalidPort,
    InvalidHost,
}

impl HostPortError {
    fn message(&self) -> &'static str {
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
fn parse_host_port(input: &str) -> Result<(String, u16), HostPortError> {
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

/// Lives in [`crate::preview::PREVIEW_STATE`] (thread-local), so a `RefCell` is enough.
#[derive(Default)]
pub struct RemoteDiscovery {
    inner: RefCell<Option<ActiveDiscovery>>,
    daemon: RefCell<Option<mdns_sd::ServiceDaemon>>,
    expiry_timer: RefCell<Option<slint::Timer>>,
}

struct ActiveDiscovery {
    stop_flag: Arc<AtomicBool>,
}

/// Prune viewers that haven't been re-resolved in this long.
const EXPIRY_AGE: std::time::Duration = std::time::Duration::from_secs(60);
const EXPIRY_CHECK_INTERVAL: std::time::Duration = std::time::Duration::from_secs(5);
/// Short enough to close the dialog promptly, long enough to avoid busy-looping.
const STOP_FLAG_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(250);

impl RemoteDiscovery {
    pub fn start(&self, api: slint::Weak<Api<'static>>) {
        let mut guard = self.inner.borrow_mut();
        if guard.is_some() {
            return;
        }

        let daemon = {
            let mut daemon_guard = self.daemon.borrow_mut();
            if daemon_guard.is_none() {
                match mdns_sd::ServiceDaemon::new() {
                    Ok(daemon) => *daemon_guard = Some(daemon),
                    Err(err) => {
                        tracing::error!("Failed to create mDNS service daemon: {err}");
                        return;
                    }
                }
            }
            daemon_guard.as_ref().unwrap().clone()
        };

        let receiver = match daemon.browse(SERVICE_TYPE) {
            Ok(r) => r,
            Err(err) => {
                tracing::error!("Failed to start mDNS browsing: {err}");
                return;
            }
        };

        let _ = api.upgrade_in_event_loop(|api| {
            api.set_remote_discovered_viewers(
                ModelRc::new(VecModel::<RemoteViewerInfo>::default()),
            );
        });

        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_flag_clone = stop_flag.clone();
        let api_for_timer = api.clone();
        // Detached: joining from the UI thread would freeze the preview
        // window if mdns-sd's stop_browse stalled.
        std::thread::spawn(move || {
            browse_loop(receiver, api, stop_flag_clone);
        });

        let timer = slint::Timer::default();
        timer.start(slint::TimerMode::Repeated, EXPIRY_CHECK_INTERVAL, move || {
            if let Some(api) = api_for_timer.upgrade() {
                prune_stale(&api);
            }
        });
        *self.expiry_timer.borrow_mut() = Some(timer);

        *guard = Some(ActiveDiscovery { stop_flag });
    }

    pub fn stop(&self) {
        self.expiry_timer.borrow_mut().take();
        let Some(active) = self.inner.borrow_mut().take() else {
            return;
        };
        active.stop_flag.store(true, Ordering::Relaxed);
        if let Some(daemon) = self.daemon.borrow().as_ref() {
            // Best-effort; the worker also polls `stop_flag`.
            let _ = daemon.stop_browse(SERVICE_TYPE);
        }
    }
}

impl Drop for RemoteDiscovery {
    fn drop(&mut self) {
        self.stop();
        if let Some(daemon) = self.daemon.borrow_mut().take()
            && let Err(err) = daemon.shutdown()
        {
            tracing::error!("Failed shutting down mDNS service daemon: {err}");
        }
    }
}

fn browse_loop(
    receiver: mdns_sd::Receiver<mdns_sd::ServiceEvent>,
    api: slint::Weak<Api<'static>>,
    stop_flag: Arc<AtomicBool>,
) {
    loop {
        if stop_flag.load(Ordering::Relaxed) {
            return;
        }
        let event = match receiver.try_recv() {
            Ok(e) => e,
            Err(_) => {
                if receiver.is_disconnected() {
                    return;
                }
                std::thread::sleep(STOP_FLAG_POLL_INTERVAL);
                continue;
            }
        };
        if stop_flag.load(Ordering::Relaxed) {
            return;
        }
        // `ResolvedService` is `Send`, so we forward it as-is and do the
        // Slint-side conversion on the UI thread.
        match event {
            mdns_sd::ServiceEvent::ServiceResolved(resolved) => {
                let api = api.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(api) = api.upgrade() {
                        upsert_viewer(&api, *resolved);
                    }
                });
            }
            mdns_sd::ServiceEvent::ServiceRemoved(_, fullname) => {
                let api = api.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(api) = api.upgrade() {
                        remove_viewer(&api, &fullname);
                    }
                });
            }
            _ => {}
        }
    }
}

/// Run `f` against the discovered-viewers `VecModel`, or warn and skip if
/// the model has been swapped for a non-`VecModel` type.
fn with_viewers<R>(api: &Api, f: impl FnOnce(&VecModel<RemoteViewerInfo>) -> R) -> Option<R> {
    let model = api.get_remote_discovered_viewers();
    let result = model.as_any().downcast_ref::<VecModel<RemoteViewerInfo>>().map(f);
    if result.is_none() {
        tracing::warn!("remote-discovered-viewers is not a VecModel; update dropped");
    }
    result
}

fn upsert_viewer(api: &Api, resolved: mdns_sd::ResolvedService) {
    let new_info = to_remote_viewer_info(resolved);
    let fullname = new_info.fullname.clone();
    with_viewers(api, |viewers| {
        if let Some((row, _)) =
            viewers.iter().enumerate().find(|(_, existing)| existing.fullname == fullname)
        {
            viewers.set_row_data(row, new_info);
        } else {
            viewers.push(new_info);
        }
    });
}

fn remove_viewer(api: &Api, fullname: &str) {
    with_viewers(api, |viewers| {
        if let Some((row, _)) =
            viewers.iter().enumerate().find(|(_, existing)| existing.fullname == fullname)
        {
            viewers.remove(row);
        }
    });
}

fn prune_stale(api: &Api) {
    let cutoff = now_secs().saturating_sub(EXPIRY_AGE.as_secs() as i32);
    with_viewers(api, |viewers| {
        // Walk backwards: removals don't shift indices we haven't visited yet.
        for i in (0..viewers.row_count()).rev() {
            let Some(row) = viewers.row_data(i) else { continue };
            if row.last_seen_secs < cutoff {
                tracing::debug!("Pruning stale discovered viewer {:?}", row.fullname);
                viewers.remove(i);
            }
        }
    });
}

fn to_remote_viewer_info(resolved: mdns_sd::ResolvedService) -> RemoteViewerInfo {
    // Instance name is the leading label of `fullname`; mdns-sd keeps it raw.
    let name = resolved
        .fullname
        .strip_suffix(&format!(".{}", resolved.ty_domain))
        .filter(|n| !n.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| resolved.host.clone());

    let viewer_protocols =
        resolved.txt_properties.get_property_val_str(TXT_PROTOCOLS_KEY).map(str::to_owned);
    let viewer_slint_version =
        resolved.txt_properties.get_property_val_str(TXT_SLINT_VERSION_KEY).map(str::to_owned);
    let (compatible, incompatible_reason) =
        check_compatibility(viewer_protocols.as_deref(), viewer_slint_version.as_deref());

    let addresses = dialable_addresses(resolved.addresses);

    RemoteViewerInfo {
        fullname: resolved.fullname.into(),
        name: name.into(),
        host: resolved.host.into(),
        port: resolved.port as i32,
        addresses: ModelRc::new(VecModel::from(addresses)),
        compatible,
        incompatible_reason: incompatible_reason.into(),
        last_seen_secs: now_secs(),
    }
}

/// Convert resolved mDNS addresses into strings the connector can dial.
fn dialable_addresses(addresses: impl IntoIterator<Item = mdns_sd::ScopedIp>) -> Vec<SharedString> {
    addresses
        .into_iter()
        .filter_map(|addr| match addr {
            mdns_sd::ScopedIp::V4(ip) => Some(ip.addr().to_string().into()),
            mdns_sd::ScopedIp::V6(ip) => dialable_v6(ip.addr(), ip.scope_id().index),
            _ => None,
        })
        .collect()
}

/// Format an IPv6 address so the connector can dial it. Link-local needs a
/// zone id, and only the numeric form (`[fe80::1%1]`) survives the ws://
/// stack. `scope_index` is the local interface the mDNS record arrived on;
/// 0 means unknown, which makes link-local undialable, so drop it.
fn dialable_v6(addr: &std::net::Ipv6Addr, scope_index: u32) -> Option<SharedString> {
    if addr.is_unicast_link_local() {
        (scope_index != 0).then(|| format!("[{addr}%{scope_index}]").into())
    } else {
        Some(format!("[{addr}]").into())
    }
}

fn check_compatibility(
    viewer_protocols: Option<&str>,
    viewer_slint_version: Option<&str>,
) -> (bool, String) {
    let Some(protocols) = viewer_protocols else {
        return (
            false,
            format!(
                "Viewer pre-dates protocol versioning; this LSP speaks {PROTOCOL_SUBPROTOCOL} (Slint {SLINT_VERSION})"
            ),
        );
    };
    if protocols.split(',').any(|p| p.trim() == PROTOCOL_SUBPROTOCOL) {
        (true, String::new())
    } else {
        let viewer_version = viewer_slint_version.unwrap_or("unknown");
        (
            false,
            format!(
                "Viewer runs Slint {viewer_version} (protocols {protocols}); this LSP speaks {PROTOCOL_SUBPROTOCOL} (Slint {SLINT_VERSION})"
            ),
        )
    }
}

// Seconds since this process started. Monotonic and only ever used to
// compute relative ages for pruning, so it side-steps the year-2038 wrap
// that absolute Unix time stored in an i32 would hit.
fn now_secs() -> i32 {
    use std::sync::OnceLock;
    use std::time::Instant;
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs() as i32
}

#[cfg(test)]
mod dialable_addresses_tests {
    use super::*;
    use std::net::{IpAddr, Ipv6Addr};

    fn scoped(ip: &str) -> mdns_sd::ScopedIp {
        mdns_sd::ScopedIp::from(ip.parse::<IpAddr>().unwrap())
    }

    fn v6(ip: &str) -> Ipv6Addr {
        ip.parse().unwrap()
    }

    #[test]
    fn link_local_gets_numeric_zone() {
        assert_eq!(dialable_v6(&v6("fe80::1"), 1), Some("[fe80::1%1]".into()));
    }

    #[test]
    fn link_local_without_interface_index_is_dropped() {
        assert_eq!(dialable_v6(&v6("fe80::1"), 0), None);
    }

    #[test]
    fn global_ipv6_needs_no_zone() {
        assert_eq!(dialable_v6(&v6("2001:db8::5"), 0), Some("[2001:db8::5]".into()));
        assert_eq!(dialable_v6(&v6("2001:db8::5"), 16), Some("[2001:db8::5]".into()));
    }

    #[test]
    fn keeps_ipv4() {
        // `From<IpAddr>` yields interface index 0, so the link-local
        // address is dropped here.
        let addresses = dialable_addresses([scoped("192.168.1.57"), scoped("fe80::1")]);
        assert_eq!(addresses, vec![SharedString::from("192.168.1.57")]);
    }
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
