// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::{cell::RefCell, rc::Rc};

#[cfg(not(target_vendor = "apple"))]
pub use mdns_sd::ResolvedService;

#[cfg(target_vendor = "apple")]
pub type ResolvedService = zeroconf_tokio::ServiceDiscovery;

#[derive(Clone, Default)]
pub struct ServiceModelController {
    services: RefCell<Vec<ResolvedService>>,
    notify: Rc<slint::ModelNotify>,
}

impl ServiceModelController {
    pub fn insert(&self, service: ResolvedService) {
        let mut services = self.services.borrow_mut();
        if !services.iter().any(|s| Self::compare_service(s, &service)) {
            services.push(service);
            self.notify.row_added(services.len(), 1);
        }
    }

    #[cfg(target_vendor = "apple")]
    pub fn remove(&self, removal: &zeroconf_tokio::ServiceRemoval) {
        let mut services = self.services.borrow_mut();
        for (index, service) in services.iter().enumerate() {
            let service_type = service.service_type();
            if service_type.name() == removal.name()
                && service_type.protocol() == removal.kind()
                && service.domain() == removal.domain()
            {
                services.remove(index);
                self.notify.row_removed(index, 1);
                return;
            }
        }
    }

    #[cfg(not(target_vendor = "apple"))]
    pub fn remove(&self, fullname: &str) {
        let mut services = self.services.borrow_mut();
        for (index, service) in services.iter().enumerate() {
            if service.get_fullname() == fullname {
                services.remove(index);
                self.notify.row_removed(index, 1);
                return;
            }
        }
    }

    fn compare_service(one: &ResolvedService, two: &ResolvedService) -> bool {
        #[cfg(target_vendor = "apple")]
        {
            let one_type = one.service_type();
            let other_type = two.service_type();
            one_type.name() == other_type.name()
                && one_type.protocol() == other_type.protocol()
                && one.domain() == two.domain()
        }
        #[cfg(not(target_vendor = "apple"))]
        {
            one.get_fullname() == two.get_fullname()
        }
    }
}

impl slint::Model for ServiceModelController {
    type Data = ResolvedService;

    fn row_count(&self) -> usize {
        self.services.borrow().len()
    }

    fn row_data(&self, row: usize) -> Option<Self::Data> {
        self.services.borrow().get(row).cloned()
    }

    fn model_tracker(&self) -> &dyn slint::ModelTracker {
        self.notify.as_ref()
    }
}
