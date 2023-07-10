// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

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

/// This struct is filled/provided by the ItemRenderer to return collected metrics
/// during the rendering of the scene.
#[derive(Default, Clone)]
pub struct RenderingMetrics {
    /// The number of layers that were created. None if the renderer does not create layers.
    pub layers_created: Option<usize>,
}

impl core::fmt::Display for RenderingMetrics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(layer_count) = self.layers_created {
            write!(f, "[{} layers created]", layer_count)
        } else {
            Ok(())
        }
    }
}

struct FrameData {
    timestamp: instant::Instant,
    metrics: RenderingMetrics,
}

/// Helper class that rendering backends can use to provide an FPS counter
pub struct RenderingMetricsCollector {
    collected_frame_data_since_second_ago: RefCell<Vec<FrameData>>,
    update_timer: Timer,
    refresh_mode: RefreshMode,
    output_console: bool,
    output_overlay: bool,
}

impl RenderingMetricsCollector {
    /// Returns a new instance of the counter if requested by the user (via `SLINT_DEBUG_PERFORMANCE` environment variable).
    /// The environment variable holds a comma separated list of options:
    ///     * `refresh_lazy`: selects the lazy refresh mode, where measurements are only taken when a frame is rendered (due to user input or animations)
    ///     * `refresh_full_speed`: frames are continuously rendered
    ///     * `console`: the measurement is printed to the console
    ///     * `overlay`: the measurement is drawn as overlay on top of the scene
    ///
    /// If enabled, this will also print out some system information such as whether
    /// this is a debug or release build, as well as the provided winsys_info string.
    pub fn new(winsys_info: &str) -> Option<Rc<Self>> {
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

        let collector = Rc::new(Self {
            collected_frame_data_since_second_ago: Default::default(),
            update_timer: Default::default(),
            refresh_mode,
            output_console,
            output_overlay,
        });

        #[cfg(debug_assertions)]
        let build_config = "debug";
        #[cfg(not(debug_assertions))]
        let build_config = "release";

        eprintln!("Slint: Build config: {}; Backend: {}", build_config, winsys_info);

        let self_weak = Rc::downgrade(&collector);
        collector.update_timer.stop();
        collector.update_timer.start(
            TimerMode::Repeated,
            std::time::Duration::from_secs(1),
            move || {
                let this = match self_weak.upgrade() {
                    Some(this) => this,
                    None => return,
                };
                this.trim_frame_data_to_second_boundary();

                let mut last_frame_details = String::new();
                if let Some(last_frame_data) =
                    this.collected_frame_data_since_second_ago.borrow().last()
                {
                    use core::fmt::Write;
                    if write!(&mut last_frame_details, "{}", last_frame_data.metrics).is_ok()
                        && !last_frame_details.is_empty()
                    {
                        last_frame_details.insert_str(0, "details from last frame: ");
                    }
                }

                if this.output_console {
                    eprintln!(
                        "average frames per second: {} {}",
                        this.collected_frame_data_since_second_ago.borrow().len(),
                        last_frame_details
                    );
                }
            },
        );

        Some(collector)
    }

    fn trim_frame_data_to_second_boundary(self: &Rc<Self>) {
        let mut i = 0;
        let mut frame_times = self.collected_frame_data_since_second_ago.borrow_mut();
        while i < frame_times.len() {
            if frame_times[i].timestamp.elapsed() > std::time::Duration::from_secs(1) {
                frame_times.remove(i);
            } else {
                i += 1
            }
        }
    }

    /// Call this function every time you've completed the rendering of a frame. The `renderer` parameter
    /// is used to collect additional data and is used to render an overlay if enabled.
    pub fn measure_frame_rendered(
        self: &Rc<Self>,
        renderer: &mut dyn crate::item_rendering::ItemRenderer,
    ) {
        self.collected_frame_data_since_second_ago
            .borrow_mut()
            .push(FrameData { timestamp: instant::Instant::now(), metrics: renderer.metrics() });
        if matches!(self.refresh_mode, RefreshMode::FullSpeed) {
            crate::animations::CURRENT_ANIMATION_DRIVER
                .with(|driver| driver.set_has_active_animations());
        }
        self.trim_frame_data_to_second_boundary();

        if self.output_overlay {
            renderer.draw_string(
                &format!("FPS: {}", self.collected_frame_data_since_second_ago.borrow().len()),
                crate::Color::from_rgb_u8(0, 128, 128),
            );
        }
    }
}
