// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use crate::screen::{Screen, ScreenCallbackHelper, ScreenContext, ScreenWithHeader};
use crate::store::AppStore;
use slint::SharedString;
use slint_interpreter::{ComponentInstance, Value};
use std::rc::Rc;

pub struct OrderDetailScreen {
    store: Rc<AppStore>,
}

impl OrderDetailScreen {
    pub fn create(store: Rc<AppStore>) -> Box<dyn Screen> {
        println!("[OrderDetailScreen] Controller CREATED");
        Box::new(Self { store })
    }
}

impl Drop for OrderDetailScreen {
    fn drop(&mut self) {
        println!("[OrderDetailScreen] Controller DESTROYED");
    }
}

impl Screen for OrderDetailScreen {
    fn name(&self) -> &'static str {
        "order-detail"
    }

    fn on_loaded(&self, instance: &ComponentInstance, ctx: &ScreenContext) {
        ScreenCallbackHelper::setup_navigate(instance, ctx);

        // Get selected order ID from store
        if let Some(id) = self.store.get::<String>("selected_order_id") {
            let _ = instance.set_property("order-id", Value::String(SharedString::from(&id)));

            // Mock data based on ID
            let (status, date, total, address) = match id.as_str() {
                "ord-001" => {
                    ("Delivered", "January 15, 2024", "$299.99", "123 Main St, City, 12345")
                }
                "ord-002" => {
                    ("In Transit", "January 18, 2024", "$149.50", "456 Oak Ave, Town, 67890")
                }
                "ord-003" => {
                    ("Processing", "January 20, 2024", "$89.00", "789 Pine Rd, Village, 11111")
                }
                "ord-004" => {
                    ("Delivered", "January 10, 2024", "$450.00", "321 Elm Blvd, Metro, 22222")
                }
                _ => ("Unknown", "Unknown", "$0.00", "Unknown"),
            };

            let _ =
                instance.set_property("order-status", Value::String(SharedString::from(status)));
            let _ = instance.set_property("order-date", Value::String(SharedString::from(date)));
            let _ = instance.set_property("order-total", Value::String(SharedString::from(total)));
            let _ = instance
                .set_property("shipping-address", Value::String(SharedString::from(address)));
        }

        println!("[OrderDetailScreen] on_loaded");
    }
}

impl ScreenWithHeader for OrderDetailScreen {
    fn title(&self) -> &str {
        "Order Details"
    }
}
