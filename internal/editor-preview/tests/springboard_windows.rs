// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![cfg(all(feature = "springboard", feature = "testing", not(target_arch = "wasm32")))]

use std::time::{Duration, Instant};

use i_slint_editor_preview::springboard::{LocalPreviewConfig, Springboard};
use i_slint_editor_preview::springboard_ui::{
    ManualEndpointState, Springboard as SpringboardGlobal, SpringboardContext,
};
use slint::{ComponentHandle, Global, Model};

fn global(window: &SpringboardContext) -> SpringboardGlobal<'_> {
    window.global::<SpringboardGlobal>()
}

fn run_until(done: impl Fn() -> bool + 'static) {
    let deadline = Instant::now() + Duration::from_secs(20);
    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::Repeated, Duration::from_millis(10), move || {
        assert!(Instant::now() < deadline, "Timed out waiting for the springboard UI");
        if done() {
            slint::quit_event_loop().unwrap();
        }
    });
    slint::run_event_loop().unwrap();
}

fn run_until_every_window_shows(
    first: &SpringboardContext,
    second: &SpringboardContext,
    state: ManualEndpointState,
) {
    let windows = [first.as_weak(), second.as_weak()];
    run_until(move || {
        windows.iter().all(|window| {
            window
                .upgrade()
                .is_some_and(|window| global(&window).get_manual_endpoint().state == state)
        })
    });
}

#[test]
fn every_attached_window_shows_the_state_and_forwards_callbacks() {
    i_slint_backend_testing::init_integration_test_with_system_time();
    let first = SpringboardContext::new().unwrap();
    let second = SpringboardContext::new().unwrap();
    let weak_globals = [&first, &second].map(|window| global(window).as_weak());

    let (stop_session, session_stopped) = tokio::sync::oneshot::channel::<()>();
    let session = std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        tokio::task::LocalSet::new().block_on(&runtime, async move {
            let (to_editor, _from_springboard) = tokio::sync::mpsc::unbounded_channel();
            let springboard = Springboard::without_discovery(
                LocalPreviewConfig {
                    executable: "slint-preview-not-started".into(),
                    arguments: Vec::new(),
                    prepare_arguments: None,
                    on_started: None,
                },
                to_editor,
            );
            for weak_global in weak_globals {
                springboard.attach_ui(weak_global);
            }
            session_stopped.await.unwrap();
        });
    });

    let (first_weak, second_weak) = (first.as_weak(), second.as_weak());
    run_until(move || {
        [&first_weak, &second_weak].iter().all(|window| {
            window
                .upgrade()
                .is_some_and(|window| global(&window).get_saved_endpoints().row_count() == 1)
        })
    });

    global(&second).invoke_probe_address("127.0.0.1".into(), 0);
    run_until_every_window_shows(&first, &second, ManualEndpointState::Unreachable);

    global(&first).invoke_probe_address("".into(), 0);
    run_until_every_window_shows(&first, &second, ManualEndpointState::Idle);

    stop_session.send(()).unwrap();
    session.join().unwrap();
}
