// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use crate::screen::{Screen, ScreenCallbackHelper, ScreenContext, ScreenWithHeader};
use crate::store::AppStore;
use slint_interpreter::ComponentInstance;
use std::rc::Rc;

/// Settings category list screen - just navigation, no state.
pub struct SettingsScreen;

impl SettingsScreen {
    pub fn create(_store: Rc<AppStore>) -> Box<dyn Screen> {
        println!("[SettingsScreen] Controller CREATED");
        Box::new(Self)
    }
}

impl Drop for SettingsScreen {
    fn drop(&mut self) {
        println!("[SettingsScreen] Controller DESTROYED");
    }
}

impl Screen for SettingsScreen {
    fn name(&self) -> &'static str {
        "settings"
    }

    fn on_loaded(&self, instance: &ComponentInstance, ctx: &ScreenContext) {
        ScreenCallbackHelper::setup_navigate(instance, ctx);
        println!("[SettingsScreen] on_loaded");
    }
}

impl ScreenWithHeader for SettingsScreen {
    fn title(&self) -> &str {
        "Settings"
    }
}
