// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use chrono::{Datelike, Local, Timelike};
use kira::{
    AudioManager, AudioManagerSettings, DefaultBackend,
    sound::static_sound::StaticSoundData,
};
use std::io::Cursor;
use slint::{Timer, TimerMode};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

slint::slint! {
    export { Api, AppWindow } from "../ui/demo.slint";
}

// https://sourceforge.net/projects/sox/
// $ sox -n dial-tick.wav synth 0.01 sine 1000 fade t 0 0.01 0.005 gain -1
const DIAL_TICK : &[u8] = include_bytes!("../ui/sounds/dial-tick.wav");

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub fn main() {
    // This provides better error messages in debug mode.
    // It's disabled in release mode so it doesn't bloat up the file size.
    #[cfg(all(debug_assertions, target_arch = "wasm32"))]
    console_error_panic_hook::set_once();

    let app = AppWindow::new().expect("AppWindow::new() failed");
    let app_weak = app.as_weak();

     let mut manager = AudioManager::<DefaultBackend>::new(AudioManagerSettings::default()).unwrap();

    let api = app.global::<Api>();
    api.on_play_sound(move |sound| {
        let sound = match sound {
            SoundEffect::DialTick => DIAL_TICK,
        };
        let cursor = Cursor::new(sound);
        let sound_data = StaticSoundData::from_cursor(cursor).unwrap();
        let _sound_handle = manager.play(sound_data).unwrap();
    });
    
    let timer = Timer::default();
    timer.start(TimerMode::Repeated, std::time::Duration::from_millis(1000), move || {
        if let Some(app) = app_weak.upgrade() {
            let api = app.global::<Api>();
            let now = Local::now();
            let mut date = Date::default();
            date.year = now.year() as i32;
            date.month = now.month() as i32;
            date.day = now.day() as i32;
            api.set_current_date(date);

            let mut time = Time::default();
            time.hour = now.hour() as i32;
            time.minute = now.minute() as i32;
            time.second = now.second() as i32;
            api.set_current_time(time);
        }
    });

    app.run().expect("AppWindow::run() failed");
}
