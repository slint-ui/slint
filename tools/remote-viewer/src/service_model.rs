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
        if !services.contains(&service) {
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
