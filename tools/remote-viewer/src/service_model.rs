use std::{cell::RefCell, rc::Rc};

use mdns_sd::ResolvedService;

#[derive(Clone, Default)]
pub struct ServiceModelController {
    services: RefCell<Vec<ResolvedService>>,
    notify: Rc<slint::ModelNotify>,
}

impl ServiceModelController {
    pub fn insert(&self, service: ResolvedService) {
        let mut services = self.services.borrow_mut();
        services.push(service);
        self.notify.row_added(services.len(), 1);
    }

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
