// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use slint::ComponentHandle;
#[cfg(not(target_arch = "wasm32"))]
use std::{collections::VecDeque, time::Instant};

slint::include_modules!();

/// Run the Home Assistant demo with optional initial room, lock, and tour settings.
pub fn run(mut args: impl Iterator<Item = String>) -> Result<(), Box<dyn std::error::Error>> {
    let ui = HomeAssistant::new()?;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--no-tour" => ui.set_tour_enabled(false),
            "--unlocked" => ui.set_locked(false),
            "--room" => {
                let name = args.next().ok_or("--room requires a room name")?;
                let index = ["living", "kitchen", "bedroom", "office", "garden"]
                    .iter()
                    .position(|&room| room == name)
                    .ok_or("room must be living, kitchen, bedroom, office, or garden")?;
                ui.set_current_room(index as i32);
            }
            "--help" | "-h" => {
                println!(
                    "Home Assistant\n\n  --no-tour   Disable the idle demonstration\n  --unlocked  Start on the dashboard\n  --room      living | kitchen | bedroom | office | garden"
                );
                return Ok(());
            }
            _ => return Err(format!("unknown option: {arg}").into()),
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    install_render_diagnostics(&ui);
    ui.run()?;
    Ok(())
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn wasm_main() -> Result<(), wasm_bindgen::JsValue> {
    #[cfg(debug_assertions)]
    console_error_panic_hook::set_once();
    run(std::iter::empty()).map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))
}

// These are measured CPU submission times, not GPU completion times, refresh
// intervals, or the firmware's damage-tile measurements. Sampling is opt-in.
#[cfg(not(target_arch = "wasm32"))]
fn install_render_diagnostics(ui: &HomeAssistant) {
    let weak = ui.as_weak();
    let mut start = Instant::now();
    let mut published = start;
    let mut peak = 0.0_f32;
    let mut history = VecDeque::from(vec![0.0_f32; 63]);
    let _ = ui.window().set_rendering_notifier(move |state, _| {
        let Some(ui) = weak.upgrade() else { return };
        if !ui.global::<Home>().get_diagnostics() {
            published = Instant::now();
            peak = 0.0;
            return;
        }
        match state {
            slint::RenderingState::BeforeRendering => start = Instant::now(),
            slint::RenderingState::AfterRendering => {
                peak = peak.max(start.elapsed().as_secs_f32() * 1000.0);
                if published.elapsed().as_millis() >= 250 {
                    history.pop_front();
                    history.push_back(peak);
                    let values: Vec<f32> = history.iter().copied().collect();
                    let measured_peak = peak;
                    // Schedule property writes after the renderer has returned.
                    let _ = weak.upgrade_in_event_loop(move |ui| {
                        let stats = ui.global::<RenderStats>();
                        stats.set_available(true);
                        stats.set_peak_ms(measured_peak);
                        stats.set_history(slint::ModelRc::new(slint::VecModel::from(values)));
                    });
                    peak = 0.0;
                    published = Instant::now();
                }
            }
            _ => {}
        }
    });
    // Some surfaces (including native Metal) accept the notifier but do not
    // emit graphics-API callbacks. Only an actual sample establishes availability.
}
