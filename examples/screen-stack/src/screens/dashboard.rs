// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use crate::screen::{Screen, ScreenCallbackHelper, ScreenContext, ScreenWithHeader};
use crate::store::AppStore;
use slint_interpreter::{ComponentInstance, Value};
use std::rc::Rc;

pub struct DashboardScreen {
    #[allow(dead_code)]
    store: Rc<AppStore>,
}

impl DashboardScreen {
    pub fn create(store: Rc<AppStore>) -> Box<dyn Screen> {
        println!("[DashboardScreen] Controller CREATED");
        Box::new(Self { store })
    }
}

impl Drop for DashboardScreen {
    fn drop(&mut self) {
        println!("[DashboardScreen] Controller DESTROYED");
    }
}

impl Screen for DashboardScreen {
    fn name(&self) -> &'static str {
        "dashboard"
    }

    fn on_loaded(&self, instance: &ComponentInstance, ctx: &ScreenContext) {
        ScreenCallbackHelper::setup_navigate(instance, ctx);

        // Handle tab changes
        let _ = instance.set_callback("tab-changed", move |args| {
            if let Some(Value::Number(index)) = args.first() {
                println!("[DashboardScreen] Tab changed to: {}", *index as i32);
            }
            Value::Void
        });

        println!("[DashboardScreen] on_loaded");
    }
}

impl ScreenWithHeader for DashboardScreen {
    fn title(&self) -> &str {
        "Dashboard"
    }
}
