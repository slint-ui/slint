// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! On Windows, the software renderer presents to a DXGI swap chain.
//! A whole frame, and the damage of the frames after it, must reach what DWM composes.

#[cfg(not(target_os = "windows"))]
fn main() {}

#[cfg(target_os = "windows")]
fn main() {
    use std::cell::Cell;
    use std::rc::Rc;
    use std::time::Duration;

    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use slint::ComponentHandle;
    use slint::winit_030::{CustomApplicationHandler, EventResult, WinitWindowAccessor, winit};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Gdi::{
        CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetPixel,
        ReleaseDC, SelectObject,
    };
    use windows::Win32::Storage::Xps::{PRINT_WINDOW_FLAGS, PW_CLIENTONLY, PrintWindow};

    slint::slint! {
        #[edition = "1.20"]
        export component App inherits Window {
            width: 120px;
            height: 80px;
            background: #ff0000;
            in property <color> first: #ff0000;
            in property <color> second: #ff0000;
            Rectangle {
                x: 10px;
                y: 10px;
                width: 30px;
                height: 30px;
                background: root.first;
            }
            Rectangle {
                x: 70px;
                y: 40px;
                width: 30px;
                height: 30px;
                background: root.second;
            }
        }
    }

    struct CountFrames(Rc<Cell<u32>>);

    impl CustomApplicationHandler for CountFrames {
        fn window_event(
            &mut self,
            _event_loop: &winit::event_loop::ActiveEventLoop,
            _window_id: winit::window::WindowId,
            _winit_window: Option<&winit::window::Window>,
            _slint_window: Option<&slint::Window>,
            event: &winit::event::WindowEvent,
        ) -> EventResult {
            if matches!(event, winit::event::WindowEvent::RedrawRequested) {
                self.0.set(self.0.get() + 1);
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

    /// The color as `0xRRGGBB` at a physical position of the window, as DWM composes it.
    fn composed_color(window: HWND, size: slint::PhysicalSize, x: i32, y: i32) -> u32 {
        unsafe {
            let window_dc = GetDC(Some(window));
            let memory_dc = CreateCompatibleDC(Some(window_dc));
            let bitmap = CreateCompatibleBitmap(window_dc, size.width as i32, size.height as i32);
            let previous = SelectObject(memory_dc, bitmap.into());
            // 2 is PW_RENDERFULLCONTENT, which the bindings lack.
            // Without it, PrintWindow skips content presented through DXGI.
            let printed = PrintWindow(window, memory_dc, PRINT_WINDOW_FLAGS(PW_CLIENTONLY.0 | 2));
            let color = GetPixel(memory_dc, x, y).0;
            SelectObject(memory_dc, previous);
            let _ = DeleteObject(bitmap.into());
            let _ = DeleteDC(memory_dc);
            ReleaseDC(Some(window), window_dc);
            assert!(printed.as_bool(), "PrintWindow failed");
            (color & 0xff) << 16 | color & 0xff00 | (color >> 16) & 0xff
        }
    }

    let frames = Rc::new(Cell::new(0));
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("software".into())
        .with_winit_custom_application_handler(CountFrames(frames.clone()))
        .select()
        .unwrap();

    let app = App::new().unwrap();

    slint::spawn_local({
        let app = app.as_weak();
        async move {
            let app = app.unwrap();
            app.show().unwrap();
            sleep(Duration::from_millis(500)).await;
            // Blacksmith's Windows runners never send `RedrawRequested`.
            if frames.get() > 0 {
                app.set_first(slint::Color::from_rgb_u8(0, 0, 255));
                sleep(Duration::from_millis(200)).await;
                app.set_second(slint::Color::from_rgb_u8(0, 255, 0));
                sleep(Duration::from_millis(300)).await;

                let native = app.window().winit_window().await.unwrap();
                let RawWindowHandle::Win32(handle) = native.window_handle().unwrap().as_raw()
                else {
                    panic!("expected a Win32 window");
                };
                let window = HWND(handle.hwnd.get() as *mut core::ffi::c_void);
                let size = app.window().size();
                let scale = app.window().scale_factor();
                let at = |x: f32, y: f32| {
                    composed_color(window, size, (x * scale) as i32, (y * scale) as i32)
                };
                assert_eq!(at(60., 5.), 0xff0000, "the background of the first frame");
                assert_eq!(at(25., 25.), 0x0000ff, "the damage of the second frame");
                assert_eq!(at(85., 55.), 0x00ff00, "the damage of the third frame");
            }
            app.hide().unwrap();
            slint::quit_event_loop().unwrap();
        }
    })
    .unwrap();
    slint::run_event_loop_until_quit().unwrap();
}
