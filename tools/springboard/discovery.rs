// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Continuous discovery of remote Slint viewers on the trusted local network.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context as _, Result};
use i_slint_live_preview::protocol::{
    PROTOCOL_SUBPROTOCOL, SERVICE_TYPE, SLINT_VERSION, TXT_DEVICE_ID_KEY, TXT_PLATFORM_KEY,
    TXT_PROTOCOLS_KEY, TXT_SLINT_VERSION_KEY,
};
use i_slint_springboard::{
    Device, DeviceCapabilities, DeviceId, DeviceKind, DeviceOrigin, DeviceStatus,
};
use mdns_sd::{ResolvedService, ServiceDaemon, ServiceEvent};
use tokio::sync::mpsc;

const REMOTE_DEVICE_PREFIX: &str = "remote:";

/// A resolved remote viewer with all currently advertised endpoints.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoveredRemoteViewer {
    pub id: DeviceId,
    pub name: String,
    pub origin: DeviceOrigin,
    pub platform: String,
    pub slint_version: Option<String>,
    pub protocols: Vec<String>,
    pub addresses: Vec<String>,
    pub port: u16,
}

impl DiscoveredRemoteViewer {
    pub fn compatible(&self) -> bool {
        self.protocols.iter().any(|protocol| protocol == PROTOCOL_SUBPROTOCOL)
    }

    pub fn to_device(&self) -> Device {
        let compatible = self.compatible();
        Device {
            id: self.id.clone(),
            name: self.name.clone(),
            kind: DeviceKind::RemoteViewer,
            origin: self.origin,
            status: if compatible {
                DeviceStatus::Available
            } else {
                DeviceStatus::Incompatible {
                    installed: self.slint_version.clone().unwrap_or_else(|| "unknown".into()),
                    required: SLINT_VERSION.into(),
                }
            },
            capabilities: DeviceCapabilities {
                launch: compatible,
                stop: true,
                refresh: true,
                reconnect: compatible,
                rebuild: false,
            },
            version: self.slint_version.clone(),
            platform: Some(self.platform.clone()),
        }
    }

    pub fn endpoint_strings(&self) -> Vec<String> {
        self.addresses.iter().map(|address| format_endpoint(address, self.port)).collect()
    }

    pub fn manual(address: &str) -> Result<Self> {
        let (host, port) = parse_endpoint(address)?;
        let endpoint = format_endpoint(&host, port);
        Ok(Self {
            id: DeviceId::new(format!("manual:{endpoint}"))?,
            name: endpoint,
            origin: DeviceOrigin::Manual,
            platform: "manual".into(),
            slint_version: None,
            protocols: vec![PROTOCOL_SUBPROTOCOL.into()],
            addresses: vec![host],
            port,
        })
    }
}

pub fn format_endpoint(address: &str, port: u16) -> String {
    let address =
        address.strip_prefix('[').and_then(|value| value.strip_suffix(']')).unwrap_or(address);
    if address.contains(':') { format!("[{address}]:{port}") } else { format!("{address}:{port}") }
}

pub fn parse_endpoint(address: &str) -> Result<(String, u16)> {
    let address = address.trim();
    let (host, port) = if let Some(rest) = address.strip_prefix('[') {
        let (host, port) = rest.split_once("]:").with_context(|| {
            format!("Invalid remote viewer address {address}; expected [host]:port")
        })?;
        (host, port)
    } else {
        address.rsplit_once(':').with_context(|| {
            format!("Invalid remote viewer address {address}; expected host:port")
        })?
    };
    if host.is_empty() || host.chars().any(char::is_whitespace) || host.contains('/') {
        anyhow::bail!("Invalid remote viewer host in {address}");
    }
    let port =
        port.parse::<u16>().with_context(|| format!("Invalid remote viewer port in {address}"))?;
    if port == 0 {
        anyhow::bail!("Remote viewer port must not be zero");
    }
    Ok((host.to_owned(), port))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RemoteDiscoveryEvent {
    Upsert(DiscoveredRemoteViewer),
    Removed(DeviceId),
    Warning(String),
}

pub struct RemoteViewerDiscovery {
    daemon: ServiceDaemon,
    task: tokio::task::JoinHandle<()>,
    events: mpsc::UnboundedReceiver<RemoteDiscoveryEvent>,
}

impl RemoteViewerDiscovery {
    pub fn start() -> Result<Self> {
        let daemon = ServiceDaemon::new().context("Failed to start local-network discovery")?;
        let receiver =
            daemon.browse(SERVICE_TYPE).context("Failed to browse for Slint remote viewers")?;
        let (event_sender, events) = mpsc::unbounded_channel();
        let task = tokio::spawn(async move {
            let mut registry = DiscoveryRegistry::default();
            while let Ok(event) = receiver.recv_async().await {
                let events = match event {
                    ServiceEvent::ServiceResolved(service) => {
                        match announcement(service.as_ref()) {
                            Ok(announcement) => registry.resolve(announcement),
                            Err(warning) => vec![RemoteDiscoveryEvent::Warning(warning)],
                        }
                    }
                    ServiceEvent::ServiceRemoved(_, fullname) => registry.remove(&fullname),
                    ServiceEvent::SearchStopped(_) => break,
                    _ => Vec::new(),
                };
                for event in events {
                    if event_sender.send(event).is_err() {
                        return;
                    }
                }
            }
        });
        Ok(Self { daemon, task, events })
    }

    pub fn take_events(&mut self) -> Vec<RemoteDiscoveryEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.events.try_recv() {
            events.push(event);
        }
        events
    }
}

