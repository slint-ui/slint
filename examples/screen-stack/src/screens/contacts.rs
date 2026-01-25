// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use crate::screen::{Screen, ScreenCallbackHelper, ScreenContext, ScreenWithHeader};
use crate::store::AppStore;
use slint::{ModelRc, SharedString, VecModel};
use slint_interpreter::{ComponentInstance, Struct, Value};
use std::rc::Rc;

/// Contact data model
#[derive(Clone)]
pub struct Contact {
    pub id: String,
    pub name: String,
    pub email: String,
}

pub struct ContactsScreen {
    store: Rc<AppStore>,
    contacts: Vec<Contact>,
}

impl ContactsScreen {
    pub fn create(store: Rc<AppStore>) -> Box<dyn Screen> {
        println!("[ContactsScreen] Controller CREATED");

        // Model data - could come from database, API, etc.
        let contacts = vec![
            Contact {
                id: "1".into(),
                name: "Alice Smith".into(),
                email: "alice@example.com".into(),
            },
            Contact { id: "2".into(), name: "Bob Johnson".into(), email: "bob@example.com".into() },
            Contact {
                id: "3".into(),
                name: "Carol White".into(),
                email: "carol@example.com".into(),
            },
            Contact {
                id: "4".into(),
                name: "David Brown".into(),
                email: "david@example.com".into(),
            },
        ];

        Box::new(Self { store, contacts })
    }

    /// Convert Contact to ListItem struct for Slint
    fn to_list_item(contact: &Contact) -> Value {
        let mut item = Struct::default();
        item.set_field("id".into(), Value::String(SharedString::from(&contact.id)));
        item.set_field("icon".into(), Value::String("👤".into()));
        item.set_field("title".into(), Value::String(SharedString::from(&contact.name)));
        item.set_field("subtitle".into(), Value::String(SharedString::from(&contact.email)));
        item.set_field("has-arrow".into(), Value::Bool(true));
        Value::Struct(item)
    }
}

impl Drop for ContactsScreen {
    fn drop(&mut self) {
        println!("[ContactsScreen] Controller DESTROYED");
    }
}

impl Screen for ContactsScreen {
    fn name(&self) -> &'static str {
        "contacts"
    }

    fn on_loaded(&self, instance: &ComponentInstance, ctx: &ScreenContext) {
        ScreenCallbackHelper::setup_navigate(instance, ctx);

        // Set contacts from Rust
        let items: Vec<Value> = self.contacts.iter().map(Self::to_list_item).collect();
        let model = ModelRc::new(VecModel::from(items));
        let _ = instance.set_property("contacts", Value::Model(model));

        // Handle item clicks - save ID and navigate to detail
        let store = self.store.clone();
        let ctx = ctx.clone();
        let _ = instance.set_callback("item-clicked", move |args| {
            if let Some(Value::String(id)) = args.first() {
                println!("[ContactsScreen] Item clicked: {}", id);
                store.set("selected_contact_id", id.to_string());
                ctx.push("contact-detail");
            }
            Value::Void
        });

        println!("[ContactsScreen] on_loaded with {} contacts", self.contacts.len());
    }
}

impl ScreenWithHeader for ContactsScreen {
    fn title(&self) -> &str {
        "Contacts"
    }
}
