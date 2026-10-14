// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use crate::{
    MainWindow, Pages, PrinterQueueData, PrinterState, QueueMotion, SettingsMotion, SubPage,
};
use core::cell::Cell;
use mcu_board_support::{TouchPhase, dispatch_touch_event};
use slint::{ComponentHandle, Model};
#[path = "pointer_replay.rs"]
mod pointer_replay;

pub fn start(window: &MainWindow, queue: &PrinterQueueData) -> slint::Timer {
    let timer = slint::Timer::default();
    let Some(scene) = option_env!("BOX_BENCH_SCENE") else { return timer };
    assert!(matches!(scene, "queue" | "settings" | "ink"), "unknown BOX benchmark scene");
    let workload = option_env!("BOX_BENCH_WORKLOAD").unwrap_or("held");
    assert!(matches!(workload, "held" | "flick" | "replay"), "unknown BOX benchmark workload");
    assert!(workload != "replay" || scene == "queue", "replay is a Queue fixture");
    let windows = option_env!("BOX_BENCH_WINDOWS").unwrap_or("3").parse::<u64>().unwrap();
    assert!((1..=3).contains(&windows), "benchmark trace holds at most three windows");
    window.global::<PrinterState>().invoke_show_page(Pages::Ink);
    window.global::<PrinterState>().set_active_subpage(SubPage::None);
    window.global::<SettingsMotion>().on_moved(mcu_board_support::profile_scroll_y);
    let touch_x = option_env!("BOX_BENCH_TOUCH_X").map(|value| value.parse::<f32>().unwrap());
    assert!(touch_x.is_none_or(|x| (0.0..320.0).contains(&x)), "invalid touch x");
    let data = queue.data.clone();
    let initial: alloc::vec::Vec<_> = data.iter().collect();
    let weak = window.as_weak();
    let start = mcu_board_support::profile_now_us();
    let previous = Cell::new(u64::MAX);
    let last_case = Cell::new(u64::MAX);
    let pressed = Cell::new(false);
    let done = Cell::new(false);
    let replay_cursor = Cell::new(0usize);
    let replay_cycle = Cell::new(u64::MAX);
    let origin = Cell::new(0u64);
    timer.start(slint::TimerMode::Repeated, core::time::Duration::from_millis(1), move || {
        let Some(window) = weak.upgrade() else { return };
        let elapsed = mcu_board_support::profile_now_us() - start;
        if elapsed < 3_000_000 || done.get() {
            return;
        }
        let tick = mcu_board_support::profile_animation_tick();
        if previous.replace(tick) == tick {
            return;
        }
        let case = (elapsed - 3_000_000) / 11_000_000;
        let (x, y) = if scene == "queue" { (240., 190.) } else { (180., 150.) };
        let x = touch_x.unwrap_or(x);
        let touch = |phase, y| {
            dispatch_touch_event(window.window(), slint::LogicalPosition::new(x, y), phase)
        };
        if case >= windows {
            mcu_board_support::profile_mode(0);
            if pressed.replace(false) {
                touch(TouchPhase::Ended, y);
            }
            done.set(true);
            mcu_board_support::profile_capture();
            return;
        }
        if last_case.replace(case) != case {
            window.global::<PrinterState>().invoke_show_page(match scene {
                "queue" => Pages::Home,
                "settings" => Pages::Settings,
                _ => Pages::Ink,
            });
            data.set_vec(initial.clone());
            if workload == "replay" {
                if pressed.replace(false) {
                    touch(TouchPhase::Ended, 120.);
                }
                window.global::<QueueMotion>().set_raw_y(-24.045448);
            }
            origin.set(tick);
            replay_cursor.set(0);
            replay_cycle.set(u64::MAX);
        }
        let measuring = (elapsed - 3_000_000) % 11_000_000 >= 1_000_000;
        mcu_board_support::profile_mode(if measuring { case as u32 + 1 } else { 0 });
        if scene == "ink" {
            return;
        }
        if workload == "replay" {
            let age = tick - origin.get();
            if replay_cycle.replace(age / 360) != age / 360 {
                replay_cursor.set(0);
            }
            let mut last_move = None;
            while let Some(&(at, x, y, kind)) = pointer_replay::EVENTS.get(replay_cursor.get()) {
                if at as u64 > age % 360 {
                    break;
                }
                replay_cursor.set(replay_cursor.get() + 1);
                if kind == 2 {
                    last_move = Some((x, y));
                } else {
                    if let Some((x, y)) = last_move.take() {
                        dispatch_touch_event(
                            window.window(),
                            slint::LogicalPosition::new(x as f32, y as f32),
                            TouchPhase::Moved,
                        );
                    }
                    let phase = if kind == 1 { TouchPhase::Started } else { TouchPhase::Ended };
                    pressed.set(kind == 1);
                    dispatch_touch_event(
                        window.window(),
                        slint::LogicalPosition::new(x as f32, y as f32),
                        phase,
                    );
                }
            }
            if let Some((x, y)) = last_move {
                dispatch_touch_event(
                    window.window(),
                    slint::LogicalPosition::new(x as f32, y as f32),
                    TouchPhase::Moved,
                );
            }
        } else if workload == "flick" {
            let phase = (tick - origin.get()) % 120;
            let direction = if (tick - origin.get()) / 120 % 2 == 0 { -1. } else { 1. };
            if phase == 0 {
                touch(TouchPhase::Started, y);
                pressed.set(true);
            }
            if phase < 24 {
                touch(TouchPhase::Moved, y + direction * phase as f32 * 2.);
            } else if pressed.replace(false) {
                touch(TouchPhase::Ended, y + direction * 48.);
            }
        } else {
            if !pressed.replace(true) {
                if scene == "queue" {
                    window.global::<QueueMotion>().set_raw_y(-60.);
                } else {
                    window.global::<SettingsMotion>().set_raw_y(-40.);
                }
                touch(TouchPhase::Started, y);
            }
            let period = if scene == "queue" { 120 } else { 96 };
            let phase = tick % period;
            let phase = if phase < period / 2 { phase } else { period - phase };
            touch(TouchPhase::Moved, y - phase as f32 * if scene == "queue" { 2. } else { 1. });
        }
        mcu_board_support::profile_scroll_y(if scene == "queue" {
            window.global::<QueueMotion>().get_raw_y()
        } else {
            window.global::<SettingsMotion>().get_raw_y()
        });
    });
    timer
}
