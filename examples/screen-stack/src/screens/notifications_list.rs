// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use crate::screen::{Screen, ScreenCallbackHelper, ScreenContext, ScreenWithHeader};
use crate::store::AppStore;
use slint::{ModelRc, SharedString, VecModel};
use slint_interpreter::{ComponentInstance, Struct, Value};
use std::rc::Rc;

pub struct Notification {
    pub id: String,
    pub icon: String,
    pub title: String,
    pub subtitle: String,
    pub has_arrow: bool,
}

pub struct NotificationsListScreen {
    store: Rc<AppStore>,
    notifications: Vec<Notification>,
}

impl NotificationsListScreen {
    pub fn create(store: Rc<AppStore>) -> Box<dyn Screen> {
        println!("[NotificationsListScreen] Controller CREATED");

        let notifications = vec![
            Notification {
                id: "n1".into(),
                icon: "🔔".into(),
                title: "New message".into(),
                subtitle: "You have a new message from Alice".into(),
                has_arrow: true,
            },
            Notification {
                id: "n2".into(),
                icon: "📢".into(),
                title: "System update".into(),
                subtitle: "A new version is available".into(),
                has_arrow: true,
            },
            Notification {
                id: "n3".into(),
                icon: "🎉".into(),
                title: "Welcome!".into(),
                subtitle: "Thanks for using Screen Stack Demo".into(),
                has_arrow: false,
            },
        ];

        Box::new(Self { store, notifications })
    }

    fn to_list_item(notif: &Notification) -> Value {
        let mut item = Struct::default();
        item.set_field("id".into(), Value::String(SharedString::from(&notif.id)));
        item.set_field("icon".into(), Value::String(SharedString::from(&notif.icon)));
        item.set_field("title".into(), Value::String(SharedString::from(&notif.title)));
        item.set_field("subtitle".into(), Value::String(SharedString::from(&notif.subtitle)));
        item.set_field("has-arrow".into(), Value::Bool(notif.has_arrow));
        Value::Struct(item)
    }
}

impl Drop for NotificationsListScreen {
    fn drop(&mut self) {
        println!("[NotificationsListScreen] Controller DESTROYED");
    }
}

impl Screen for NotificationsListScreen {
    fn name(&self) -> &'static str {
        "notifications-list"
    }

    fn on_loaded(&self, instance: &ComponentInstance, ctx: &ScreenContext) {
        ScreenCallbackHelper::setup_navigate(instance, ctx);

        // Set notifications from Rust
        let items: Vec<Value> = self.notifications.iter().map(Self::to_list_item).collect();
        let model = ModelRc::new(VecModel::from(items));
        let _ = instance.set_property("notifications", Value::Model(model));

        // Handle item clicks - save ID and navigate to detail
        let store = self.store.clone();
        let ctx = ctx.clone();
        let _ = instance.set_callback("item-clicked", move |args| {
            if let Some(Value::String(id)) = args.first() {
                println!("[NotificationsListScreen] Notification clicked: {}", id);
                store.set("selected_notification_id", id.to_string());
                ctx.push("notification-detail");
            }
            Value::Void
        });

        println!(
            "[NotificationsListScreen] on_loaded with {} notifications",
            self.notifications.len()
        );
    }
}

impl ScreenWithHeader for NotificationsListScreen {
    fn title(&self) -> &str {
        "Notifications"
    }
}
