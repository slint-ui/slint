// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use crate::screen::{Screen, ScreenCallbackHelper, ScreenContext, ScreenWithHeader};
use crate::store::AppStore;
use slint::SharedString;
use slint_interpreter::{ComponentInstance, Value};
use std::rc::Rc;

pub struct ContactDetailScreen {
    store: Rc<AppStore>,
}

impl ContactDetailScreen {
    pub fn create(store: Rc<AppStore>) -> Box<dyn Screen> {
        println!("[ContactDetailScreen] Controller CREATED");
        Box::new(Self { store })
    }
}

impl Drop for ContactDetailScreen {
    fn drop(&mut self) {
        println!("[ContactDetailScreen] Controller DESTROYED");
    }
}

impl Screen for ContactDetailScreen {
    fn name(&self) -> &'static str {
        "contact-detail"
    }

    fn on_loaded(&self, instance: &ComponentInstance, ctx: &ScreenContext) {
        ScreenCallbackHelper::setup_navigate(instance, ctx);

        // Get selected contact ID from store
        if let Some(id) = self.store.get::<String>("selected_contact_id") {
            println!("[ContactDetailScreen] Loading contact ID: {}", id);
            let _ = instance.set_property("contact-id", Value::String(SharedString::from(&id)));

            // In real app, fetch contact data from store/database
            // For demo, use mock data based on ID
            let (name, initials, email, phone, company) = match id.as_str() {
                "1" => ("Alice Smith", "A", "alice@example.com", "+1 234 567 8901", "Acme Corp"),
                "2" => ("Bob Johnson", "B", "bob@example.com", "+1 234 567 8902", "Tech Inc"),
                "3" => ("Carol White", "C", "carol@example.com", "+1 234 567 8903", "Design Co"),
                "4" => ("David Brown", "D", "david@example.com", "+1 234 567 8904", "Data Inc"),
                _ => ("Unknown", "?", "unknown@example.com", "+1 000 000 0000", "Unknown"),
            };

            let _ = instance.set_property("contact-name", Value::String(SharedString::from(name)));
            let _ = instance
                .set_property("contact-initials", Value::String(SharedString::from(initials)));
            let _ =
                instance.set_property("contact-email", Value::String(SharedString::from(email)));
            let _ =
                instance.set_property("contact-phone", Value::String(SharedString::from(phone)));
            let _ = instance
                .set_property("contact-company", Value::String(SharedString::from(company)));
        }

        let _ = instance.set_callback("call-contact", move |_| {
            println!("[ContactDetailScreen] Call contact");
            Value::Void
        });

        let _ = instance.set_callback("email-contact", move |_| {
            println!("[ContactDetailScreen] Email contact");
            Value::Void
        });

        println!("[ContactDetailScreen] on_loaded");
    }
}

impl ScreenWithHeader for ContactDetailScreen {
    fn title(&self) -> &str {
        "Contact"
    }
}
