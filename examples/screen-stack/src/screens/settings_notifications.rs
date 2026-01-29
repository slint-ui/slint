// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use crate::screen::{Screen, ScreenCallbackHelper, ScreenContext, ScreenWithHeader};
use crate::store::AppStore;
use slint_interpreter::{ComponentInstance, Value};
use std::rc::Rc;

const STORE_KEY: &str = "notifications";

#[derive(Debug, Clone)]
pub struct NotificationsModel {
    pub enabled: bool,
    pub sound: bool,
    pub vibration: bool,
    pub badges: bool,
}

impl Default for NotificationsModel {
    fn default() -> Self {
        Self { enabled: true, sound: true, vibration: true, badges: true }
    }
}

pub struct SettingsNotificationsScreen {
    store: Rc<AppStore>,
}

impl SettingsNotificationsScreen {
    pub fn create(store: Rc<AppStore>) -> Box<dyn Screen> {
        println!("[SettingsNotificationsScreen] Controller CREATED");
        Box::new(Self { store })
    }
}

impl Drop for SettingsNotificationsScreen {
    fn drop(&mut self) {
        println!("[SettingsNotificationsScreen] Controller DESTROYED");
    }
}

impl Screen for SettingsNotificationsScreen {
    fn name(&self) -> &'static str {
        "settings-notifications"
    }

    fn on_loaded(&self, instance: &ComponentInstance, ctx: &ScreenContext) {
        let m = self.store.get_or_default::<NotificationsModel>(STORE_KEY);
        let _ = instance.set_property("notifications", Value::Bool(m.enabled));
        let _ = instance.set_property("sound-enabled", Value::Bool(m.sound));
        let _ = instance.set_property("vibration", Value::Bool(m.vibration));
        let _ = instance.set_property("badges", Value::Bool(m.badges));
        ScreenCallbackHelper::setup_navigate(instance, ctx);
        println!("[SettingsNotificationsScreen] on_loaded");
    }

    fn on_unload(&self, instance: &ComponentInstance) {
        let mut m = NotificationsModel::default();
        if let Ok(Value::Bool(v)) = instance.get_property("notifications") {
            m.enabled = v;
        }
        if let Ok(Value::Bool(v)) = instance.get_property("sound-enabled") {
            m.sound = v;
        }
        if let Ok(Value::Bool(v)) = instance.get_property("vibration") {
            m.vibration = v;
        }
        if let Ok(Value::Bool(v)) = instance.get_property("badges") {
            m.badges = v;
        }
        self.store.set(STORE_KEY, m);
        println!("[SettingsNotificationsScreen] on_unload");
    }
}

impl ScreenWithHeader for SettingsNotificationsScreen {
    fn title(&self) -> &str {
        "Notifications"
    }
}
