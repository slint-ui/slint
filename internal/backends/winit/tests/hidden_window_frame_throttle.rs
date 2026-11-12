// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! A window hidden and changed right after a frame, while the timer-based frame throttle is still running, must not
//! keep that timer ticking; showing the window again must draw the change.

#[cfg(not(target_vendor = "apple"))]
fn main() {
    use std::cell::Cell;
    use std::rc::Rc;
    use std::time::Duration;

    use slint::ComponentHandle;
    use slint::winit_030::{CustomApplicationHandler, EventResult, winit};

    slint::slint! {
        #[edition = "1.20"]
        export component App inherits Window {
            width: 100px;
            height: 100px;
            in property <color> fill: red;
            background: root.fill;
        }
    }

    #[derive(Default)]
    struct Counts {
        wake_ups: Cell<u32>,
        frames: Cell<u32>,
        // Run once, in the event loop's turn after the next frame.
        after_next_frame: Cell<Option<Box<dyn FnOnce()>>>,
    }

    struct Count(Rc<Counts>);

    impl CustomApplicationHandler for Count {
        fn window_event(
            &mut self,
            _event_loop: &winit::event_loop::ActiveEventLoop,
            _window_id: winit::window::WindowId,
            _winit_window: Option<&winit::window::Window>,
            _slint_window: Option<&slint::Window>,
            event: &winit::event::WindowEvent,
        ) -> EventResult {
            if matches!(event, winit::event::WindowEvent::RedrawRequested) {
                self.0.frames.set(self.0.frames.get() + 1);
                if let Some(then) = self.0.after_next_frame.take() {
                    slint::Timer::single_shot(Duration::ZERO, then);
                }
            }
            EventResult::Propagate
        }

        fn new_events(
            &mut self,
            _event_loop: &winit::event_loop::ActiveEventLoop,
            cause: winit::event::StartCause,
        ) -> EventResult {
            if matches!(cause, winit::event::StartCause::ResumeTimeReached { .. }) {
                self.0.wake_ups.set(self.0.wake_ups.get() + 1);
            }
            EventResult::Propagate
        }
    }

    async fn sleep(duration: Duration) {
        let done = Rc::new(Cell::new(false));
        let mut started = false;
        std::future::poll_fn(|cx| {
            if done.get() {
                return std::task::Poll::Ready(());
            }
            if !started {
                started = true;
                let (done, waker) = (done.clone(), cx.waker().clone());
                slint::Timer::single_shot(duration, move || {
                    done.set(true);
                    waker.wake();
                });
            }
            std::task::Poll::Pending
        })
        .await
    }

    let counts = Rc::new(Counts::default());
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .with_winit_custom_application_handler(Count(counts.clone()))
        .select()
        .unwrap();

    let app = App::new().unwrap();

    slint::spawn_local({
        let app = app.as_weak();
        async move {
            let app = app.unwrap();
            app.show().unwrap();
            sleep(Duration::from_millis(300)).await;
            // Hidden and changed right after a frame: the throttle's timer is still running then, until its next
            // tick a refresh interval later.
            counts.after_next_frame.set(Some(Box::new({
                let app = app.as_weak();
                move || {
                    let app = app.unwrap();
                    app.hide().unwrap();
                    app.set_fill(slint::Color::from_rgb_u8(0, 255, 0));
                }
            })));
            app.set_fill(slint::Color::from_rgb_u8(0, 0, 255));
            sleep(Duration::from_millis(300)).await;
            assert!(!app.window().is_visible(), "the window wasn't hidden after its frame");

            counts.wake_ups.set(0);
            sleep(Duration::from_secs(1)).await;
            // The one wake-up that ends the sleep, and a little slack.
            let wake_ups = counts.wake_ups.get();
            assert!(wake_ups <= 3, "{wake_ups} timer wake-ups in a second while hidden");

            counts.frames.set(0);
            app.show().unwrap();
            sleep(Duration::from_millis(300)).await;
            assert!(counts.frames.get() > 0, "the window wasn't drawn when shown again");
            app.hide().unwrap();
            slint::quit_event_loop().unwrap();
        }
    })
    .unwrap();
    slint::run_event_loop_until_quit().unwrap();
}

#[cfg(target_vendor = "apple")]
fn main() {}
