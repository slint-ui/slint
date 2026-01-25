// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use crate::screen::{Screen, ScreenCallbackHelper, ScreenContext, ScreenWithHeader};
use crate::store::AppStore;
use slint_interpreter::{ComponentInstance, Value};
use std::rc::Rc;

const STORE_KEY: &str = "appearance";

#[derive(Debug, Clone, Default)]
pub struct AppearanceModel {
    pub dark_mode: bool,
}

pub struct SettingsAppearanceScreen {
    store: Rc<AppStore>,
}

impl SettingsAppearanceScreen {
    pub fn create(store: Rc<AppStore>) -> Box<dyn Screen> {
        println!("[SettingsAppearanceScreen] Controller CREATED");
        Box::new(Self { store })
    }
}

impl Drop for SettingsAppearanceScreen {
    fn drop(&mut self) {
        println!("[SettingsAppearanceScreen] Controller DESTROYED");
    }
}

impl Screen for SettingsAppearanceScreen {
    fn name(&self) -> &'static str {
        "settings-appearance"
    }

    fn on_loaded(&self, instance: &ComponentInstance, ctx: &ScreenContext) {
        let m = self.store.get_or_default::<AppearanceModel>(STORE_KEY);
        let _ = instance.set_property("dark-mode", Value::Bool(m.dark_mode));
        ScreenCallbackHelper::setup_navigate(instance, ctx);
        println!("[SettingsAppearanceScreen] on_loaded");
    }

    fn on_unload(&self, instance: &ComponentInstance) {
        let mut m = self.store.get_or_default::<AppearanceModel>(STORE_KEY);
        if let Ok(Value::Bool(v)) = instance.get_property("dark-mode") {
            m.dark_mode = v;
        }
        self.store.set(STORE_KEY, m);
        println!("[SettingsAppearanceScreen] on_unload");
    }
}

impl ScreenWithHeader for SettingsAppearanceScreen {
    fn title(&self) -> &str {
        "Appearance"
    }
}
