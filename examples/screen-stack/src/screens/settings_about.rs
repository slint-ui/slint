// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use crate::screen::{Screen, ScreenCallbackHelper, ScreenContext, ScreenWithHeader};
use crate::store::AppStore;
use slint_interpreter::ComponentInstance;
use std::rc::Rc;

pub struct SettingsAboutScreen;

impl SettingsAboutScreen {
    pub fn create(_store: Rc<AppStore>) -> Box<dyn Screen> {
        println!("[SettingsAboutScreen] Controller CREATED");
        Box::new(Self)
    }
}

impl Drop for SettingsAboutScreen {
    fn drop(&mut self) {
        println!("[SettingsAboutScreen] Controller DESTROYED");
    }
}

impl Screen for SettingsAboutScreen {
    fn name(&self) -> &'static str {
        "settings-about"
    }

    fn on_loaded(&self, instance: &ComponentInstance, ctx: &ScreenContext) {
        ScreenCallbackHelper::setup_navigate(instance, ctx);
        println!("[SettingsAboutScreen] on_loaded");
    }
}

impl ScreenWithHeader for SettingsAboutScreen {
    fn title(&self) -> &str {
        "About"
    }
}
