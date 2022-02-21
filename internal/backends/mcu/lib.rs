// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

#![doc = include_str!("README.md")]
#![doc(html_logo_url = "https://slint-ui.com/logo/slint-logo-square-light.svg")]
#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(any(feature = "pico-st7789", feature = "stm32h735g"), feature(alloc_error_handler))]

extern crate alloc;

use alloc::boxed::Box;
use core::cell::RefCell;
use embedded_graphics::prelude::*;
use i_slint_core::items::{Item, WindowItem};
use i_slint_core::lengths::*;
use i_slint_core::swrenderer as renderer;
#[cfg(all(not(feature = "std"), feature = "unsafe_single_core"))]
use i_slint_core::thread_local_ as thread_local;
#[cfg(all(not(feature = "std"), feature = "unsafe_single_core"))]
use i_slint_core::unsafe_single_core;

mod profiler;
#[cfg(feature = "simulator")]
mod simulator;

#[cfg(feature = "simulator")]
use simulator::event_loop;

/// The Pixel type of the backing store
pub type TargetPixel = embedded_graphics::pixelcolor::Rgb565;

pub trait Devices {
    fn screen_size(&self) -> PhysicalSize;
    /// Called before the frame is being drawn, with the dirty region. Return the actual dirty region
    fn prepare_frame(&mut self, dirty_region: PhysicalRect) -> PhysicalRect {
        dirty_region
    }
    fn fill_region(&mut self, region: PhysicalRect, pixels: &[TargetPixel]);
    fn flush_frame(&mut self) {}
    fn read_touch_event(&mut self) -> Option<i_slint_core::input::MouseEvent> {
        None
    }
    fn debug(&mut self, _: &str);
    fn time(&self) -> core::time::Duration {
        core::time::Duration::ZERO
    }
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

    fn fill_region(&mut self, region: PhysicalRect, pixels: &[TargetPixel]) {
        self.color_converted()
            .fill_contiguous(
                &embedded_graphics::primitives::Rectangle::new(
                    Point::new(region.origin.x as i32, region.origin.y as i32),
                    Size::new(region.size.width as u32, region.size.height as u32),
                ),
                pixels.iter().copied(),
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
            Text::new(text, Point::new(3, line as i32 * 13 + 1), style).draw(self).unwrap();
        });
    }
}

