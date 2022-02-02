// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

/*!
This module contains a simple helper type to measure the average number of frames rendered per second.
*/

use crate::timers::*;
use std::cell::RefCell;
use std::convert::TryFrom;
use std::rc::Rc;

enum RefreshMode {
    Lazy,      // render only when necessary (default)
    FullSpeed, // continuously render to the screen
}

impl<'a> TryFrom<&Vec<&'a str>> for RefreshMode {
    type Error = ();

    fn try_from(options: &Vec<&'a str>) -> Result<Self, Self::Error> {
        if options.contains(&"refresh_lazy") {
            Ok(Self::Lazy)
        } else if options.contains(&"refresh_full_speed") {
            Ok(Self::FullSpeed)
        } else {
            Err(())
        }
    }
}

/// Helper class that rendering backends can use to provide an FPS counter
pub struct FPSCounter {
    frame_times: RefCell<Vec<instant::Instant>>,
    update_timer: Timer,
    refresh_mode: RefreshMode,
    output_console: bool,
    output_overlay: bool,
}

impl FPSCounter {
    /// Returns a new instance of the counter if requested by the user (via `SLINT_DEBUG_PERFORMANCE` environment variable).
    /// The environment variable holds a comma separated list of options:
    ///     * `refresh_lazy`: selects the lazy refresh mode, where measurements are only taken when a frame is rendered (due to user input or animations)
    ///     * `refresh_full_speed`: frames are continuously rendered
    ///     * `console`: the measurement is printed to the console
    ///     * `overlay`: the measurement is drawn as overlay on top of the scene
    pub fn new() -> Option<Rc<Self>> {
        let options = match std::env::var("SLINT_DEBUG_PERFORMANCE") {
            Ok(var) => var,
            _ => return None,
        };
        let options: Vec<&str> = options.split(',').collect();

        let refresh_mode = match RefreshMode::try_from(&options) {
            Ok(mode) => mode,
            Err(_) => {
                eprintln!("Missing refresh mode in SLINT_DEBUG_PERFORMANCE. Please specify either refresh_full_speed or refresh_lazy");
                return None;
            }
        };

        let output_console = options.contains(&"console");
        let output_overlay = options.contains(&"overlay");

        if !output_console && !output_overlay {
            eprintln!("Missing output mode in SLINT_DEBUG_PERFORMANCE. Please specify either console or overlay (or both)");
            return None;
        }

        Some(Rc::new(Self {
            frame_times: Default::default(),
            update_timer: Default::default(),
            refresh_mode,
            output_console,
            output_overlay,
        }))
    }

    /// Call this function if you want to start measurements. This will also print out some system information such as whether
    /// this is a debug or release build, as well as the provided winsys_info string.
    pub fn start(self: &Rc<Self>, winsys_info: &str) {
        #[cfg(debug_assertions)]
        let build_config = "debug";
        #[cfg(not(debug_assertions))]
        let build_config = "release";

        eprintln!("SixtyFPS: Build config: {}; Backend: {}", build_config, winsys_info);

        let this = self.clone();
        self.update_timer.stop();
        self.update_timer.start(TimerMode::Repeated, std::time::Duration::from_secs(1), move || {
            this.trim_frame_times();
            if this.output_console {
                eprintln!("average frames per second: {}", this.frame_times.borrow().len());
            }
        })
    }

    fn trim_frame_times(self: &Rc<Self>) {
        let mut i = 0;
        let mut frame_times = self.frame_times.borrow_mut();
        while i < frame_times.len() {
            if frame_times[i].elapsed() > std::time::Duration::from_secs(1) {
                frame_times.remove(i);
            } else {
                i += 1
            }
        }
    }

    /// Call this function every time you've completed the rendering of a frame.
    pub fn measure_frame_rendered(
        self: &Rc<Self>,
        renderer_for_overlay: &mut dyn crate::item_rendering::ItemRenderer,
    ) {
        self.frame_times.borrow_mut().push(instant::Instant::now());
        if matches!(self.refresh_mode, RefreshMode::FullSpeed) {
            crate::animations::CURRENT_ANIMATION_DRIVER
                .with(|driver| driver.set_has_active_animations());
        }
        self.trim_frame_times();

        if self.output_overlay {
            renderer_for_overlay.draw_string(
                &format!("FPS: {}", self.frame_times.borrow().len()),
                crate::Color::from_rgb_u8(0, 128, 128),
            );
        }
    }
}
