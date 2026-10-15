// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use crate::{
    InkParticles, Kiosk, MainWindow, Pages, PrinterState, QueueMotion, SettingsMotion, SubPage,
};
use alloc::rc::Rc;
use core::cell::Cell;
use slint::ComponentHandle;

pub struct Controller {
    scene: slint::Timer,
    motion: slint::Timer,
}

fn show(window: &MainWindow, stage: u8) {
    if stage != 2 {
        window.global::<InkParticles>().invoke_release();
    }
    window.global::<PrinterState>().set_active_subpage(SubPage::None);
    window.global::<PrinterState>().set_active_page(match stage {
        0 => Pages::Home,
        1 => Pages::Settings,
        _ => Pages::Ink,
    });
    #[cfg(feature = "pico2-queue-profile")]
    mcu_board_support::profile_kiosk_state(true, stage);
}

pub fn start(window: &MainWindow) -> Rc<Controller> {
    let controller =
        Rc::new(Controller { scene: slint::Timer::default(), motion: slint::Timer::default() });
    let timers = controller.clone();
    let weak = window.as_weak();
    window.global::<Kiosk>().on_toggled(move |enabled| {
        timers.scene.stop();
        timers.motion.stop();
        #[cfg(feature = "pico2-queue-profile")]
        mcu_board_support::profile_kiosk_state(enabled, 0);
        if !enabled {
            return;
        }
        let Some(window) = weak.upgrade() else { return };
        let stage = Rc::new(Cell::new(0u8));
        show(&window, 0);
        let scene_stage = stage.clone();
        let scene_window = weak.clone();
        timers.scene.start(
            slint::TimerMode::Repeated,
            core::time::Duration::from_secs(crate::kiosk_motion::SCENE_SECONDS),
            move || {
                let Some(window) = scene_window.upgrade() else { return };
                if !window.global::<Kiosk>().get_enabled() {
                    return;
                }
                let next = (scene_stage.get() + 1) % 3;
                scene_stage.set(next);
                show(&window, next);
            },
        );
        let motion_window = weak.clone();
        let previous_tick = Cell::new(u64::MAX);
        let previous_stage = Cell::new(u8::MAX);
        let origin = Cell::new(0u64);
        #[cfg(not(feature = "pico2"))]
        let software_tick = Cell::new(0u64);
        timers.motion.start(
            slint::TimerMode::Repeated,
            core::time::Duration::from_millis(if cfg!(feature = "pico2") { 1 } else { 16 }),
            move || {
                let Some(window) = motion_window.upgrade() else { return };
                if !window.global::<Kiosk>().get_enabled() {
                    return;
                }
                #[cfg(feature = "pico2-queue-profile")]
                if mcu_board_support::profile_capture_offset().is_some() {
                    return;
                }
                #[cfg(feature = "pico2")]
                let tick = u64::from(mcu_board_support::panel_tick());
                #[cfg(not(feature = "pico2"))]
                let tick = {
                    software_tick.set(software_tick.get().wrapping_add(1));
                    software_tick.get()
                };
                if previous_tick.replace(tick) == tick {
                    return;
                }
                let stage = stage.get();
                if previous_stage.replace(stage) != stage {
                    origin.set(tick);
                }
                let elapsed = tick.wrapping_sub(origin.get());
                match stage {
                    0 => {
                        let offset = crate::kiosk_motion::scroll_offset(
                            elapsed,
                            window.global::<QueueMotion>().get_maximum(),
                        );
                        window.global::<QueueMotion>().set_raw_y(-offset);
                    }
                    1 => {
                        let offset = crate::kiosk_motion::scroll_offset(
                            elapsed,
                            window.global::<SettingsMotion>().get_maximum(),
                        );
                        window.global::<SettingsMotion>().set_raw_y(-offset);
                    }
                    _ => {}
                }
                #[cfg(feature = "pico2-queue-profile")]
                {
                    let run = mcu_board_support::profile_run();
                    if run & 255 == 11 && stage < 2 {
                        mcu_board_support::profile_phase(run, tick as u32);
                    }
                }
            },
        );
    });
    controller
}
