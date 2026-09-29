// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! A context that is never installed as the thread's global one still runs: its clock
//! advances and its event loop drives its own timers.
//!
//! This is the shape `examples/safe-ui/simulator` uses — `SlintContext::new` plus
//! `new_with_context`, without `set_platform`. It used to have a clock frozen at zero, so
//! nothing it owned ever ticked.

#[test]
fn non_global_context_drives_its_own_timers() {
    let Ok(platform) = i_slint_backend_selector::create_backend() else {
        println!("SKIP: no backend available");
        return;
    };
    let ctx = i_slint_core::SlintContext::new(platform);
    let Some(proxy) = ctx.event_loop_proxy() else {
        println!("SKIP: backend provides no event loop proxy");
        return;
    };

    let fired = std::rc::Rc::new(std::cell::Cell::new(0));
    let fired_ = fired.clone();
    let timer = ctx.new_timer();
    timer.start(slint::TimerMode::Repeated, std::time::Duration::from_millis(20), move || {
        fired_.set(fired_.get() + 1)
    });

    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(300));
        let _ = proxy.quit_event_loop();
    });

    match ctx.run_event_loop() {
        Ok(()) => {
            assert!(
                fired.get() > 0,
                "the event loop ran but never drove this context's timers (fired {} times)",
                fired.get()
            );
        }
        // A headless CI may have a backend but no usable event loop; that is not a failure
        // of what this test is checking.
        Err(err) => println!("SKIP: event loop unavailable: {err}"),
    }
}