thread_local! { static DEVICES: RefCell<Option<Box<dyn Devices + 'static>>> = RefCell::new(None) }
thread_local! { static LINE_RENDERER: RefCell<crate::renderer::LineRenderer> = RefCell::new(Default::default()) }

mod the_backend {
    use super::*;
    use alloc::boxed::Box;
    use alloc::collections::VecDeque;
    use alloc::rc::{Rc, Weak};
    use alloc::string::String;
    use core::cell::{Cell, RefCell};
    use core::pin::Pin;
    use i_slint_core::component::ComponentRc;
    use i_slint_core::graphics::{Color, Point, Rect, Size};
    use i_slint_core::items::ItemRef;
    use i_slint_core::window::PlatformWindow;
    use i_slint_core::window::Window;
    use i_slint_core::{Coord, ImageInner, StaticTextures};

    thread_local! { static WINDOWS: RefCell<Option<Rc<McuWindow>>> = RefCell::new(None) }

    pub struct McuWindow {
        backend: &'static MCUBackend,
        self_weak: Weak<Window>,
        background_color: Cell<Color>,
        initial_dirty_region_for_next_frame: Cell<i_slint_core::item_rendering::DirtyRegion>,
    }

    impl PlatformWindow for McuWindow {
        fn show(self: Rc<Self>) {
            let w = self.self_weak.upgrade().unwrap();
            w.set_scale_factor(
                option_env!("SLINT_SCALE_FACTOR").and_then(|x| x.parse().ok()).unwrap_or(1.),
            );
            w.scale_factor_property().set_constant();
            WINDOWS.with(|x| *x.borrow_mut() = Some(self))
        }
        fn hide(self: Rc<Self>) {
            WINDOWS.with(|x| *x.borrow_mut() = None)
        }
        fn request_redraw(&self) {
            self.backend.with_inner(|inner| inner.post_event(McuEvent::Repaint))
        }
        fn free_graphics_resources<'a>(
            &self,
            _: i_slint_core::component::ComponentRef,
            items: &mut dyn Iterator<Item = Pin<i_slint_core::items::ItemRef<'a>>>,
        ) {
            super::LINE_RENDERER.with(|renderer| {
                renderer.borrow().free_graphics_resources(items);
            });
        }

        fn show_popup(&self, popup: &ComponentRc, position: i_slint_core::graphics::Point) {
            let runtime_window = self.self_weak.upgrade().unwrap();
            let size = runtime_window.set_active_popup(i_slint_core::window::PopupWindow {
                location: i_slint_core::window::PopupWindowLocation::ChildWindow(position),
                component: popup.clone(),
            });

            let popup = ComponentRc::borrow_pin(popup);
            let popup_root = popup.as_ref().get_item_ref(0);
            if let Some(window_item) = ItemRef::downcast_pin(popup_root) {
                let width_property =
                    i_slint_core::items::WindowItem::FIELD_OFFSETS.width.apply_pin(window_item);
                let height_property =
                    i_slint_core::items::WindowItem::FIELD_OFFSETS.height.apply_pin(window_item);
                width_property.set(size.width);
                height_property.set(size.height);
            }
        }

        fn close_popup(&self, popup: &i_slint_core::window::PopupWindow) {
            match popup.location {
                i_slint_core::window::PopupWindowLocation::TopLevel(_) => {}
                i_slint_core::window::PopupWindowLocation::ChildWindow(offset) => {
                    let popup_component = ComponentRc::borrow_pin(&popup.component);
                    let popup_root = popup_component.as_ref().get_item_ref(0);
                    if let Some(window_item) = ItemRef::downcast_pin::<WindowItem>(popup_root) {
                        let popup_region = i_slint_core::properties::evaluate_no_tracking(|| {
                            window_item.geometry()
                        })
                        .translate(offset.to_vector());

                        if !popup_region.is_empty() {
                            self.initial_dirty_region_for_next_frame.set(
                                self.initial_dirty_region_for_next_frame
                                    .get()
                                    .union(&popup_region.to_box2d()),
                            );
                        }
                    }
                }
            }
        }

        fn request_window_properties_update(&self) {}
        fn apply_window_properties(&self, window_item: Pin<&i_slint_core::items::WindowItem>) {
            self.background_color.set(window_item.background());
        }
        fn apply_geometry_constraint(
            &self,
            _constraints_horizontal: i_slint_core::layout::LayoutInfo,
            _constraints_vertical: i_slint_core::layout::LayoutInfo,
        ) {
        }
        fn set_mouse_cursor(&self, _cursor: i_slint_core::items::MouseCursor) {}
        fn text_size(
            &self,
            font_request: i_slint_core::graphics::FontRequest,
            text: &str,
            max_width: Option<Coord>,
        ) -> Size {
            let runtime_window = self.self_weak.upgrade().unwrap();
            renderer::fonts::text_size(
                font_request.merge(&runtime_window.default_font_properties()),
                text,
                max_width,
                ScaleFactor::new(runtime_window.scale_factor()),
            )
            .to_untyped()
        }

        fn text_input_byte_offset_for_position(
            &self,
            _text_input: Pin<&i_slint_core::items::TextInput>,
            _pos: Point,
        ) -> usize {
            0
        }
        fn text_input_cursor_rect_for_byte_offset(
            &self,
            _text_input: Pin<&i_slint_core::items::TextInput>,
            _byte_offset: usize,
        ) -> Rect {
            Default::default()
        }
        fn as_any(&self) -> &dyn core::any::Any {
            self
        }
    }

    enum McuEvent {
        Custom(Box<dyn FnOnce() + Send>),
        Quit,
        Repaint,
    }

    #[derive(Default)]
    struct MCUBackendInner {
        event_queue: VecDeque<McuEvent>,
        clipboard: String,
    }

    impl MCUBackendInner {
        fn post_event(&mut self, event: McuEvent) {
            self.event_queue.push_back(event);
            // TODO! wake
        }
    }

    #[derive(Default)]
    pub struct MCUBackend {
        #[cfg(all(not(feature = "std"), feature = "unsafe_single_core"))]
        inner: RefCell<MCUBackendInner>,
        #[cfg(any(feature = "std", not(feature = "unsafe_single_core")))]
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
                #[cfg(any(feature = "std", not(feature = "unsafe_single_core")))]
                &mut self.inner.lock().unwrap(),
            )
        }

        fn draw(&self, window: Rc<McuWindow>) {
            let runtime_window = window.self_weak.upgrade().unwrap();
            runtime_window.update_window_properties();

            DEVICES.with(|devices| {
                let mut devices = devices.borrow_mut();
                let devices = devices.as_mut().unwrap();
                let size = devices.screen_size().to_f32() / runtime_window.scale_factor();
                runtime_window.set_window_item_geometry(size.width as _, size.height as _);

                struct BufferProvider<'a> {
                    screen_fill_profiler: profiler::Timer,
                    span_drawing_profiler: profiler::Timer,
                    prepare_scene_profiler: profiler::Timer,
                    compute_dirty_regions_profiler: profiler::Timer,
                    line_processing_profiler: profiler::Timer,
                    devices: &'a mut dyn Devices,
                    line_buffer: alloc::vec::Vec<TargetPixel>,
                    dirty_region: PhysicalRect,
                }
                impl renderer::LineBufferProvider for BufferProvider<'_> {
                    type TargetPixel = super::TargetPixel;

                    fn set_dirty_region(&mut self, mut dirty_region: PhysicalRect) -> PhysicalRect {
                        self.compute_dirty_regions_profiler.stop(self.devices);
                        dirty_region = self.devices.prepare_frame(dirty_region);
                        self.dirty_region = dirty_region;
                        self.prepare_scene_profiler.start(self.devices);
                        dirty_region
                    }

                    fn process_line(
                        &mut self,
                        line: PhysicalLength,
                        render_fn: impl FnOnce(&mut [super::TargetPixel]),
                    ) {
                        self.prepare_scene_profiler.stop(self.devices);
                        self.span_drawing_profiler.start(self.devices);
                        render_fn(&mut self.line_buffer);
                        self.span_drawing_profiler.stop(self.devices);

                        self.screen_fill_profiler.start(self.devices);
                        self.devices.fill_region(
                            euclid::rect(
                                self.dirty_region.min_x(),
                                line.get(),
                                self.dirty_region.width(),
                                1,
                            ),
                            &self.line_buffer[self.dirty_region.min_x() as usize
                                ..self.dirty_region.max_x() as usize],
                        );
                        self.screen_fill_profiler.stop(self.devices);
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
                    line_buffer: alloc::vec![Default::default(); size.width as usize],
                    dirty_region: PhysicalRect::default(),
                };

                LINE_RENDERER.with(|renderer| {
                    renderer.borrow().render(
                        runtime_window,
                        window.initial_dirty_region_for_next_frame.take(),
                        buffer_provider,
                    )
                });
            });
        }
    }

    impl i_slint_core::backend::Backend for MCUBackend {
        fn create_window(&'static self) -> Rc<i_slint_core::window::Window> {
            i_slint_core::window::Window::new(|window| {
                Rc::new(McuWindow {
                    backend: self,
                    self_weak: window.clone(),
                    background_color: Color::from_rgb_u8(0, 0, 0).into(),
                    initial_dirty_region_for_next_frame: Default::default(),
                })
            })
        }

        fn run_event_loop(&'static self, behavior: i_slint_core::backend::EventLoopQuitBehavior) {
            loop {
                i_slint_core::timers::TimerList::maybe_activate_timers();
                i_slint_core::animations::update_animations();
                match self.with_inner(|inner| inner.event_queue.pop_front()) {
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
                            let w = window.self_weak.upgrade().unwrap();
                            // scale the event by the scale factor:
                            if let Some(p) = event.pos() {
                                event.translate((p.cast() / w.scale_factor()).cast() - p);
                            }
                            w.process_mouse_input(event);
                        }
                    }
                });
                match behavior {
                    i_slint_core::backend::EventLoopQuitBehavior::QuitOnLastWindowClosed => {
                        if WINDOWS.with(|x| x.borrow().is_none()) {
                            break;
                        }
                    }
                    i_slint_core::backend::EventLoopQuitBehavior::QuitOnlyExplicitly => (),
                }
            }
        }

        fn quit_event_loop(&'static self) {
            self.with_inner(|inner| inner.post_event(McuEvent::Quit))
        }

        fn register_bitmap_font(
            &'static self,
            font_data: &'static i_slint_core::graphics::BitmapFont,
        ) {
            crate::renderer::fonts::register_bitmap_font(font_data);
        }

        fn set_clipboard_text(&'static self, text: String) {
            self.with_inner(|inner| inner.clipboard = text)
        }

        fn clipboard_text(&'static self) -> Option<String> {
            let c = self.with_inner(|inner| inner.clipboard.clone());
            c.is_empty().then(|| c)
        }

        fn post_event(&'static self, event: Box<dyn FnOnce() + Send>) {
            self.with_inner(|inner| inner.post_event(McuEvent::Custom(event)));
        }

        fn image_size(
            &'static self,
            image: &i_slint_core::graphics::Image,
        ) -> i_slint_core::graphics::IntSize {
            let inner: &ImageInner = image.into();
            match inner {
                ImageInner::None => Default::default(),
                ImageInner::AbsoluteFilePath(_) | ImageInner::EmbeddedData { .. } => {
                    unimplemented!()
                }
                ImageInner::EmbeddedImage(buffer) => buffer.size(),
                ImageInner::StaticTextures(StaticTextures { original_size, .. }) => *original_size,
            }
        }

        #[cfg(feature = "std")]
        fn register_font_from_memory(
            &'static self,
            _data: &'static [u8],
        ) -> Result<(), Box<dyn std::error::Error>> {
            unimplemented!()
        }
        #[cfg(feature = "std")]
        fn register_font_from_path(
            &'static self,
            _path: &std::path::Path,
        ) -> Result<(), Box<dyn std::error::Error>> {
            unimplemented!()
        }

        fn duration_since_start(&'static self) -> core::time::Duration {
            DEVICES.with(|devices| devices.borrow_mut().as_mut().unwrap().time())
        }
    }
}

