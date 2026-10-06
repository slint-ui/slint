// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_live_preview::protocol::{
    PROTOCOL_SUBPROTOCOL, SERVICE_TYPE, SLINT_VERSION, TXT_PROTOCOLS_KEY, TXT_SLINT_VERSION_KEY,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct DiscoveredViewer {
    pub fullname: String,
    pub name: String,
    pub addresses: Vec<String>,
    pub port: u16,
    pub unavailable_reason: String,
}

#[derive(Debug)]
pub(super) enum DiscoveryEvent {
    Updated(DiscoveredViewer),
    Removed(String),
}

pub(super) struct Discovery {
    daemon: Option<mdns_sd::ServiceDaemon>,
    receiver: mdns_sd::Receiver<mdns_sd::ServiceEvent>,
}

impl Discovery {
    pub fn start() -> crate::Result<Self> {
        let daemon = mdns_sd::ServiceDaemon::new()?;
        match daemon.browse(SERVICE_TYPE) {
            Ok(receiver) => Ok(Self { daemon: Some(daemon), receiver }),
            Err(error) => {
                let _ = daemon.shutdown();
                Err(error.into())
            }
        }
    }

    pub async fn next_event(&self) -> crate::Result<DiscoveryEvent> {
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
    pub(super) fn from_receiver(receiver: mdns_sd::Receiver<mdns_sd::ServiceEvent>) -> Self {
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

fn normalize(viewer: mdns_sd::ResolvedService) -> DiscoveredViewer {
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
            mdns_sd::ScopedIp::V6(address) => dialable_v6(address.addr(), address.scope_id().index),
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
    DiscoveredViewer {
        fullname: viewer.fullname,
        name,
        addresses,
        port: viewer.port,
        unavailable_reason,
    }
}

fn dialable_v6(address: &std::net::Ipv6Addr, scope_index: u32) -> Option<String> {
    if address.is_unicast_link_local() {
        (scope_index != 0).then(|| format!("[{address}%{scope_index}]"))
    } else {
        Some(format!("[{address}]"))
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
    fn scoped_ipv6_is_dialable_only_with_a_zone() {
        for (address, index, expected) in [
            ("fe80::1", 0, None),
            ("fe80::1", 7, Some("[fe80::1%7]")),
            ("2001:db8::5", 0, Some("[2001:db8::5]")),
            ("2001:db8::5", 7, Some("[2001:db8::5]")),
        ] {
            assert_eq!(dialable_v6(&address.parse().unwrap(), index).as_deref(), expected);
        }
    }

    #[test]
    fn incompatible_or_undialable_viewers_are_unavailable() {
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
