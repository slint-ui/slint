// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#![no_std]
#![cfg_attr(not(feature = "simulator"), no_main)]

extern crate alloc;

use alloc::vec::Vec;

use alloc::rc::Rc;
use core::cell::Cell;

#[cfg(feature = "ink-particles")]
mod ink_animation;
#[cfg(feature = "ink-particles")]
mod ink_particles;
mod scroll_pause;
use scroll_pause::ScrollGate;

struct ScrollPause {
    gate: Cell<ScrollGate>,
    settle_timer: slint::Timer,
}

impl ScrollPause {
    fn observe(self: &Rc<Self>, raw_y: f32) {
        if !ScrollGate::observe_cell(&self.gate, raw_y) {
            return;
        }
        let weak = Rc::downgrade(self);
        self.settle_timer.start(
            slint::TimerMode::SingleShot,
            core::time::Duration::from_millis(200),
            move || {
                if let Some(pause) = weak.upgrade() {
                    let mut gate = pause.gate.get();
                    gate.settle();
                    pause.gate.set(gate);
                }
            },
        );
    }

    fn allow_progress(&self) -> bool {
        self.gate.get().progress_tick()
    }
}

use slint::Model;

#[allow(unused_imports)]
use mcu_board_support::prelude::*;

slint::include_modules!();

struct PrinterQueueData {
    data: Rc<slint::VecModel<PrinterQueueItem>>,
    print_progress_timer: slint::Timer,
    scroll_pause: Rc<ScrollPause>,
}

impl PrinterQueueData {
    fn push_job(&self, title: slint::SharedString) {
        self.data.push(PrinterQueueItem {
            status: "waiting".into(),
            progress: 0,
            title,
            owner: env!("CARGO_PKG_AUTHORS").into(),
            pages: 1,
            size: "100kB".into(),
            submission_date: "".into(),
        })
    }
}

#[mcu_board_support::entry]
fn main() -> ! {
    mcu_board_support::init();
    let main_window = MainWindow::new().unwrap();
    main_window.global::<PrinterState>().set_ink_levels(
        [
            InkLevel { color: slint::Color::from_rgb_u8(0, 255, 255), level: 0.40 },
            InkLevel { color: slint::Color::from_rgb_u8(255, 0, 255), level: 0.65 },
            InkLevel { color: slint::Color::from_rgb_u8(255, 255, 0), level: 0.50 },
            InkLevel { color: slint::Color::from_rgb_u8(0, 0, 0), level: 0.80 },
        ]
        .into(),
    );

    #[cfg(feature = "box-line-damage")]
    {
        let state = main_window.global::<PrinterState>();
        mcu_board_support::set_line_damage_enabled(state.get_active_page() == Pages::Settings);
        state.on_active_page_changed(|page| {
            mcu_board_support::set_line_damage_enabled(page == Pages::Settings);
        });
    }

    #[cfg(feature = "ink-particles")]
    let _ink_animation = ink_animation::start(&main_window);

    let default_queue: Vec<PrinterQueueItem> =
        main_window.global::<PrinterQueue>().get_printer_queue().iter().collect();
    let printer_queue = Rc::new(PrinterQueueData {
        data: Rc::new(slint::VecModel::from(default_queue.clone())),
        print_progress_timer: Default::default(),
        scroll_pause: Rc::new(ScrollPause {
            gate: Cell::new(ScrollGate::default()),
            settle_timer: Default::default(),
        }),
    });
    main_window.global::<PrinterQueue>().set_printer_queue(printer_queue.data.clone().into());
    main_window.global::<QueueMotion>().set_pixel_snap(cfg!(feature = "box-pixel-snap"));
    main_window.global::<SettingsMotion>().set_pixel_snap(cfg!(feature = "box-pixel-snap"));
    printer_queue.scroll_pause.observe(main_window.global::<QueueMotion>().get_raw_y());
    let pause = printer_queue.scroll_pause.clone();
    main_window.global::<QueueMotion>().on_moved(move |raw_y| {
        pause.observe(raw_y);
    });

    main_window.on_quit(move || {
        #[cfg(not(target_arch = "wasm32"))]
        slint::quit_event_loop().unwrap();
    });

    let printer_queue_copy = printer_queue.clone();
    main_window.global::<PrinterQueue>().on_start_job(move |title| {
        printer_queue_copy.push_job(title);
    });

    let printer_queue_copy = printer_queue.clone();
    main_window.global::<PrinterQueue>().on_cancel_job(move |idx| {
        printer_queue_copy.data.remove(idx as usize);
    });

    let printer_queue_weak = Rc::downgrade(&printer_queue);
    let progress_window = main_window.as_weak();
    printer_queue.print_progress_timer.start(
        slint::TimerMode::Repeated,
        core::time::Duration::from_secs(1),
        move || {
            if let Some(printer_queue) = printer_queue_weak.upgrade() {
                if let Some(window) = progress_window.upgrade() {
                    printer_queue.scroll_pause.observe(window.global::<QueueMotion>().get_raw_y());
                }
                if !printer_queue.scroll_pause.allow_progress() {
                    return;
                }
                if printer_queue.data.row_count() > 0 {
                    let mut top_item = printer_queue.data.row_data(0).unwrap();
                    top_item.progress += 1;
                    top_item.status = "printing".into();
                    if top_item.progress > 100 {
                        printer_queue.data.remove(0);
                        if printer_queue.data.row_count() == 0 {
                            return;
                        }
                        top_item = printer_queue.data.row_data(0).unwrap();
                    }
                    printer_queue.data.set_row_data(0, top_item);
                } else {
                    printer_queue.data.set_vec(default_queue.clone());
                }
            }
        },
    );

    main_window.run().unwrap();

    panic!("The MCU demo should not quit")
}
