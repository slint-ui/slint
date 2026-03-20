// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

//! Rust version of the SDL underlay example.
//!
//! Demonstrates rendering custom game content with SDL_Renderer before
//! Slint draws its UI overlay on top. Uses the C FFI functions so the
//! approach is identical to what a C++ game would do.

slint::include_modules!();

fn main() {
    let app = App::new().unwrap();

    // Set up the pre-render callback via the C FFI.
    // In the callback, we use raw SDL3 calls to draw game content.
    unsafe {
        slint_sdl_set_pre_render_callback(Some(pre_render), std::ptr::null_mut(), None);
    }

    let app_weak = app.as_weak();
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_millis(16),
        move || {
            if let Some(app) = app_weak.upgrade() {
                app.window().request_redraw();
            }
        },
    );

    app.on_quit(|| slint::quit_event_loop().unwrap());
    app.run().unwrap();
}

// Import the C FFI from the SDL backend
unsafe extern "C" {
    fn slint_sdl_set_pre_render_callback(
        callback: Option<unsafe extern "C" fn(*mut std::ffi::c_void, *mut std::ffi::c_void)>,
        user_data: *mut std::ffi::c_void,
        drop_user_data: Option<unsafe extern "C" fn(*mut std::ffi::c_void)>,
    );
}

/// Pre-render callback — draws animated rectangles using SDL_Renderer.
unsafe extern "C" fn pre_render(renderer: *mut std::ffi::c_void, _user_data: *mut std::ffi::c_void) {
    // We use inline FFI declarations here to keep the example self-contained.
    // A real game would use the SDL3 C headers directly.
    unsafe extern "C" {
        fn SDL_SetRenderDrawColor(r: *mut std::ffi::c_void, red: u8, green: u8, blue: u8, alpha: u8) -> bool;
        fn SDL_RenderFillRect(r: *mut std::ffi::c_void, rect: *const [f32; 4]) -> bool;
        fn SDL_SetRenderDrawBlendMode(r: *mut std::ffi::c_void, mode: u32) -> bool;
    }

    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs_f32();

    unsafe {
        // Dark blue background
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
