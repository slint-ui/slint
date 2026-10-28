// Copyright © 2026 Koichiro Ohba
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore Koichiro Ohba

#[cfg(all(
    target_os = "macos",
    any(feature = "renderer-femtovg", feature = "renderer-femtovg-wgpu")
))]
async fn wait_until(transition: &str, mut condition: impl FnMut() -> bool) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    std::future::poll_fn(|cx| {
        if condition() {
            return std::task::Poll::Ready(());
        }
        assert!(std::time::Instant::now() < deadline, "{transition} timed out");
        let waker = cx.waker().clone();
        slint::Timer::single_shot(std::time::Duration::from_millis(10), move || waker.wake());
        std::task::Poll::Pending
    })
    .await;
}

#[cfg(all(
    target_os = "macos",
    any(feature = "renderer-femtovg", feature = "renderer-femtovg-wgpu")
))]
fn main() {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use slint::ComponentHandle;
    use slint::winit_030::WinitWindowAccessor;
    use std::cell::Cell;
    use std::rc::Rc;

    let Some(renderer) =
        std::env::args().find_map(|arg| arg.strip_prefix("--renderer=").map(str::to_owned))
    else {
        for renderer in [
            #[cfg(feature = "renderer-femtovg")]
            "femtovg",
            #[cfg(feature = "renderer-femtovg-wgpu")]
            "femtovg-wgpu",
        ] {
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .arg(format!("--renderer={renderer}"))
                .status()
                .unwrap();
            assert!(status.success(), "{renderer} regression test failed");
        }
        return;
    };
    slint::slint! {
        #[slint_edition = "1.19"]
        export component App inherits Window {
            width: 100px;
            height: 100px;
            in property <color> fill: red;
            background: root.fill;
        }
    }
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name(renderer.clone())
        .select()
        .unwrap();

    slint::spawn_local(async move {
        let app = App::new().unwrap();
        let native = app.window().winit_window().await.unwrap();
        let RawWindowHandle::AppKit(handle) = native.window_handle().unwrap().as_raw() else {
            panic!("expected an AppKit window");
        };
        // SAFETY: The native window owns this view and remains alive throughout the test.
        let view = unsafe { handle.ns_view.cast::<objc2_app_kit::NSView>().as_ref() };
        let ns_window = view.window().unwrap();
        ns_window.setAlphaValue(0.75);
        let initial_alpha = ns_window.alphaValue();
        let frames = Rc::new(Cell::new(0));
        let needs_fresh_frame = Rc::new(Cell::new(true));
        let protected_frames = Rc::new(Cell::new(true));
        let protected = protected_frames.clone();
        let frame_count = frames.clone();
        let fresh_frame = needs_fresh_frame.clone();
        let window = ns_window.clone();
        app.window()
            .set_rendering_notifier(move |state, _| {
                if matches!(state, slint::RenderingState::AfterRendering) {
                    if fresh_frame.replace(false) {
                        protected.set(
                            protected.get() && (!window.isVisible() || window.alphaValue() == 0.),
                        );
                    }
                    frame_count.set(frame_count.get() + 1);
                }
            })
            .unwrap();

        app.show().unwrap();
        wait_until("render before reveal", || {
            !needs_fresh_frame.get() && ns_window.alphaValue() == initial_alpha
        })
        .await;
        assert!(ns_window.isVisible());
        for (color, wait_for_occlusion) in [
            (slint::Color::from_rgb_u8(0, 255, 0), false),
            (slint::Color::from_rgb_u8(0, 0, 255), true),
        ] {
            app.hide().unwrap();
            if wait_for_occlusion {
                wait_until("hidden occlusion state", || {
                    !ns_window
                        .occlusionState()
                        .contains(objc2_app_kit::NSWindowOcclusionState::Visible)
                })
                .await;
            }
            app.set_fill(color);
            needs_fresh_frame.set(true);
            app.show().unwrap();
            wait_until("render before reveal", || {
                !needs_fresh_frame.get() && ns_window.alphaValue() == initial_alpha
            })
            .await;
            assert!(ns_window.isVisible());
        }
        assert!(protected_frames.get(), "the old frame was revealed before rendering");
        app.hide().unwrap();
        wait_until("hidden occlusion state", || {
            !ns_window.occlusionState().contains(objc2_app_kit::NSWindowOcclusionState::Visible)
        })
        .await;
        app.show().unwrap();
        app.hide().unwrap();
        assert_eq!(ns_window.alphaValue(), initial_alpha);
        needs_fresh_frame.set(true);
        app.show().unwrap();
        wait_until("reveal after interrupted show", || {
            !needs_fresh_frame.get() && ns_window.alphaValue() == initial_alpha
        })
        .await;

        assert!(protected_frames.get(), "the old frame was revealed before rendering");
        for (color, native_minimize) in [
            (slint::Color::from_rgb_u8(255, 255, 0), false),
            (slint::Color::from_rgb_u8(0, 255, 255), true),
        ] {
            if native_minimize {
                ns_window.miniaturize(None);
            } else {
                app.window().set_minimized(true);
            }
            wait_until("minimize", || ns_window.isMiniaturized() && app.window().is_minimized())
                .await;
            app.set_fill(color);
            let before = frames.get();
            app.window().set_minimized(false);
            wait_until("restore and redraw", || {
                !ns_window.isMiniaturized() && frames.get() > before
            })
            .await;
            assert!(ns_window.isVisible());
            assert_eq!(ns_window.alphaValue(), initial_alpha);
        }
        app.hide().unwrap();
        assert_eq!(ns_window.alphaValue(), initial_alpha);
        slint::quit_event_loop().unwrap();
    })
    .unwrap();
    slint::run_event_loop_until_quit().unwrap();
}

#[cfg(not(all(
    target_os = "macos",
    any(feature = "renderer-femtovg", feature = "renderer-femtovg-wgpu")
)))]
fn main() {}
