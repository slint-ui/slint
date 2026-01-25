// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use crate::screen::{Screen, ScreenCallbackHelper, ScreenContext, ScreenWithHeader};
use crate::store::AppStore;
use slint_interpreter::{ComponentInstance, Value};
use std::rc::Rc;

pub struct MediaScreen {
    #[allow(dead_code)]
    store: Rc<AppStore>,
}

impl MediaScreen {
    pub fn create(store: Rc<AppStore>) -> Box<dyn Screen> {
        println!("[MediaScreen] Controller CREATED");
        Box::new(Self { store })
    }
}

impl Drop for MediaScreen {
    fn drop(&mut self) {
        println!("[MediaScreen] Controller DESTROYED");
    }
}

impl Screen for MediaScreen {
    fn name(&self) -> &'static str {
        "media"
    }

    fn on_loaded(&self, instance: &ComponentInstance, ctx: &ScreenContext) {
        ScreenCallbackHelper::setup_navigate(instance, ctx);

        // Handle tab changes
        let _ = instance.set_callback("tab-changed", move |args| {
            if let Some(Value::Number(index)) = args.first() {
                let tab_name = match *index as i32 {
                    0 => "Photos",
                    1 => "Videos",
                    2 => "Music",
                    _ => "Unknown",
                };
                println!("[MediaScreen] Tab changed to: {} ({})", tab_name, *index as i32);
            }
            Value::Void
        });

        println!("[MediaScreen] on_loaded");
    }
}

impl ScreenWithHeader for MediaScreen {
    fn title(&self) -> &str {
        "Media"
    }
}
