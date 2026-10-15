// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use crate::ink_particles::{FrameState, Pixel, Vial};
use crate::{InkParticles, MainWindow, Pages, PrinterState, SubPage};
use alloc::rc::Rc;
use core::cell::{Cell, RefCell};
use slint::{ComponentHandle, Model, Rgb565Pixel, SharedPixelBuffer};

impl Pixel for Rgb565Pixel {
    fn word(self) -> u16 {
        self.0
    }
    fn from_word(value: u16) -> Self {
        Self(value)
    }
    fn copy_background(pixels: &mut [Self], background: &[u16]) {
        bytemuck::cast_slice_mut::<Self, u16>(pixels).copy_from_slice(background);
    }
}

struct Tile {
    vial: Vial,
    buffers: [SharedPixelBuffer<Rgb565Pixel>; 2],
    back: usize,
    states: [FrameState; 2],
}
struct Animation {
    size: (usize, usize),
    tiles: [Option<Tile>; 4],
    frames: Rc<slint::VecModel<slint::Image>>,
    entered: Option<u64>,
}

impl Animation {
    fn release(&mut self) {
        if self.entered.take().is_some() {
            self.tiles = core::array::from_fn(|_| None);
            for index in 0..4 {
                self.frames.set_row_data(index, slint::Image::default());
            }
        }
    }
}

pub fn start(window: &MainWindow) -> slint::Timer {
    let frames = Rc::new(slint::VecModel::from(alloc::vec![slint::Image::default(); 4]));
    window.global::<InkParticles>().set_enabled(true);
    window.global::<InkParticles>().set_frames(frames.clone().into());
    let animation = Rc::new(RefCell::new(Animation {
        size: (0, 0),
        tiles: core::array::from_fn(|_| None),
        frames,
        entered: None,
    }));
    let resized = animation.clone();
    window.global::<InkParticles>().on_resize(move |width, height| {
        let size = (width.clamp(8, 512) as usize, height.clamp(24, 512) as usize);
        let mut animation = resized.borrow_mut();
        if animation.size != size {
            animation.size = size;
            animation.tiles = core::array::from_fn(|_| None);
        }
    });
    let held = Rc::new(Cell::new(0u8));
    let released = animation.clone();
    let released_hold = held.clone();
    window.global::<InkParticles>().on_release(move || {
        released_hold.set(0);
        released.borrow_mut().release();
    });
    let pointer_held = held.clone();
    window.global::<InkParticles>().on_hold(move |index, pressed| {
        if (0..4).contains(&index) {
            let bit = 1 << index;
            pointer_held.set(if pressed {
                pointer_held.get() | bit
            } else {
                pointer_held.get() & !bit
            });
        }
    });
    let weak = window.as_weak();
    let previous_tick = Cell::new(u64::MAX);
    #[cfg(not(feature = "pico2"))]
    let software_tick = Cell::new(0u64);
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        core::time::Duration::from_millis(if cfg!(feature = "pico2") { 1 } else { 16 }),
        move || {
            #[cfg(feature = "pico2-queue-profile")]
            let run = mcu_board_support::profile_run();
            #[cfg(feature = "pico2")]
            let tick = mcu_board_support::panel_tick() as u64;
            #[cfg(not(feature = "pico2"))]
            let tick = {
                software_tick.set(software_tick.get().wrapping_add(1));
                software_tick.get()
            };
            if previous_tick.replace(tick) == tick {
                return;
            }
            let Some(window) = weak.upgrade() else { return };
            let mut animation = animation.borrow_mut();
            let active = window.global::<PrinterState>().get_active_page() == Pages::Ink
                && window.global::<PrinterState>().get_active_subpage() == SubPage::None;
            if !active {
                held.set(0);
                #[cfg(feature = "pico2-queue-profile")]
                mcu_board_support::profile_ink_state(0, [0; 4]);
                animation.release();
                return;
            }
            #[cfg(feature = "pico2-queue-profile")]
            if mcu_board_support::profile_capture_offset().is_some() {
                return;
            }
            let size = animation.size;
            if size.0 == 0 {
                return;
            }
            #[cfg(feature = "pico2-queue-profile")]
            let started = mcu_board_support::profile_now_us();
            let entered = *animation.entered.get_or_insert(tick);
            let t = tick.saturating_sub(entered).min(45) as u32 * 1024 / 45;
            let reveal = t * t * (3072 - 2 * t) / (1024 * 1024);
            let hold_mask = held.get();
            #[cfg(feature = "pico2-queue-profile")]
            let hold_mask = if run & 255 == 9 { 15 } else { hold_mask };
            #[cfg(feature = "pico2-queue-profile")]
            let mut counts = [0; 4];
            #[cfg(feature = "pico2-queue-profile")]
            let mut times = [0; 4];
            for index in 0..4 {
                let level = window
                    .global::<PrinterState>()
                    .get_ink_levels()
                    .row_data(index)
                    .map_or(0., |ink| ink.level);
                let tile = animation.tiles[index].get_or_insert_with(|| Tile {
                    vial: Vial::new(size.0, size.1, index),
                    buffers: core::array::from_fn(|_| {
                        SharedPixelBuffer::new(size.0 as u32, size.1 as u32)
                    }),
                    back: 0,
                    states: core::array::from_fn(|_| FrameState::default()),
                });
                tile.vial.set_level(((level.clamp(0., 1.) * reveal as f32) as u16).min(1024));
                tile.vial.set_held(hold_mask & (1 << index) != 0, tick);
                #[cfg(feature = "pico2-queue-profile")]
                {
                    counts[index] = tile.vial.particle_count() as u32;
                }
                #[cfg(feature = "pico2-queue-profile")]
                let buffer_started = mcu_board_support::profile_now_us();
                let pixels = tile.buffers[tile.back].make_mut_slice();
                #[cfg(feature = "pico2-queue-profile")]
                let restore_started = mcu_board_support::profile_now_us();
                tile.vial.restore_cached(pixels, &mut tile.states[tile.back]);
                #[cfg(feature = "pico2-queue-profile")]
                let stamp_started = mcu_board_support::profile_now_us();
                tile.vial.stamp_cached(tick, pixels, &mut tile.states[tile.back]);
                #[cfg(feature = "pico2-queue-profile")]
                let publish_started = mcu_board_support::profile_now_us();
                let frame = slint::Image::from_rgb565(tile.buffers[tile.back].clone());
                tile.back ^= 1;
                animation.frames.set_row_data(index, frame);
                #[cfg(feature = "pico2-queue-profile")]
                {
                    times[0] += restore_started - buffer_started;
                    times[1] += stamp_started - restore_started;
                    times[2] += publish_started - stamp_started;
                    times[3] += mcu_board_support::profile_now_us() - publish_started;
                }
            }
            #[cfg(feature = "pico2-queue-profile")]
            {
                mcu_board_support::profile_animation_time(
                    mcu_board_support::profile_now_us() - started,
                );
                mcu_board_support::profile_ink_times(times);
                mcu_board_support::profile_ink_state(hold_mask as u32, counts);
                if matches!(run & 255, 4 | 9 | 10 | 11) {
                    mcu_board_support::profile_phase(run, tick as u32);
                }
            }
        },
    );
    timer
}
