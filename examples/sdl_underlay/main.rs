// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

//! Rust version of the SDL underlay example.
//!
//! Demonstrates rendering custom game content with SDL_Renderer before
//! Slint draws its UI overlay on top.

slint::include_modules!();

use std::cell::Cell;
use std::rc::Rc;

struct GameState {
    start: std::time::Instant,
    animation_enabled: Rc<Cell<bool>>,
    speed: Rc<Cell<f32>>,
}

fn main() {
    let app = App::new().unwrap();

    // Shared state read by the pre-render callback
    let animation_enabled = Rc::new(Cell::new(true));
    let speed = Rc::new(Cell::new(1.0f32));
    let start = std::time::Instant::now();

    let state = Box::new(GameState {
        start,
        animation_enabled: animation_enabled.clone(),
        speed: speed.clone(),
    });

    unsafe {
        slint_sdl_set_pre_render_callback(
            Some(pre_render),
            Box::into_raw(state) as *mut std::ffi::c_void,
            Some(drop_state),
        );
    }

    // Timer to poll UI properties and drive redraws
    let app_weak = app.as_weak();
    let anim = animation_enabled.clone();
    let spd = speed.clone();
    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::Repeated, std::time::Duration::from_millis(16), move || {
        if let Some(app) = app_weak.upgrade() {
            anim.set(app.get_animation_enabled());
            spd.set(app.get_speed());
            app.window().request_redraw();
        }
    });

    app.on_quit(|| slint::quit_event_loop().unwrap());
    app.run().unwrap();
}

// ---------------------------------------------------------------------------
// C FFI for the pre-render callback
// ---------------------------------------------------------------------------

unsafe extern "C" {
    fn slint_sdl_set_pre_render_callback(
        callback: Option<unsafe extern "C" fn(*mut std::ffi::c_void, *mut std::ffi::c_void)>,
        user_data: *mut std::ffi::c_void,
        drop_user_data: Option<unsafe extern "C" fn(*mut std::ffi::c_void)>,
    );
}

unsafe extern "C" fn drop_state(ptr: *mut std::ffi::c_void) {
    unsafe { drop(Box::from_raw(ptr as *mut GameState)) };
}

unsafe extern "C" fn pre_render(renderer: *mut std::ffi::c_void, user_data: *mut std::ffi::c_void) {
    unsafe extern "C" {
        fn SDL_SetRenderDrawColor(
            r: *mut std::ffi::c_void,
            red: u8,
            green: u8,
            blue: u8,
            alpha: u8,
        ) -> bool;
        fn SDL_RenderFillRect(r: *mut std::ffi::c_void, rect: *const [f32; 4]) -> bool;
        fn SDL_SetRenderDrawBlendMode(r: *mut std::ffi::c_void, mode: u32) -> bool;
    }

    let state = unsafe { &*(user_data as *const GameState) };

    let elapsed = state.start.elapsed().as_secs_f32();
    let t = if state.animation_enabled.get() { elapsed * state.speed.get() } else { 0.0 };

    unsafe {
        SDL_SetRenderDrawBlendMode(renderer, 1); // SDL_BLENDMODE_BLEND
        SDL_SetRenderDrawColor(renderer, 20, 20, 40, 255);
        SDL_RenderFillRect(renderer, std::ptr::null());

        // Animated colored rectangles
        for i in 0..8 {
            let fi = i as f32;
            let phase = t * 2.0 + fi * 0.8;
            let x = 100.0 + fi * 90.0;
            let y = 300.0 + 100.0 * phase.sin();
            let size = 30.0 + 20.0 * (t * 1.5 + fi).sin();

            let r = (128.0 + 127.0 * (t * 0.7 + fi * 0.5).sin()) as u8;
            let g = (128.0 + 127.0 * (t * 0.5 + fi * 0.4).sin()) as u8;
            let b = (128.0 + 127.0 * (t * 0.3 + fi * 0.3).sin()) as u8;

            SDL_SetRenderDrawColor(renderer, r, g, b, 200);
            let rect = [x - size / 2.0, y - size / 2.0, size, size];
            SDL_RenderFillRect(renderer, &rect);
        }
    }
}
