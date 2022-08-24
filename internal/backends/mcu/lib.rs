// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

// cSpell: ignore deque pico

#![doc = include_str!("README.md")]
#![doc(html_logo_url = "https://slint-ui.com/logo/slint-logo-square-light.svg")]
#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(any(feature = "pico-st7789", feature = "stm32h735g"), feature(alloc_error_handler))]

extern crate alloc;

use alloc::boxed::Box;
use alloc::vec;
use core::cell::RefCell;
use embedded_graphics::prelude::*;
use i_slint_core::lengths::*;
use i_slint_core::swrenderer as renderer;
#[cfg(all(not(feature = "std"), feature = "unsafe_single_core"))]
use i_slint_core::thread_local_ as thread_local;
#[cfg(all(not(feature = "std"), feature = "unsafe_single_core"))]
use i_slint_core::unsafe_single_core;

mod profiler;

/// The Pixel type of the backing store
pub type TargetPixel = embedded_graphics::pixelcolor::Rgb565;

pub trait Devices {
    fn screen_size(&self) -> PhysicalSize;
    /// If the device supports it, return the target buffer where to draw the frame. Must be width * height large.
    /// Also return the dirty area.
    fn get_buffer(&mut self) -> Option<&mut [TargetPixel]> {
        None
    }

    fn flush_frame(&mut self) {}

    /// Call the fill_line function with a buffer of `self.screen_size().width`.
    /// The parts within the dirty_region will be filled by the FnMut.
    /// this function should then send the buffer to the screen.
    fn render_line(
        &mut self,
        line: PhysicalLength,
        range: core::ops::Range<PhysicalLength>,
        fill_buffer: &mut dyn FnMut(&mut [TargetPixel]),
    );
    fn read_touch_event(&mut self) -> Option<i_slint_core::input::MouseEvent> {
        None
    }
    fn debug(&mut self, _: &str);
    fn time(&self) -> core::time::Duration {
        core::time::Duration::ZERO
    }
    fn sleep(&self, _duration: Option<core::time::Duration>) {}
}

impl<T: embedded_graphics::draw_target::DrawTarget> crate::Devices for T
where
    T::Error: core::fmt::Debug,
    T::Color: core::convert::From<TargetPixel>,
{
    fn screen_size(&self) -> PhysicalSize {
        let s = self.bounding_box().size;
        PhysicalSize::new(s.width as i16, s.height as i16)
    }

    fn render_line(
        &mut self,
        line: PhysicalLength,
        range: core::ops::Range<PhysicalLength>,
        fill_buffer: &mut dyn FnMut(&mut [TargetPixel]),
    ) {
        let mut buffer = vec![TargetPixel::default(); self.screen_size().width as usize];
        fill_buffer(&mut buffer);
        self.color_converted()
            .fill_contiguous(
                &embedded_graphics::primitives::Rectangle::new(
                    Point::new(range.start.get() as i32, line.get() as i32),
                    Size::new((range.end - range.start).get() as u32, 1),
                ),
                buffer.into_iter().skip(range.start.get() as usize),
            )
            .unwrap()
    }

    fn debug(&mut self, text: &str) {
        use embedded_graphics::{
            mono_font::{ascii, MonoTextStyle},
            text::Text,
        };
        let style = MonoTextStyle::new(&ascii::FONT_8X13, TargetPixel::RED.into());
        thread_local! { static LINE: core::cell::Cell<i16>  = core::cell::Cell::new(0) }
        LINE.with(|l| {
            let line = (l.get() + 1) % (self.screen_size().height / 13 - 2);
            l.set(line);
            Text::new(text, Point::new(3, (1 + line) as i32 * 13), style).draw(self).unwrap();
        });
    }
}

