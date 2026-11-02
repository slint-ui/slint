// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::collections::HashMap;

use tokio::sync::mpsc;

use i_slint_live_preview::protocol::{
    PROTOCOL_SUBPROTOCOL, SERVICE_TYPE, SLINT_VERSION, TXT_PROTOCOLS_KEY, TXT_SLINT_VERSION_KEY,
};

use super::{EndpointIdentity, SerialIdentity};
use crate::serial::{
    SERIAL_SCAN_INTERVAL, SerialConnection, SerialEvent, UsbSerialPort, usb_serial_ports,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct RemoteViewer {
    pub fullname: String,
    pub name: String,
    pub addresses: Vec<String>,
    pub port: u16,
    pub unavailable_reason: String,
}

#[derive(Debug)]
enum DiscoveryEvent {
    Updated(RemoteViewer),
    Removed(String),
}

struct Discovery {
    daemon: Option<mdns_sd::ServiceDaemon>,
    receiver: mdns_sd::Receiver<mdns_sd::ServiceEvent>,
}

impl Discovery {
    fn start() -> crate::Result<Self> {
        let daemon = mdns_sd::ServiceDaemon::new()?;
        match daemon.browse(SERVICE_TYPE) {
            Ok(receiver) => Ok(Self { daemon: Some(daemon), receiver }),
            Err(error) => {
                let _ = daemon.shutdown();
                Err(error.into())
            }
        }
    }

    /// Query again right away, instead of waiting out mDNS's query backoff.
    fn restart(&mut self) -> crate::Result<()> {
        let Some(daemon) = &self.daemon else { return Ok(()) };
        // Calling `browse` again without stopping keeps the old query schedule
        // running next to the new one.
        daemon.stop_browse(SERVICE_TYPE)?;
        self.receiver = daemon.browse(SERVICE_TYPE)?;
        Ok(())
    }

    async fn next_event(&self) -> crate::Result<DiscoveryEvent> {
        loop {
            match self.receiver.recv_async().await? {
                mdns_sd::ServiceEvent::ServiceResolved(viewer) => {
                    return Ok(DiscoveryEvent::Updated(normalize(*viewer)));
                }
                mdns_sd::ServiceEvent::ServiceRemoved(_, fullname) => {
                    return Ok(DiscoveryEvent::Removed(fullname));
                }
                _ => {}
            }
        }
    }

    #[cfg(test)]
    fn from_receiver(receiver: mdns_sd::Receiver<mdns_sd::ServiceEvent>) -> Self {
        Self { daemon: None, receiver }
    }
}

impl Drop for Discovery {
    fn drop(&mut self) {
        if let Some(daemon) = &self.daemon {
            let _ = daemon.stop_browse(SERVICE_TYPE);
            let _ = daemon.shutdown();
        }
    }
}

impl RemoteViewer {
    /// A viewer the user entered by address, dialed like a discovered one.
    // NOTE: a scoped IPv6 address such as `fe80::1%eth0` doesn't parse, so it
    // isn't bracketed and can't be dialed.
    pub(super) fn manual(host: &str, port: u16) -> Self {
        let address = match host.parse::<std::net::Ipv6Addr>() {
            Ok(_) => format!("[{host}]"),
            Err(_) => host.to_owned(),
        };
        Self {
            fullname: String::new(),
            name: host.to_owned(),
            addresses: vec![address],
            port,
            unavailable_reason: String::new(),
        }
    }

    // TODO: use the host the viewer advertises in its TXT record, which survives
    // conflict renaming, once viewers advertise one.
    pub(super) fn identity(&self) -> EndpointIdentity {
        EndpointIdentity::Remote { host: self.name.clone() }
    }
}

fn normalize(viewer: mdns_sd::ResolvedService) -> RemoteViewer {
    let name = viewer
        .fullname
        .strip_suffix(&format!(".{}", viewer.ty_domain))
        .filter(|name| !name.is_empty())
        .unwrap_or(&viewer.host)
        .to_owned();
    let protocols = viewer.txt_properties.get_property_val_str(TXT_PROTOCOLS_KEY);
    let mut unavailable_reason = match protocols {
        Some(protocols)
            if protocols.split(',').any(|protocol| protocol.trim() == PROTOCOL_SUBPROTOCOL) =>
        {
            String::new()
        }
        Some(protocols) => format!(
            "Viewer runs Slint {} (protocols {protocols}); Springboard speaks {PROTOCOL_SUBPROTOCOL} (Slint {SLINT_VERSION})",
            viewer.txt_properties.get_property_val_str(TXT_SLINT_VERSION_KEY).unwrap_or("unknown")
        ),
        None => format!(
            "Viewer pre-dates protocol versioning; Springboard speaks {PROTOCOL_SUBPROTOCOL} (Slint {SLINT_VERSION})"
        ),
    };
    let mut addresses: Vec<_> = viewer
        .addresses
        .into_iter()
        .filter_map(|address| match address {
            mdns_sd::ScopedIp::V4(address) => Some(address.addr().to_string()),
            mdns_sd::ScopedIp::V6(address) => usable_ipv6(address.addr(), address.scope_id().index),
            _ => None,
        })
        .collect();
    addresses.sort();
    addresses.dedup();
    if unavailable_reason.is_empty() {
        if viewer.port == 0 {
            unavailable_reason = "Viewer advertises an invalid port".into();
        } else if addresses.is_empty() {
            unavailable_reason = "Viewer has no usable address".into();
        }
    }
    RemoteViewer {
        fullname: viewer.fullname,
        name,
        addresses,
        port: viewer.port,
        unavailable_reason,
    }
}

fn usable_ipv6(address: &std::net::Ipv6Addr, scope_index: u32) -> Option<String> {
    if address.is_unicast_link_local() {
        (scope_index != 0).then(|| format!("[{address}%{scope_index}]"))
    } else {
        Some(format!("[{address}]"))
    }
}

#[derive(Default)]
pub(super) struct DiscoveredViewers {
    mdns: Option<Discovery>,
    pub(super) error: String,
    pub(super) discovered: Vec<RemoteViewer>,
}

impl DiscoveredViewers {
    pub(super) fn start() -> Self {
        let mut viewers = Self::default();
        viewers.set_mdns(Discovery::start());
        viewers
    }

    fn set_mdns(&mut self, mdns: crate::Result<Discovery>) {
        match mdns {
            Ok(mdns) => {
                self.mdns = Some(mdns);
                self.error.clear();
            }
            Err(error) => {
                self.mdns = None;
                self.discovered.clear();
                self.error = format!("Cannot discover viewers: {error}");
            }
        }
    }

    /// Start browsing again, or restart it to reset mDNS's query backoff.
    pub(super) fn restart(&mut self) {
        // mdns-sd drops its cache without reporting the removals, so a viewer
        // that vanished meanwhile would stay listed.
        self.discovered.clear();
        let mdns = match self.mdns.take() {
            Some(mut mdns) => mdns.restart().map(|()| mdns),
            None => Discovery::start(),
        };
        self.set_mdns(mdns);
    }

    /// Wait until `discovered` or `error` changes. Pending while not browsing.
    pub(super) async fn changed(&mut self) {
        loop {
            let Some(mdns) = &self.mdns else { return std::future::pending().await };
            let event = mdns.next_event().await;
            match event {
                Ok(event) => {
                    if self.update(event) {
                        return;
                    }
                }
                Err(error) => {
                    self.mdns = None;
                    self.discovered.clear();
                    self.error = format!("Viewer discovery stopped: {error}");
                    return;
                }
            }
        }
    }

    fn update(&mut self, event: DiscoveryEvent) -> bool {
        match event {
            DiscoveryEvent::Updated(viewer) => {
                match self
                    .discovered
                    .iter_mut()
                    .find(|existing| existing.fullname == viewer.fullname)
                {
                    Some(existing) if *existing == viewer => return false,
                    Some(existing) => *existing = viewer,
                    None => self.discovered.push(viewer),
                }
            }
            DiscoveryEvent::Removed(fullname) => {
                let count = self.discovered.len();
                self.discovered.retain(|viewer| viewer.fullname != fullname);
                return self.discovered.len() != count;
            }
        }
        true
    }

    pub(super) fn find(&self, identity: &EndpointIdentity) -> Option<&RemoteViewer> {
        self.discovered.iter().find(|viewer| &viewer.identity() == identity)
    }

    #[cfg(test)]
    pub(super) fn from_receiver(receiver: mdns_sd::Receiver<mdns_sd::ServiceEvent>) -> Self {
        Self { mdns: Some(Discovery::from_receiver(receiver)), ..Default::default() }
    }

    #[cfg(test)]
    pub(super) fn is_browsing(&self) -> bool {
        self.mdns.is_some()
    }
}

/// Finds the USB devices with the Slint viewer firmware of this Slint version, until it's
/// dropped.
pub(super) struct UsbDiscovery {
    receiver: mpsc::UnboundedReceiver<Vec<(SerialIdentity, String)>>,
}

impl UsbDiscovery {
    pub(super) fn start() -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        crate::spawn_local(async move {
            // By port name, `None` if no viewer answered. A port is probed again only once
            // it's unplugged, so a port that didn't open, such as without permission or
            // while `idf.py monitor` holds it, stays hidden until then.
            let mut probed = HashMap::new();
            let mut sent = Vec::new();
            while !sender.is_closed() {
                let endpoints = Self::scan(&mut probed).await;
                if endpoints != sent {
                    let _ = sender.send(endpoints.clone());
                    sent = endpoints;
                }
                tokio::time::sleep(SERIAL_SCAN_INTERVAL).await;
            }
        });
        Self { receiver }
    }

    /// Waits until the plugged-in devices change, and returns them with their port names.
    pub(super) async fn get_endpoints(&mut self) -> Vec<(SerialIdentity, String)> {
        match self.receiver.recv().await {
            Some(endpoints) => endpoints,
            None => std::future::pending().await,
        }
    }

    /// Lists the ports, and probes those that aren't in `probed` yet.
    // TODO: this generated code may need a refactor; keep it isolated in here meanwhile.
    async fn scan(
        probed: &mut HashMap<String, Option<SerialIdentity>>,
    ) -> Vec<(SerialIdentity, String)> {
        // Espressif. Only its ports are probed, since opening a port can reset the device behind it.
        const VIEWER_USB_VENDORS: &[u16] = &[0x303a];
        // Listing the ports blocks, for long on Windows
        let ports = tokio::task::spawn_blocking(usb_serial_ports).await.unwrap_or_default();
        probed.retain(|name, _| ports.iter().any(|port| &port.name == name));
        let new_ports: Vec<_> = ports
            .into_iter()
            .filter(|port| {
                VIEWER_USB_VENDORS.contains(&port.vid) && !probed.contains_key(&port.name)
            })
            .collect();
        // Concurrently, so that a port that doesn't answer doesn't delay the others
        let identities = futures_util::future::join_all(new_ports.iter().map(Self::probe)).await;
        for (port, identity) in new_ports.into_iter().zip(identities) {
            probed.insert(port.name, identity);
        }
        let mut endpoints: Vec<_> = probed
            .iter()
            .filter_map(|(name, identity)| Some((identity.clone()?, name.clone())))
            .collect();
        endpoints.sort_by(|a, b| a.1.cmp(&b.1));
        endpoints
    }

    /// Connects to `port`, and identifies the device if the viewer firmware answers.
    async fn probe(port: &UsbSerialPort) -> Option<SerialIdentity> {
        // Also bounds a device that keeps the session busy, such as by logging without pause
        const PROBE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);
        let (connection, mut events) = SerialConnection::connect(port.name.clone(), None);
        let board = tokio::time::timeout(PROBE_TIMEOUT, async {
            loop {
                match events.recv().await? {
                    SerialEvent::Connected { board, .. } => return Some(board),
                    SerialEvent::Failed(_) => return None,
                    SerialEvent::Message(_) => {}
                }
            }
        })
        .await
        .ok()
        .flatten();
        drop(connection);
        // The session releases the port when it ends, which closes `events`
        let _ =
            tokio::time::timeout(PROBE_TIMEOUT, async { while events.recv().await.is_some() {} })
                .await;
        Some(SerialIdentity { name: board?, hardware_id: port.hardware_id() })
    }

    #[cfg(test)]
    pub(super) fn from_receiver(
        receiver: mpsc::UnboundedReceiver<Vec<(SerialIdentity, String)>>,
    ) -> Self {
        Self { receiver }
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    pub(crate) fn service(name: &str, port: u16) -> mdns_sd::ResolvedService {
        mdns_sd::ServiceInfo::new(
            SERVICE_TYPE,
            name,
            "viewer.local.",
            "127.0.0.1",
            port,
            [(TXT_PROTOCOLS_KEY, PROTOCOL_SUBPROTOCOL), (TXT_SLINT_VERSION_KEY, SLINT_VERSION)]
                .as_slice(),
        )
        .unwrap()
        .as_resolved_service()
    }

    #[tokio::test]
    async fn raw_events_are_normalized_and_removals_preserved() {
        let (sender, receiver) = flume::unbounded();
        let discovery = Discovery::from_receiver(receiver);
        let mut viewer = service("Phone", 1234);
        viewer.addresses.insert("2001:db8::5".parse::<std::net::IpAddr>().unwrap().into());
        viewer.addresses.insert("fe80::1".parse::<std::net::IpAddr>().unwrap().into());
        let fullname = viewer.fullname.clone();
        sender.send(mdns_sd::ServiceEvent::ServiceResolved(Box::new(viewer))).unwrap();
        let DiscoveryEvent::Updated(viewer) = discovery.next_event().await.unwrap() else {
            panic!("Expected viewer update");
        };
        assert_eq!(viewer.name, "Phone");
        assert_eq!(viewer.addresses, ["127.0.0.1", "[2001:db8::5]"]);
        assert!(viewer.unavailable_reason.is_empty());
        sender
            .send(mdns_sd::ServiceEvent::ServiceRemoved(SERVICE_TYPE.into(), fullname.clone()))
            .unwrap();
        assert!(
            matches!(discovery.next_event().await.unwrap(), DiscoveryEvent::Removed(removed) if removed == fullname)
        );
        drop(sender);
        assert!(discovery.next_event().await.is_err());
    }

    #[test]
    fn scoped_ipv6_is_usable_only_with_a_zone() {
        for (address, index, expected) in [
            ("fe80::1", 0, None),
            ("fe80::1", 7, Some("[fe80::1%7]")),
            ("2001:db8::5", 0, Some("[2001:db8::5]")),
            ("2001:db8::5", 7, Some("[2001:db8::5]")),
        ] {
            assert_eq!(usable_ipv6(&address.parse().unwrap(), index).as_deref(), expected);
        }
    }

    #[test]
    fn incompatible_viewers_or_unusable_addresses_are_unavailable() {
        for (protocols, address, port) in [
            (None, "127.0.0.1", 1234),
            (Some("slint-preview.0.1"), "127.0.0.1", 1234),
            (Some(PROTOCOL_SUBPROTOCOL), "fe80::1", 1234),
            (Some(PROTOCOL_SUBPROTOCOL), "127.0.0.1", 0),
        ] {
            let properties =
                protocols.map(|protocols| vec![(TXT_PROTOCOLS_KEY, protocols)]).unwrap_or_default();
            let viewer = mdns_sd::ServiceInfo::new(
                SERVICE_TYPE,
                "Phone",
                "viewer.local.",
                address,
                port,
                properties.as_slice(),
            )
            .unwrap()
            .as_resolved_service();
            assert!(!normalize(viewer).unavailable_reason.is_empty());
        }
    }
}
