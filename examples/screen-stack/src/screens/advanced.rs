// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use crate::screen::{Screen, ScreenCallbackHelper, ScreenContext, ScreenWithHeader};
use crate::store::AppStore;
use slint_interpreter::{ComponentInstance, Value};
use std::cell::RefCell;
use std::rc::Rc;

const STORE_KEY: &str = "advanced";

#[derive(Debug, Clone, Default)]
pub struct AdvancedModel {
    pub debug_mode: bool,
    pub experimental: bool,
}

pub struct AdvancedScreen {
    model: RefCell<AdvancedModel>,
    store: Rc<AppStore>,
}

impl AdvancedScreen {
    pub fn create(store: Rc<AppStore>) -> Box<dyn Screen> {
        println!("[AdvancedScreen] Controller CREATED");
        let model = store.get_or_default::<AdvancedModel>(STORE_KEY);
        Box::new(Self { model: RefCell::new(model), store })
    }
}

impl Drop for AdvancedScreen {
    fn drop(&mut self) {
        println!("[AdvancedScreen] Controller DESTROYED");
    }
}

impl Screen for AdvancedScreen {
    fn name(&self) -> &'static str {
        "advanced"
    }

    fn on_loaded(&self, instance: &ComponentInstance, ctx: &ScreenContext) {
        let m = self.model.borrow();
        let _ = instance.set_property("debug-mode", Value::Bool(m.debug_mode));
        let _ = instance.set_property("experimental-features", Value::Bool(m.experimental));

        ScreenCallbackHelper::setup_navigate(instance, ctx);

        println!(
            "[AdvancedScreen] on_loaded - debug={}, experimental={}",
            m.debug_mode, m.experimental
        );
    }

    fn on_unload(&self, instance: &ComponentInstance) {
        if let Ok(Value::Bool(v)) = instance.get_property("debug-mode") {
            self.model.borrow_mut().debug_mode = v;
        }
        if let Ok(Value::Bool(v)) = instance.get_property("experimental-features") {
            self.model.borrow_mut().experimental = v;
        }

        let m = self.model.borrow().clone();
        println!(
            "[AdvancedScreen] on_unload - saving: debug={}, experimental={}",
            m.debug_mode, m.experimental
        );
        self.store.set(STORE_KEY, m);
    }
}

impl ScreenWithHeader for AdvancedScreen {
    fn title(&self) -> &str {
        "Advanced Settings"
    }
}
