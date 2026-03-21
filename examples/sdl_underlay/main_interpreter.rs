// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

//! Interpreter-based SDL underlay example with live reload.
//!
//! Edit `scene.slint` while this is running and see changes instantly!
//!
//! Usage: SLINT_BACKEND=sdl cargo run -p sdl-underlay --bin sdl_underlay_live [path/to/scene.slint]

use slint_interpreter::ComponentHandle;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

fn main() {
    let slint_path =
        std::env::args().nth(1).unwrap_or_else(|| "examples/sdl_underlay/scene.slint".into());

    // Initial load
    let instance = load_component(&slint_path).expect("Failed to load .slint file");
    setup_callbacks(&instance);

    // Set up game rendering callback
    let start = std::time::Instant::now();
    let animation_enabled = Rc::new(Cell::new(true));
    let speed = Rc::new(Cell::new(1.0f32));

    {
        let anim = animation_enabled.clone();
        let spd = speed.clone();
        unsafe {
            let state = Box::new((start, anim, spd));
            slint_sdl_set_pre_render_callback(
                Some(pre_render),
                Box::into_raw(state) as *mut _,
                Some(drop_state),
            );
        }
    }

    // Shared instance for reload
    let instance = Rc::new(RefCell::new(instance));

    // File watcher: poll mtime every 500ms
    let last_mtime = Rc::new(Cell::new(
        std::fs::metadata(&slint_path)
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH),
    ));
    let reload_path = slint_path.clone();
    let reload_inst = instance.clone();
    let reload_mtime = last_mtime.clone();
    let reload_timer = slint::Timer::default();
    reload_timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_millis(500),
        move || {
            let Ok(mtime) = std::fs::metadata(&reload_path).and_then(|m| m.modified()) else {
                return;
            };
            if mtime == reload_mtime.get() {
                return;
            }
            reload_mtime.set(mtime);
            eprintln!("[sdl_underlay] Reloading {}...", reload_path);
            if let Some(new_instance) = load_component(&reload_path) {
                let mut inst = reload_inst.borrow_mut();
                // Show new before hiding old, so the window count never drops
                // to zero (which would quit the event loop).
                setup_callbacks(&new_instance);
                new_instance.show().ok();
                inst.hide().ok();
                *inst = new_instance;
            }
        },
    );

    // Animation timer
    let anim_inst = instance.clone();
    let anim_enabled = animation_enabled.clone();
    let anim_speed = speed.clone();
    let anim_timer = slint::Timer::default();
    anim_timer.start(slint::TimerMode::Repeated, std::time::Duration::from_millis(16), move || {
        let inst = anim_inst.borrow();
        if let Some(v) =
            inst.get_property("animation-enabled").ok().and_then(|v| bool::try_from(v).ok())
        {
            anim_enabled.set(v);
        }
        if let Some(v) = inst.get_property("speed").ok().and_then(|v| f64::try_from(v).ok()) {
            anim_speed.set(v as f32);
        }
        inst.window().request_redraw();
    });

    instance.borrow().show().unwrap();
    slint::run_event_loop().unwrap();
}

fn load_component(path: &str) -> Option<slint_interpreter::ComponentInstance> {
    let compiler = slint_interpreter::Compiler::default();
    let result = spin_on::spin_on(compiler.build_from_path(std::path::PathBuf::from(path)));
    for diag in result.diagnostics() {
        eprintln!("{}", diag);
    }
    if result.has_errors() {
        return None;
    }
    let def = result.components().next()?;
    Some(def.create().unwrap())
}

fn setup_callbacks(instance: &slint_interpreter::ComponentInstance) {
    let _ = instance.set_callback("quit", |_| {
        slint::quit_event_loop().unwrap();
        slint_interpreter::Value::default()
    });
}

// ---------------------------------------------------------------------------
// Pre-render callback
// ---------------------------------------------------------------------------

type State = (std::time::Instant, Rc<Cell<bool>>, Rc<Cell<f32>>);

unsafe extern "C" {
    fn slint_sdl_set_pre_render_callback(
        callback: Option<unsafe extern "C" fn(*mut std::ffi::c_void, *mut std::ffi::c_void)>,
        user_data: *mut std::ffi::c_void,
        drop_user_data: Option<unsafe extern "C" fn(*mut std::ffi::c_void)>,
    );
}

unsafe extern "C" fn drop_state(ptr: *mut std::ffi::c_void) {
    unsafe { drop(Box::from_raw(ptr as *mut State)) };
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

    let (start, anim, spd) = unsafe { &*(user_data as *const State) };
    let elapsed = start.elapsed().as_secs_f32();
    let t = if anim.get() { elapsed * spd.get() } else { 0.0 };

    unsafe {
        SDL_SetRenderDrawBlendMode(renderer, 1);
        SDL_SetRenderDrawColor(renderer, 20, 20, 40, 255);
        SDL_RenderFillRect(renderer, std::ptr::null());

        for i in 0..8 {
            let fi = i as f32;
            let x = 100.0 + fi * 90.0;
            let y = 300.0 + 100.0 * (t * 2.0 + fi * 0.8).sin();
            let size = 30.0 + 20.0 * (t * 1.5 + fi).sin();
            let r = (128.0 + 127.0 * (t * 0.7 + fi * 0.5).sin()) as u8;
            let g = (128.0 + 127.0 * (t * 0.5 + fi * 0.4).sin()) as u8;
            let b = (128.0 + 127.0 * (t * 0.3 + fi * 0.3).sin()) as u8;
            SDL_SetRenderDrawColor(renderer, r, g, b, 200);
            SDL_RenderFillRect(renderer, &[x - size / 2.0, y - size / 2.0, size, size]);
        }
    }
}
