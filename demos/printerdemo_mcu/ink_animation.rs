// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use crate::{
    InkParticles, MainWindow, Pages, PrinterState, SubPage,
    ink_particles::{FrameState, Pixel, Vial},
};
use alloc::rc::Rc;
use core::cell::RefCell;
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
    fn release_inactive_images(&mut self) {
        if self.entered.take().is_some() {
            for index in 0..4 {
                self.frames.set_row_data(index, slint::Image::default());
            }
            self.tiles = core::array::from_fn(|_| None);
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
    let leaving = animation.clone();
    let leaving_window = window.as_weak();
    window.global::<PrinterState>().on_before_page_change(move |page| {
        if page != Pages::Ink {
            if let Some(window) = leaving_window.upgrade() {
                window.global::<InkParticles>().set_held([false; 4].into());
            }
            // Free fast memory before the next page constructs its items.
            leaving.borrow_mut().release_inactive_images();
        }
    });
    let weak = window.as_weak();
    let render = move |tick: u64| {
        let Some(window) = weak.upgrade() else { return };
        let active = window.global::<PrinterState>().get_active_page() == Pages::Ink
            && window.global::<PrinterState>().get_active_subpage() == SubPage::None;
        #[cfg(all(feature = "box-ink-tilt", target_arch = "xtensa"))]
        mcu_board_support::set_ink_active(active);
        let mut animation = animation.borrow_mut();
        if !active {
            if animation.entered.is_some() {
                window.global::<InkParticles>().set_held([false; 4].into());
            }
            animation.release_inactive_images();
            return;
        }
        let size = animation.size;
        if size.0 == 0 {
            return;
        }
        let entered = *animation.entered.get_or_insert(tick);
        let age = tick.saturating_sub(entered).min(45) as u32;
        let t = age * 1024 / 45;
        let reveal = t * t * (3072 - 2 * t) / (1024 * 1024);
        #[cfg(feature = "box-bench")]
        let mut times = [0u32; 4];
        for index in 0..4 {
            #[cfg(feature = "box-bench")]
            let started = mcu_board_support::profile_now_us();
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
            #[cfg(all(feature = "box-ink-tilt", target_arch = "xtensa"))]
            tile.vial.advance_tilt(mcu_board_support::ink_up(), tick);
            tile.vial.set_held(
                window.global::<InkParticles>().get_held().row_data(index).unwrap_or(false),
                tick,
            );
            #[cfg(feature = "box-bench")]
            let prepared = mcu_board_support::profile_now_us();
            let pixels = tile.buffers[tile.back].make_mut_slice();
            tile.vial.restore_cached(pixels, &mut tile.states[tile.back]);
            #[cfg(feature = "box-bench")]
            let restored = mcu_board_support::profile_now_us();
            tile.vial.stamp_cached(tick, pixels, &mut tile.states[tile.back]);
            #[cfg(feature = "box-bench")]
            let stamped = mcu_board_support::profile_now_us();
            let frame = slint::Image::from_rgb565(tile.buffers[tile.back].clone());
            tile.back ^= 1;
            animation.frames.set_row_data(index, frame);
            #[cfg(feature = "box-bench")]
            {
                let end = mcu_board_support::profile_now_us();
                for (sum, value) in times.iter_mut().zip([
                    prepared - started,
                    restored - prepared,
                    stamped - restored,
                    end - stamped,
                ]) {
                    *sum += value as u32;
                }
            }
        }
        #[cfg(feature = "box-bench")]
        mcu_board_support::profile_ink_frame(times);
    };
    let timer = slint::Timer::default();
    #[cfg(all(feature = "box-ink-particles", target_arch = "xtensa"))]
    mcu_board_support::add_animation_frame_handler(render);
    #[cfg(not(all(feature = "box-ink-particles", target_arch = "xtensa")))]
    {
        let tick = core::cell::Cell::new(0u64);
        timer.start(slint::TimerMode::Repeated, core::time::Duration::from_millis(16), move || {
            tick.set(tick.get().wrapping_add(1));
            render(tick.get());
        });
    }
    timer
}
