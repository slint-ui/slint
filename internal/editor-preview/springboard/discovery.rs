// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::EndpointIdentity;
use super::remote::{Discovery, DiscoveryEvent, RemoteViewer};

#[derive(Default)]
pub(super) struct StatefulDiscovery {
    browse: Option<Discovery>,
    pub(super) error: String,
    pub(super) discovered: Vec<RemoteViewer>,
}

impl StatefulDiscovery {
    pub(super) fn new(browse: crate::Result<Discovery>) -> Self {
        let mut discovery = Self::default();
        discovery.set_browse(browse);
        discovery
    }

    fn set_browse(&mut self, browse: crate::Result<Discovery>) {
        match browse {
            Ok(browse) => {
                self.browse = Some(browse);
                self.error.clear();
            }
            Err(error) => {
                self.browse = None;
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
        let browse = match self.browse.take() {
            Some(mut browse) => browse.restart().map(|()| browse),
            None => Discovery::start(),
        };
        self.set_browse(browse);
    }

    /// Wait until `discovered` or `error` changes. Pending while not browsing.
    pub(super) async fn changed(&mut self) {
        loop {
            let Some(browse) = &self.browse else { return std::future::pending().await };
            let event = browse.next_event().await;
            match event {
                Ok(event) => {
                    if self.update(event) {
                        return;
                    }
                }
                Err(error) => {
                    self.browse = None;
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
    pub(super) fn is_browsing(&self) -> bool {
        self.browse.is_some()
    }
}