impl Drop for RemoteViewerDiscovery {
    fn drop(&mut self) {
        self.task.abort();
        self.daemon.stop_browse(SERVICE_TYPE).ok();
        self.daemon.shutdown().ok();
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RemoteViewerAnnouncement {
    fullname: String,
    device_id: String,
    name: String,
    platform: String,
    slint_version: Option<String>,
    protocols: Vec<String>,
    addresses: Vec<String>,
    port: u16,
}

fn announcement(service: &ResolvedService) -> Result<RemoteViewerAnnouncement, String> {
    let device_id = service
        .get_property_val_str(TXT_DEVICE_ID_KEY)
        .filter(|id| !id.trim().is_empty())
        .ok_or_else(|| {
            format!("Ignoring {} because it has no persistent device ID", service.fullname)
        })?;
    let name = service
        .fullname
        .strip_suffix(SERVICE_TYPE)
        .unwrap_or(&service.fullname)
        .trim_end_matches('.')
        .to_owned();
    let platform = service
        .get_property_val_str(TXT_PLATFORM_KEY)
        .filter(|platform| !platform.trim().is_empty())
        .unwrap_or("unknown")
        .to_owned();
    let slint_version = service
        .get_property_val_str(TXT_SLINT_VERSION_KEY)
        .filter(|version| !version.trim().is_empty())
        .map(str::to_owned);
    let protocols = service
        .get_property_val_str(TXT_PROTOCOLS_KEY)
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|protocol| !protocol.is_empty())
        .map(str::to_owned)
        .collect();
    let mut addresses = service.addresses.iter().map(ToString::to_string).collect::<Vec<_>>();
    addresses.sort();
    addresses.dedup();
    Ok(RemoteViewerAnnouncement {
        fullname: service.fullname.clone(),
        device_id: device_id.to_owned(),
        name,
        platform,
        slint_version,
        protocols,
        addresses,
        port: service.port,
    })
}

#[derive(Default)]
struct DiscoveryRegistry {
    announcements: BTreeMap<String, RemoteViewerAnnouncement>,
}

impl DiscoveryRegistry {
    fn resolve(&mut self, announcement: RemoteViewerAnnouncement) -> Vec<RemoteDiscoveryEvent> {
        let fullname = announcement.fullname.clone();
        let previous_id =
            self.announcements.get(&fullname).map(|announcement| announcement.device_id.clone());
        let mut affected = BTreeSet::from([announcement.device_id.clone()]);
        if let Some(previous_id) = previous_id {
            affected.insert(previous_id);
        }
        let before =
            affected.iter().map(|id| (id.clone(), self.aggregate(id))).collect::<BTreeMap<_, _>>();
        self.announcements.insert(fullname, announcement);
        self.changed_events(affected, before)
    }

    fn remove(&mut self, fullname: &str) -> Vec<RemoteDiscoveryEvent> {
        let Some(announcement) = self.announcements.get(fullname) else {
            return Vec::new();
        };
        let device_id = announcement.device_id.clone();
        let before = BTreeMap::from([(device_id.clone(), self.aggregate(&device_id))]);
        self.announcements.remove(fullname);
        self.changed_events(BTreeSet::from([device_id]), before)
    }

    fn changed_events(
        &self,
        affected: BTreeSet<String>,
        before: BTreeMap<String, Option<DiscoveredRemoteViewer>>,
    ) -> Vec<RemoteDiscoveryEvent> {
        affected
            .into_iter()
            .filter_map(|id| {
                let after = self.aggregate(&id);
                if before.get(&id) == Some(&after) {
                    return None;
                }
                match after {
                    Some(viewer) => Some(RemoteDiscoveryEvent::Upsert(viewer)),
                    None => DeviceId::new(format!("{REMOTE_DEVICE_PREFIX}{id}"))
                        .ok()
                        .map(RemoteDiscoveryEvent::Removed),
                }
            })
            .collect()
    }