pub type NativeWidgets = ();
pub type NativeGlobals = ();
pub mod native_widgets {}
pub const HAS_NATIVE_STYLE: bool = false;

#[cfg(feature = "simulator")]
pub fn init() {
    i_slint_core::backend::instance_or_init(|| alloc::boxed::Box::new(simulator::SimulatorBackend));
}

pub fn init_with_display<Display: Devices + 'static>(display: Display) {
    DEVICES.with(|d| *d.borrow_mut() = Some(Box::new(display)));
    i_slint_core::backend::instance_or_init(|| {
        alloc::boxed::Box::new(the_backend::MCUBackend::default())
    });
}

#[cfg(not(any(
    feature = "pico-st7789",
    feature = "stm32h735g",
    feature = "simulator",
    feature = "terminal"
)))]
pub fn init() {
    struct EmptyDisplay;
    impl embedded_graphics::draw_target::DrawTarget for EmptyDisplay {
        type Color = embedded_graphics::pixelcolor::Rgb888;
        type Error = ();
        fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
        where
            I: IntoIterator<Item = embedded_graphics::Pixel<Self::Color>>,
        {
            let _ = pixels.into_iter().count();
            Ok(())
        }
    }
    impl embedded_graphics::geometry::OriginDimensions for EmptyDisplay {
        fn size(&self) -> embedded_graphics::geometry::Size {
            embedded_graphics::geometry::Size::new(320, 240)
        }
    }
    init_with_display(EmptyDisplay);
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

#[cfg(feature = "terminal")]
mod terminal;
#[cfg(feature = "terminal")]
pub use terminal::*;
