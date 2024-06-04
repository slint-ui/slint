// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/*!
This module contains a simple helper type to measure the average number of frames rendered per second.
*/

use crate::animations::Instant;
use crate::debug_log;
use crate::timers::{Timer, TimerMode};
use alloc::format;
use alloc::rc::Rc;
#[cfg(not(feature = "std"))]
use alloc::string::String;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
use core::cell::RefCell;

/// The method in which we refresh the window
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RefreshMode {
    /// render only when necessary (default)
    Lazy,
    /// continuously render to the screen
    FullSpeed,
}

/// This struct is filled/provided by the ItemRenderer to return collected metrics
/// during the rendering of the scene.
#[derive(Default, Clone)]
pub struct RenderingMetrics {
    /// The number of layers that were created. None if the renderer does not create layers.
    pub layers_created: Option<usize>,
}

impl core::fmt::Display for RenderingMetrics {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if let Some(layer_count) = self.layers_created {
            write!(f, "[{} layers created]", layer_count)
        } else {
            Ok(())
        }
    }
}

struct FrameData {
    timestamp: Instant,
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
        #[cfg(feature = "std")]
        let options = std::env::var("SLINT_DEBUG_PERFORMANCE").ok()?;
        #[cfg(not(feature = "std"))]
        let options = option_env!("SLINT_DEBUG_PERFORMANCE")?;
        let mut output_console = false;
        let mut output_overlay = false;
        let mut refresh_mode = None;
        for option in options.split(',') {
            match option {
                "console" => output_console = true,
                "overlay" => output_overlay = true,
                "refresh_lazy" => refresh_mode = Some(RefreshMode::Lazy),
                "refresh_full_speed" => refresh_mode = Some(RefreshMode::FullSpeed),
                _ => {}
            }
        }

        let Some(refresh_mode) = refresh_mode else {
            debug_log!("Missing refresh mode in SLINT_DEBUG_PERFORMANCE. Please specify either refresh_full_speed or refresh_lazy");
            return None;
        };

        if !output_console && !output_overlay {
            debug_log!("Missing output mode in SLINT_DEBUG_PERFORMANCE. Please specify either console or overlay (or both)");
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

        debug_log!("Slint: Build config: {}; Backend: {}", build_config, winsys_info);

        let self_weak = Rc::downgrade(&collector);
        collector.update_timer.stop();
        collector.update_timer.start(
            TimerMode::Repeated,
            core::time::Duration::from_secs(1),
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
                    debug_log!(
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
        let mut frame_times = self.collected_frame_data_since_second_ago.borrow_mut();
        let now = Instant::now();
        frame_times.retain(|frame| {
            now.duration_since(frame.timestamp) <= core::time::Duration::from_secs(1)
        });
    }

    /// Call this function every time you've completed the rendering of a frame. The `renderer` parameter
    /// is used to collect additional data and is used to render an overlay if enabled.
    pub fn measure_frame_rendered(
        self: &Rc<Self>,
        renderer: &mut dyn crate::item_rendering::ItemRenderer,
    ) {
        self.collected_frame_data_since_second_ago
            .borrow_mut()
            .push(FrameData { timestamp: Instant::now(), metrics: renderer.metrics() });
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

    /// Always render the full screen.
    pub fn refresh_mode(&self) -> RefreshMode {
        self.refresh_mode
    }
}
