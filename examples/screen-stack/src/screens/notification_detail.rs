// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use crate::screen::{Screen, ScreenCallbackHelper, ScreenContext, ScreenWithHeader};
use crate::store::AppStore;
use slint::SharedString;
use slint_interpreter::{ComponentInstance, Value};
use std::rc::Rc;

pub struct NotificationDetailScreen {
    store: Rc<AppStore>,
}

impl NotificationDetailScreen {
    pub fn create(store: Rc<AppStore>) -> Box<dyn Screen> {
        println!("[NotificationDetailScreen] Controller CREATED");
        Box::new(Self { store })
    }
}

impl Drop for NotificationDetailScreen {
    fn drop(&mut self) {
        println!("[NotificationDetailScreen] Controller DESTROYED");
    }
}

impl Screen for NotificationDetailScreen {
    fn name(&self) -> &'static str {
        "notification-detail"
    }

    fn on_loaded(&self, instance: &ComponentInstance, ctx: &ScreenContext) {
        ScreenCallbackHelper::setup_navigate(instance, ctx);

        // Get selected notification ID from store
        if let Some(id) = self.store.get::<String>("selected_notification_id") {
            let _ =
                instance.set_property("notification-id", Value::String(SharedString::from(&id)));

            // Mock data based on ID
            let (icon, title, body, time) = match id.as_str() {
                "n1" => ("💬", "New message", "You have a new message from Alice. Tap to view the full conversation and reply to her.", "2 hours ago"),
                "n2" => ("📢", "System update", "A new version of the app is available. Update now to get the latest features and improvements.", "5 hours ago"),
                "n3" => ("🎉", "Welcome!", "Thanks for using Screen Stack Demo. Explore the app to see how screen navigation works.", "1 day ago"),
                _ => ("🔔", "Notification", "No details available.", "Unknown"),
            };

            let _ =
                instance.set_property("notification-icon", Value::String(SharedString::from(icon)));
            let _ = instance
                .set_property("notification-title", Value::String(SharedString::from(title)));
            let _ =
                instance.set_property("notification-body", Value::String(SharedString::from(body)));
            let _ =
                instance.set_property("notification-time", Value::String(SharedString::from(time)));
        }

        let _ = instance.set_callback("mark-as-read", move |_| {
            println!("[NotificationDetailScreen] Mark as read");
            Value::Void
        });

        let _ = instance.set_callback("dismiss", move |_| {
            println!("[NotificationDetailScreen] Dismiss");
            Value::Void
        });

        println!("[NotificationDetailScreen] on_loaded");
    }
}

impl ScreenWithHeader for NotificationDetailScreen {
    fn title(&self) -> &str {
        "Notification"
    }
}
