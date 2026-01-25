// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use crate::screen::{Screen, ScreenCallbackHelper, ScreenContext, ScreenWithHeader};
use crate::store::AppStore;
use slint::SharedString;
use slint_interpreter::{ComponentInstance, Value};
use std::cell::RefCell;
use std::rc::Rc;

const STORE_KEY: &str = "profile";

#[derive(Debug, Clone, Default)]
pub struct ProfileModel {
    pub user_name: String,
    pub email: String,
}

pub struct ProfileScreen {
    model: RefCell<ProfileModel>,
    store: Rc<AppStore>,
}

impl ProfileScreen {
    pub fn create(store: Rc<AppStore>) -> Box<dyn Screen> {
        println!("[ProfileScreen] Controller CREATED");
        let model = store.get_or_default::<ProfileModel>(STORE_KEY);
        Box::new(Self { model: RefCell::new(model), store })
    }
}

impl Drop for ProfileScreen {
    fn drop(&mut self) {
        println!("[ProfileScreen] Controller DESTROYED");
    }
}

impl Screen for ProfileScreen {
    fn name(&self) -> &'static str {
        "profile"
    }

    fn on_loaded(&self, instance: &ComponentInstance, ctx: &ScreenContext) {
        let m = self.model.borrow();
        let _ = instance.set_property("user-name", Value::String(SharedString::from(&m.user_name)));
        let _ = instance.set_property("email", Value::String(SharedString::from(&m.email)));

        ScreenCallbackHelper::setup_navigate(instance, ctx);

        // Save button callback
        let model = self.model.clone();
        let store = self.store.clone();
        let _ = instance.set_callback("save-profile", move |_| {
            let m = model.borrow().clone();
            println!("[ProfileScreen] Save clicked: {:?}", m);
            store.set(STORE_KEY, m);
            Value::Void
        });

        println!("[ProfileScreen] on_loaded - user: {}", m.user_name);
    }

    fn on_unload(&self, instance: &ComponentInstance) {
        if let Ok(Value::String(v)) = instance.get_property("user-name") {
            self.model.borrow_mut().user_name = v.to_string();
        }
        if let Ok(Value::String(v)) = instance.get_property("email") {
            self.model.borrow_mut().email = v.to_string();
        }

        let m = self.model.borrow().clone();
        println!("[ProfileScreen] on_unload - saving: {:?}", m);
        self.store.set(STORE_KEY, m);
    }
}

impl ScreenWithHeader for ProfileScreen {
    fn title(&self) -> &str {
        "Profile"
    }
}
