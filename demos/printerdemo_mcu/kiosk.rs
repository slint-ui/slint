// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use crate::{Kiosk, MainWindow, Pages, PrinterState, QueueMotion, SettingsMotion, SubPage};
use alloc::rc::Rc;
use core::cell::Cell;
use slint::ComponentHandle;

pub struct Controller {
    scene: slint::Timer,
    _motion: slint::Timer,
    pub stage: Rc<Cell<u8>>,
}

fn show(window: &MainWindow, stage: u8) {
    window.global::<PrinterState>().set_active_subpage(SubPage::None);
    window.global::<PrinterState>().invoke_show_page(match stage {
        0 => Pages::Home,
        1 => Pages::Settings,
        _ => Pages::Ink,
    });
}

pub fn scene_seconds() -> u64 {
    if cfg!(feature = "box-bench") && option_env!("BOX_BENCH_SCENE") == Some("kiosk") {
        12
    } else {
        crate::kiosk_motion::SCENE_SECONDS
    }
}

pub fn start(window: &MainWindow) -> Rc<Controller> {
    let stage = Rc::new(Cell::new(0));
    let previous_stage = Cell::new(u8::MAX);
    let origin = Cell::new(0u64);
    let motion_stage = stage.clone();
    let weak = window.as_weak();
    let motion = move |tick: u64| {
        let Some(window) = weak.upgrade() else { return };
        if !window.global::<Kiosk>().get_enabled() {
            previous_stage.set(u8::MAX);
            return;
        }
        let stage = motion_stage.get();
        if previous_stage.replace(stage) != stage {
            origin.set(tick);
        }
        let elapsed = tick.wrapping_sub(origin.get());
        if stage == 0 {
            let state = window.global::<QueueMotion>();
            state.set_raw_y(-crate::kiosk_motion::scroll_offset(elapsed, state.get_maximum()));
        } else if stage == 1 {
            let state = window.global::<SettingsMotion>();
            state.set_raw_y(-crate::kiosk_motion::scroll_offset(elapsed, state.get_maximum()));
        }
    };
    let timer = slint::Timer::default();
    #[cfg(all(feature = "box-ink-particles", target_arch = "xtensa"))]
    mcu_board_support::add_animation_frame_handler(motion);
    #[cfg(not(all(feature = "box-ink-particles", target_arch = "xtensa")))]
    {
        let tick = Cell::new(0u64);
        timer.start(slint::TimerMode::Repeated, core::time::Duration::from_millis(16), move || {
            tick.set(tick.get().wrapping_add(1));
            motion(tick.get());
        });
    }
    let controller = Rc::new(Controller { scene: Default::default(), _motion: timer, stage });
    let timers = controller.clone();
    let weak = window.as_weak();
    window.global::<Kiosk>().on_toggled(move |enabled| {
        timers.scene.stop();
        if !enabled {
            return;
        }
        let Some(window) = weak.upgrade() else { return };
        timers.stage.set(0);
        show(&window, 0);
        let stage = timers.stage.clone();
        let weak = weak.clone();
        timers.scene.start(
            slint::TimerMode::Repeated,
            core::time::Duration::from_secs(scene_seconds()),
            move || {
                let Some(window) = weak.upgrade() else { return };
                let next = (stage.get() + 1) % 3;
                stage.set(next);
                show(&window, next);
            },
        );
    });
    controller
}
