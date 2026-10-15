// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use crate::{Kiosk, MainWindow, Pages, PrinterState, QueueMotion, SettingsMotion, SubPage, kiosk_motion};
use slint::ComponentHandle;

pub fn start(window: &MainWindow) -> slint::Timer {
    let timer = slint::Timer::default();
    let weak = window.as_weak();
    let previous_run = core::cell::Cell::new(mcu_board_support::profile_run());
    let previous_tick = core::cell::Cell::new(u64::MAX);
    let last_touch_presses = core::cell::Cell::new(mcu_board_support::profile_touch_presses());
    let replay_position = core::cell::Cell::new(None);
    let first_tick = core::cell::Cell::new(None);
    timer.start(slint::TimerMode::Repeated, core::time::Duration::from_millis(1), move || {
        if let Some(offset) = mcu_board_support::profile_capture_offset() {
            if let Some(window) = weak.upgrade() {
                if matches!(mcu_board_support::profile_mode(), 5 | 6 | 8) {
                    window.global::<SettingsMotion>().set_raw_y(-(offset as f32));
                } else {
                    window.global::<QueueMotion>().set_raw_y(-(offset as f32));
                }
                mcu_board_support::profile_capture_applied();
            }
            return;
        }
        let run = mcu_board_support::profile_run();
        let mode = run & 255;
        if run != previous_run.replace(run) {
            mcu_board_support::profile_begin_run(run);
            let presses = mcu_board_support::profile_touch_presses();
            let new_press = last_touch_presses.replace(presses) != presses;
            let real_takeover = mode == 2 && new_press;
            if let Some(window) = weak.upgrade() {
                window.global::<Kiosk>().set_enabled(mode == 11);
            }
            if let (Some(position), Some(window)) = (replay_position.take(), weak.upgrade())
                && !real_takeover
            {
                window.window().dispatch_event(slint::platform::WindowEvent::PointerReleased {
                    position,
                    button: slint::platform::PointerEventButton::Left,
                });
                window.window().dispatch_event(slint::platform::WindowEvent::PointerExited);
            }
            previous_tick.set(u64::MAX);
            first_tick.set(None);
            if mode == 11 {
                return;
            }
            if mode != 2 {
                if let Some(window) = weak.upgrade() {
                    window.global::<PrinterState>().set_active_page(
                        if matches!(mode, 4 | 9 | 10 | 12 | 13) {
                            Pages::Ink
                        } else if matches!(mode, 5 | 6 | 8) {
                            Pages::Settings
                        } else {
                            Pages::Home
                        },
                    );
                    window.global::<PrinterState>().set_active_subpage(SubPage::None);
                }
                if let Some(window) = weak.upgrade() {
                    let offset = if mode == 1 || mode == 3 || mode == 7 {
                        window.global::<QueueMotion>().get_maximum() / 2.
                    } else {
                        0.
                    };
                    if matches!(mode, 5 | 6 | 8) {
                        window.global::<SettingsMotion>().set_raw_y(if mode == 6 {
                            -40.
                        } else {
                            0.
                        });
                    } else {
                        window.global::<QueueMotion>().set_raw_y(-(offset as u32 as f32));
                    }
                }
            }
        }
        if matches!(mode, 2 | 4 | 9 | 11 | 12 | 13) {
            return;
        }
        let Some(window) = weak.upgrade() else {
            return;
        };
        let expected_page = if mode == 10 {
            Pages::Ink
        } else if matches!(mode, 5 | 6 | 8) {
            Pages::Settings
        } else {
            Pages::Home
        };
        if window.global::<PrinterState>().get_active_page() != expected_page
            || window.global::<PrinterState>().get_active_subpage() != SubPage::None
        {
            return;
        }

        let tick = {
            let now = mcu_board_support::panel_tick();
            let start = first_tick.get().unwrap_or_else(|| {
                first_tick.set(Some(now));
                now
            });
            now.wrapping_sub(start) as u64
        };
        if previous_tick.replace(tick) == tick {
            return;
        }
        if mode == 10 {
            let stage = tick / 60;
            let position = slint::LogicalPosition::new(90. + (stage / 2 % 4) as f32 * 62., 160.);
            let started = mcu_board_support::profile_now_us();
            if stage % 2 == 0 && replay_position.get().is_none() {
                replay_position.set(Some(position));
                window.window().dispatch_event(slint::platform::WindowEvent::PointerPressed {
                    position,
                    button: slint::platform::PointerEventButton::Left,
                });
            } else if stage % 2 != 0
                && let Some(position) = replay_position.take()
            {
                window.window().dispatch_event(slint::platform::WindowEvent::PointerReleased {
                    position,
                    button: slint::platform::PointerEventButton::Left,
                });
                window.window().dispatch_event(slint::platform::WindowEvent::PointerExited);
            } else {
                return;
            }
            mcu_board_support::profile_dispatch_time(mcu_board_support::profile_now_us() - started);
            return;
        }
        if matches!(mode, 1 | 3 | 6 | 7 | 8) {
            let travel = (tick * if matches!(mode, 3 | 6 | 7 | 8) { 4 } else { 1 }) % 120;
            let offset = if travel > 60 { 120 - travel } else { travel };
            let position = slint::LogicalPosition::new(
                if matches!(mode, 6 | 8) { 300. } else { 210. },
                200. - offset as f32,
            );
            let pressed = replay_position.replace(Some(position)).is_none();
            if matches!(mode, 7 | 8) {
                mcu_board_support::profile_schedule_pointer(position.x, position.y, pressed);
                mcu_board_support::profile_phase(run, tick as u32);
                return;
            }
            let event = if !pressed {
                slint::platform::WindowEvent::PointerMoved { position }
            } else {
                slint::platform::WindowEvent::PointerPressed {
                    position,
                    button: slint::platform::PointerEventButton::Left,
                }
            };
            let dispatched = mcu_board_support::profile_now_us();
            window.window().dispatch_event(event);
            mcu_board_support::profile_dispatch_time(
                mcu_board_support::profile_now_us() - dispatched,
            );
            mcu_board_support::profile_phase(run, tick as u32);
            if mode == 6 {
                window.window().request_redraw();
            }
            return;
        }
        let max = if mode == 5 {
            window.global::<SettingsMotion>().get_maximum()
        } else {
            window.global::<QueueMotion>().get_maximum()
        };
        let offset = kiosk_motion::scroll_offset(tick, max);
        if mode == 5 {
            window.global::<SettingsMotion>().set_raw_y(-offset);
        } else {
            window.global::<QueueMotion>().set_raw_y(-offset);
        }
        mcu_board_support::profile_phase(run, tick as u32);
    });
    timer
}
