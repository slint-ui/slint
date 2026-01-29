// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use crate::screen::{Screen, ScreenCallbackHelper, ScreenContext};
use crate::store::AppStore;
use slint::SharedString;
use slint_interpreter::{ComponentInstance, Value};
use std::rc::Rc;

/// Home screen - no persistent state needed.
pub struct HomeScreen {
    user_name: String,
}

impl HomeScreen {
    pub fn create(_store: Rc<AppStore>) -> Box<dyn Screen> {
        println!("[HomeScreen] Controller CREATED");
        Box::new(Self { user_name: "Demo User".to_string() })
    }
}

impl Drop for HomeScreen {
    fn drop(&mut self) {
        println!("[HomeScreen] Controller DESTROYED");
    }
}

impl Screen for HomeScreen {
    fn name(&self) -> &'static str {
        "home"
    }

    fn on_loaded(&self, instance: &ComponentInstance, ctx: &ScreenContext) {
        let _ =
            instance.set_property("user-name", Value::String(SharedString::from(&self.user_name)));

        ScreenCallbackHelper::setup_navigate(instance, ctx);

        println!("[HomeScreen] on_loaded - user: {}", self.user_name);
    }
}

// HomeScreen uses AbstractScreen (no header), so no ScreenWithHeader trait
