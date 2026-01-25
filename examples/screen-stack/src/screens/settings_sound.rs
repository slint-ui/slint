// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use crate::screen::{Screen, ScreenCallbackHelper, ScreenContext, ScreenWithHeader};
use crate::store::AppStore;
use slint_interpreter::{ComponentInstance, Value};
use std::rc::Rc;

const STORE_KEY: &str = "sound";

#[derive(Debug, Clone)]
pub struct SoundModel {
    pub volume: f32,
    pub effects: bool,
}

impl Default for SoundModel {
    fn default() -> Self {
        Self { volume: 0.7, effects: true }
    }
}

pub struct SettingsSoundScreen {
    store: Rc<AppStore>,
}

impl SettingsSoundScreen {
    pub fn create(store: Rc<AppStore>) -> Box<dyn Screen> {
        println!("[SettingsSoundScreen] Controller CREATED");
        Box::new(Self { store })
    }
}

impl Drop for SettingsSoundScreen {
    fn drop(&mut self) {
        println!("[SettingsSoundScreen] Controller DESTROYED");
    }
}

impl Screen for SettingsSoundScreen {
    fn name(&self) -> &'static str {
        "settings-sound"
    }

    fn on_loaded(&self, instance: &ComponentInstance, ctx: &ScreenContext) {
        let m = self.store.get_or_default::<SoundModel>(STORE_KEY);
        let _ = instance.set_property("volume", Value::Number(m.volume as f64));
        let _ = instance.set_property("effects", Value::Bool(m.effects));
        ScreenCallbackHelper::setup_navigate(instance, ctx);
        println!("[SettingsSoundScreen] on_loaded - volume: {:.0}%", m.volume * 100.0);
    }

    fn on_unload(&self, instance: &ComponentInstance) {
        let mut m = SoundModel::default();
        if let Ok(Value::Number(v)) = instance.get_property("volume") {
            m.volume = v as f32;
        }
        if let Ok(Value::Bool(v)) = instance.get_property("effects") {
            m.effects = v;
        }
        self.store.set(STORE_KEY, m.clone());
        println!("[SettingsSoundScreen] on_unload - volume: {:.0}%", m.volume * 100.0);
    }
}

impl ScreenWithHeader for SettingsSoundScreen {
    fn title(&self) -> &str {
        "Sound"
    }
}