    fn aggregate(&self, device_id: &str) -> Option<DiscoveredRemoteViewer> {
        let mut matching =
            self.announcements.values().filter(|announcement| announcement.device_id == device_id);
        let first = matching.next()?.clone();
        let mut addresses = first.addresses.clone();
        for announcement in matching {
            addresses.extend(announcement.addresses.iter().cloned());
        }
        addresses.sort();
        addresses.dedup();
        Some(DiscoveredRemoteViewer {
            id: DeviceId::new(format!("{REMOTE_DEVICE_PREFIX}{device_id}")).ok()?,
            name: first.name,
            origin: DeviceOrigin::Discovered,
            platform: first.platform,
            slint_version: first.slint_version,
            protocols: first.protocols,
            addresses,
            port: first.port,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn announcement(fullname: &str, id: &str, address: &str) -> RemoteViewerAnnouncement {
        RemoteViewerAnnouncement {
            fullname: fullname.into(),
            device_id: id.into(),
            name: fullname.split('.').next().unwrap().into(),
            platform: "ios".into(),
            slint_version: Some(SLINT_VERSION.into()),
            protocols: vec![PROTOCOL_SUBPROTOCOL.into()],
            addresses: vec![address.into()],
            port: 41000,
        }
    }

    fn upsert(events: &[RemoteDiscoveryEvent]) -> &DiscoveredRemoteViewer {
        let [RemoteDiscoveryEvent::Upsert(viewer)] = events else { panic!("expected one upsert") };
        viewer
    }

    #[test]
    fn changed_addresses_replace_the_same_instance() {
        let mut registry = DiscoveryRegistry::default();
        registry.resolve(announcement("phone._slint-preview._tcp.local.", "phone-id", "10.0.0.2"));

        let events = registry.resolve(announcement(
            "phone._slint-preview._tcp.local.",
            "phone-id",
            "10.0.0.9",
        ));

        assert_eq!(upsert(&events).addresses, ["10.0.0.9"]);
    }

    #[test]
    fn duplicate_announcements_merge_by_stable_id() {
        let mut registry = DiscoveryRegistry::default();
        registry.resolve(announcement("phone._slint-preview._tcp.local.", "phone-id", "10.0.0.2"));

        let events = registry.resolve(announcement(
            "phone-2._slint-preview._tcp.local.",
            "phone-id",
            "fe80::1234%en0",
        ));

        assert_eq!(upsert(&events).addresses, ["10.0.0.2", "fe80::1234%en0"]);
    }

    #[test]
    fn expiry_waits_for_the_last_announcement() {
        let mut registry = DiscoveryRegistry::default();
        registry.resolve(announcement("phone._slint-preview._tcp.local.", "phone-id", "10.0.0.2"));
        registry.resolve(announcement(
            "phone-2._slint-preview._tcp.local.",
            "phone-id",
            "10.0.0.3",
        ));

        let first = registry.remove("phone._slint-preview._tcp.local.");
        assert_eq!(upsert(&first).addresses, ["10.0.0.3"]);
        let second = registry.remove("phone-2._slint-preview._tcp.local.");
        assert_eq!(
            second,
            [RemoteDiscoveryEvent::Removed(DeviceId::new("remote:phone-id").unwrap())]
        );
    }

    #[test]
    fn incompatible_viewers_remain_visible() {
        let mut registry = DiscoveryRegistry::default();
        let mut old = announcement("old._slint-preview._tcp.local.", "old-id", "10.0.0.4");
        old.slint_version = Some("1.17.2".into());
        old.protocols = vec!["slint-preview.1.17".into()];

        let viewer = upsert(&registry.resolve(old)).clone();

        let device = viewer.to_device();
        assert_eq!(
            device.status,
            DeviceStatus::Incompatible {
                installed: "1.17.2".into(),
                required: SLINT_VERSION.into()
            }
        );
        assert!(!device.capabilities.launch);
    }

    #[test]
    fn manual_endpoints_support_hostnames_and_ipv6() {
        assert_eq!(parse_endpoint("viewer.local:41000").unwrap(), ("viewer.local".into(), 41000));
        assert_eq!(parse_endpoint("[fe80::1%en0]:41000").unwrap(), ("fe80::1%en0".into(), 41000));
        assert_eq!(format_endpoint("fe80::1%en0", 41000), "[fe80::1%en0]:41000");
        assert!(parse_endpoint("viewer.local").is_err());
        assert!(parse_endpoint("viewer.local:0").is_err());
        assert!(parse_endpoint("ws://viewer.local:41000").is_err());

        let viewer = DiscoveredRemoteViewer::manual("viewer.local:41000").unwrap();
        assert_eq!(viewer.id.as_str(), "manual:viewer.local:41000");
        assert_eq!(viewer.origin, DeviceOrigin::Manual);
        assert_eq!(viewer.endpoint_strings(), ["viewer.local:41000"]);
    }
}