thread_local! { static DEVICES: RefCell<Option<Box<dyn Devices + 'static>>> = RefCell::new(None) }

mod the_backend {
    use super::*;
    use alloc::boxed::Box;
    use alloc::collections::VecDeque;
    use alloc::rc::{Rc, Weak};
    use alloc::string::String;
    use core::cell::RefCell;
    use i_slint_core::api::Window;
    use i_slint_core::window::{PlatformWindow, WindowHandleAccess};

    thread_local! { static WINDOWS: RefCell<Option<Rc<McuWindow>>> = RefCell::new(None) }
    thread_local! { static EVENT_QUEUE: RefCell<VecDeque<McuEvent>> = Default::default() }

    pub struct McuWindow {
        window: Window,
        self_weak: Weak<Self>,
        renderer: crate::renderer::SoftwareRenderer,
    }

    impl PlatformWindow for McuWindow {
        fn show(&self) {
            let w = self.window.window_handle();
            w.set_scale_factor(
                option_env!("SLINT_SCALE_FACTOR").and_then(|x| x.parse().ok()).unwrap_or(1.),
            );
            w.scale_factor_property().set_constant();
            WINDOWS.with(|x| *x.borrow_mut() = Some(self.self_weak.upgrade().unwrap()))
        }
        fn hide(&self) {
            WINDOWS.with(|x| *x.borrow_mut() = None)
        }
        fn request_redraw(&self) {
            EVENT_QUEUE.with(|q| q.borrow_mut().push_back(McuEvent::Repaint))
        }

        fn renderer(&self) -> &dyn i_slint_core::renderer::Renderer {
            &self.renderer
        }

        fn as_any(&self) -> &dyn core::any::Any {
            self
        }

        fn window(&self) -> &Window {
            &self.window
        }
    }

    enum McuEvent {
        Custom(Box<dyn FnOnce() + Send>),
        Quit,
        Repaint,
    }

    #[derive(Default)]
    struct MCUBackendInner {
        clipboard: String,
    }

    #[derive(Default)]
    pub struct MCUBackend {
        #[cfg(all(not(feature = "std"), feature = "unsafe_single_core"))]
        inner: RefCell<MCUBackendInner>,
        #[cfg(feature = "std")]
        inner: std::sync::Mutex<MCUBackendInner>,
    }

    #[cfg(feature = "unsafe_single_core")]
    unsafe impl Sync for MCUBackend {}
    #[cfg(feature = "unsafe_single_core")]
    unsafe impl Send for MCUBackend {}

    impl MCUBackend {
        fn with_inner<T>(&self, f: impl FnOnce(&mut MCUBackendInner) -> T) -> T {
            f(
                #[cfg(all(not(feature = "std"), feature = "unsafe_single_core"))]
                &mut self.inner.borrow_mut(),
                #[cfg(feature = "std")]
                &mut self.inner.lock().unwrap(),
            )
        }

        fn draw(&self, window: Rc<McuWindow>) {
            let runtime_window = window.window.window_handle();
            runtime_window.update_window_properties();

            DEVICES.with(|devices| {
                let mut devices = devices.borrow_mut();
                let devices = devices.as_mut().unwrap();
                let mut frame_profiler = profiler::Timer::new(&**devices);
                let screen_size = devices.screen_size();
                let scale_factor = runtime_window.scale_factor();
                let size = screen_size.to_f32() / scale_factor;
                runtime_window.set_window_item_geometry(size.width as _, size.height as _);

                if let Some(buffer) = devices.get_buffer() {
                    window.renderer.render(&window.window, buffer, screen_size.width_length());

                    devices.flush_frame();
                    frame_profiler.stop_profiling(&mut **devices, "=> frame total");
                    return;
                }

                struct BufferProvider<'a> {
                    screen_fill_profiler: profiler::Timer,
                    span_drawing_profiler: profiler::Timer,
                    prepare_scene_profiler: profiler::Timer,
                    compute_dirty_regions_profiler: profiler::Timer,
                    line_processing_profiler: profiler::Timer,
                    devices: &'a mut dyn Devices,
                }
                impl renderer::LineBufferProvider for BufferProvider<'_> {
                    type TargetPixel = super::TargetPixel;

                    fn process_line(
                        &mut self,
                        line: PhysicalLength,
                        range: core::ops::Range<PhysicalLength>,
                        render_fn: impl FnOnce(&mut [super::TargetPixel]),
                    ) {
                        let mut render_fn = Some(render_fn);
                        self.prepare_scene_profiler.stop(self.devices);
                        self.screen_fill_profiler.stop(self.devices);
                        self.span_drawing_profiler.start(self.devices);
                        self.devices.render_line(line, range.clone(), &mut |buffer| {
                            (render_fn.take().unwrap())(
                                &mut buffer[range.start.get() as usize..range.end.get() as usize],
                            );
                        });
                        self.span_drawing_profiler.stop(self.devices);
                        self.screen_fill_profiler.start(self.devices);
                    }
                }
                impl Drop for BufferProvider<'_> {
                    fn drop(&mut self) {
                        self.devices.flush_frame();
                        self.compute_dirty_regions_profiler
                            .stop_profiling(self.devices, "compute dirty regions");
                        self.prepare_scene_profiler.stop_profiling(self.devices, "prepare scene");
                        self.line_processing_profiler
                            .stop_profiling(self.devices, "line processing");
                        self.span_drawing_profiler.stop_profiling(self.devices, "span drawing");
                        self.screen_fill_profiler.stop_profiling(self.devices, "screen fill");
                    }
                }

                let buffer_provider = BufferProvider {
                    compute_dirty_regions_profiler: profiler::Timer::new(&**devices),
                    screen_fill_profiler: profiler::Timer::new_stopped(),
                    span_drawing_profiler: profiler::Timer::new_stopped(),
                    prepare_scene_profiler: profiler::Timer::new_stopped(),
                    line_processing_profiler: profiler::Timer::new_stopped(),
                    devices: &mut **devices,
                };

                window.renderer.render_by_line(&window.window, buffer_provider);
                frame_profiler.stop_profiling(&mut **devices, "=> frame total");
            });
        }
    }

    impl i_slint_core::platform::PlatformAbstraction for MCUBackend {
        fn create_window(&self) -> Rc<dyn i_slint_core::window::PlatformWindow> {
            Rc::new_cyclic(|self_weak| McuWindow {
                window: Window::new(self_weak.clone() as _),
                self_weak: self_weak.clone(),
                renderer: crate::renderer::SoftwareRenderer::new(
                    crate::renderer::DirtyTracking::DoubleBuffer,
                ),
            })
        }

        fn run_event_loop(&self, behavior: i_slint_core::platform::EventLoopQuitBehavior) {
            loop {
                i_slint_core::platform::update_timers_and_animations();
                match EVENT_QUEUE.with(|q| q.borrow_mut().pop_front()) {
                    Some(McuEvent::Quit) => break,
                    Some(McuEvent::Custom(e)) => e(),
                    Some(McuEvent::Repaint) => {
                        if let Some(window) = WINDOWS.with(|x| x.borrow().clone()) {
                            self.draw(window)
                        }
                    }
                    None => {
                        // TODO: sleep();
                    }
                }
                DEVICES.with(|devices| {
                    let e = devices.borrow_mut().as_mut().unwrap().read_touch_event();
                    if let Some(mut event) = e {
                        if let Some(window) = WINDOWS.with(|x| x.borrow().clone()) {
                            let w = window.window.window_handle();
                            // scale the event by the scale factor:
                            if let Some(p) = event.position() {
                                event.translate((p.cast() / w.scale_factor()).cast() - p);
                            }
                            w.process_mouse_input(event);
                        }
                    } else if i_slint_core::animations::CURRENT_ANIMATION_DRIVER
                        .with(|driver| !driver.has_active_animations())
                    {
                        let devices = devices.borrow();
                        let devices = devices.as_ref().unwrap();

                        let time_to_sleep =
                            i_slint_core::timers::TimerList::next_timeout().map(|instant| {
                                let time_to_sleep = instant - devices.time();
                                core::time::Duration::from_millis(time_to_sleep.0)
                            });
                        devices.sleep(time_to_sleep);
                    }
                });
                match behavior {
                    i_slint_core::platform::EventLoopQuitBehavior::QuitOnLastWindowClosed => {
                        if WINDOWS.with(|x| x.borrow().is_none()) {
                            break;
                        }
                    }
                    i_slint_core::platform::EventLoopQuitBehavior::QuitOnlyExplicitly => (),
                }
            }
        }

        fn new_event_loop_proxy(&self) -> Option<Box<dyn i_slint_core::platform::EventLoopProxy>> {
            struct Proxy;
            impl i_slint_core::platform::EventLoopProxy for Proxy {
                fn quit_event_loop(&self) {
                    EVENT_QUEUE.with(|q| q.borrow_mut().push_back(McuEvent::Quit));
                }

                fn invoke_from_event_loop(&self, event: Box<dyn FnOnce() + Send>) {
                    EVENT_QUEUE.with(|q| q.borrow_mut().push_back(McuEvent::Custom(event)));
                }
            }
            Some(Box::new(Proxy))
        }

        fn duration_since_start(&self) -> core::time::Duration {
            DEVICES.with(|devices| devices.borrow_mut().as_mut().unwrap().time())
        }

        fn set_clipboard_text(&self, text: &str) {
            self.with_inner(|inner| inner.clipboard = text.into())
        }

        fn clipboard_text(&self) -> Option<String> {
            let c = self.with_inner(|inner| inner.clipboard.clone());
            c.is_empty().then(|| c)
        }
    }
}

pub type NativeWidgets = ();
pub type NativeGlobals = ();
pub mod native_widgets {}
pub const HAS_NATIVE_STYLE: bool = false;

pub fn init_with_display<Display: Devices + 'static>(display: Display) {
    DEVICES.with(|d| *d.borrow_mut() = Some(Box::new(display)));
    i_slint_core::platform::set_platform_abstraction(Box::new(the_backend::MCUBackend::default()))
        .unwrap();
}

#[cfg(feature = "pico-st7789")]
mod pico_st7789;

#[cfg(feature = "pico-st7789")]
pub use pico_st7789::*;

#[cfg(feature = "stm32h735g")]
mod stm32h735g;

#[cfg(feature = "stm32h735g")]
pub use stm32h735g::*;

#[cfg(not(any(feature = "pico-st7789", feature = "stm32h735g")))]
pub use i_slint_core_macros::identity as entry;

#[cfg(not(any(feature = "pico-st7789", feature = "stm32h735g")))]
pub fn init() {}
