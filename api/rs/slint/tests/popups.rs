// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_renderer_skia::skia_safe;
use i_slint_renderer_skia::SkiaRenderer;
use i_slint_renderer_skia::SkiaSharedContext;
use slint::platform::software_renderer::MinimalSoftwareWindow;
use slint::platform::{PlatformError, WindowAdapter};
use slint::{PhysicalSize, SharedPixelBuffer};
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

thread_local! {
    static WINDOW: Rc<MinimalSoftwareWindow>  =
    MinimalSoftwareWindow::new(slint::platform::software_renderer::RepaintBufferType::ReusedBuffer);
    static SKIA_WINDOW: Rc<SkiaTestWindow> = SkiaTestWindow::new();
    static NEXT_WINDOW_CHOICE: Rc<RefCell<Option<Rc<dyn WindowAdapter>>>> = Rc::new(RefCell::new(None));
}

struct TestPlatform;
impl slint::platform::Platform for TestPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        Ok(NEXT_WINDOW_CHOICE.with(|choice| {
            choice.borrow_mut().take().unwrap_or_else(|| WINDOW.with(|x| x.clone()))
        }))
    }
}

struct SkiaTestWindow {
    window: slint::Window,
    renderer: SkiaRenderer,
    needs_redraw: Cell<bool>,
    size: Cell<slint::PhysicalSize>,
}

impl SkiaTestWindow {
    fn new() -> Rc<Self> {
        let render_buffer = Rc::new(SkiaTestSoftwareBuffer::default());
        let renderer = SkiaRenderer::new_with_surface(
            &SkiaSharedContext::default(),
            Box::new(i_slint_renderer_skia::software_surface::SoftwareSurface::from(
                render_buffer.clone(),
            )),
        );
        Rc::new_cyclic(|w: &Weak<Self>| Self {
            window: slint::Window::new(w.clone()),
            renderer,
            needs_redraw: Default::default(),
            size: Default::default(),
        })
    }

    fn draw_if_needed(&self) -> bool {
        if self.needs_redraw.replace(false) {
            self.renderer.render().unwrap();
            true
        } else {
            false
        }
    }
}

impl WindowAdapter for SkiaTestWindow {
    fn window(&self) -> &slint::Window {
        &self.window
    }

    fn size(&self) -> PhysicalSize {
        self.size.get()
    }

    fn renderer(&self) -> &dyn slint::platform::Renderer {
        &self.renderer
    }

    fn set_size(&self, size: slint::WindowSize) {
        self.size.set(size.to_physical(1.));
        self.window
            .dispatch_event(slint::platform::WindowEvent::Resized { size: size.to_logical(1.) })
    }

    fn request_redraw(&self) {
        self.needs_redraw.set(true);
    }
}

#[derive(Default)]
struct SkiaTestSoftwareBuffer {
    pixels: RefCell<Option<SharedPixelBuffer<slint::Rgba8Pixel>>>,
    last_dirty_region: RefCell<Option<i_slint_core::partial_renderer::DirtyRegion>>,
}

impl i_slint_renderer_skia::software_surface::RenderBuffer for SkiaTestSoftwareBuffer {
    fn with_buffer(
        &self,
        _window: &slint::Window,
        size: PhysicalSize,
        render_callback: &mut dyn FnMut(
            std::num::NonZeroU32,
            std::num::NonZeroU32,
            i_slint_renderer_skia::skia_safe::ColorType,
            u8,
            &mut [u8],
        ) -> Result<
            Option<i_slint_core::partial_renderer::DirtyRegion>,
            i_slint_core::platform::PlatformError,
        >,
    ) -> Result<(), i_slint_core::platform::PlatformError> {
        let Some((width, height)): Option<(std::num::NonZeroU32, std::num::NonZeroU32)> =
            size.width.try_into().ok().zip(size.height.try_into().ok())
        else {
            // Nothing to render
            return Ok(());
        };

        let mut shared_pixel_buffer = self.pixels.borrow_mut().take();

        if shared_pixel_buffer.as_ref().is_some_and(|existing_buffer| {
            existing_buffer.width() != width.get() || existing_buffer.height() != height.get()
        }) {
            shared_pixel_buffer = None;
        }

        let mut age = 1;
        let pixels = shared_pixel_buffer.get_or_insert_with(|| {
            age = 0;
            SharedPixelBuffer::new(width.get(), height.get())
        });

        let bytes = bytemuck::cast_slice_mut(pixels.make_mut_slice());
        *self.last_dirty_region.borrow_mut() =
            render_callback(width, height, skia_safe::ColorType::RGBA8888, age, bytes)?;

        *self.pixels.borrow_mut() = shared_pixel_buffer;

        Ok(())
    }
}

#[test]
fn interaction_with_dead_popup_impossible() {
    fn click(window: &Rc<SkiaTestWindow>) {
        window.window().dispatch_event(slint::platform::WindowEvent::PointerPressed {
            position: slint::LogicalPosition { x: 50.0, y: 50.0 },
            button: slint::platform::PointerEventButton::Left,
        });

        window.draw_if_needed();

        window.window().dispatch_event(slint::platform::WindowEvent::PointerReleased {
            position: slint::LogicalPosition { x: 50.0, y: 50.0 },
            button: slint::platform::PointerEventButton::Left,
        });

        window.draw_if_needed();
    }

    slint::slint! {
            export component Ui inherits Window {
                property <bool> condition: true;
                out property <int> result;
                out property <int> under;
                out property <int> open;

                width: 100px;
                height: 100px;

                TouchArea {
                    clicked => {
                        root.under += 1;
                        debug("under" , root.under);
                    }
                }

                if condition: TouchArea {
                    clicked => {
                        root.open += 1;
                        debug("open" , root.open);
                        popup.show();
                    }
                    popup := PopupWindow {
                        close-policy: close-on-click-outside;
                        width: 100%;
                        height: 100%;
                        Rectangle {
                            width: 100%;
                            height: 100%;
                            background: green;
                        }
                        TouchArea {
                            clicked => {
                                root.result += 1;
                                debug("result", root.result);
                                root.condition = false;
                                debug("CONDITION CLEARED");
                            }
                        }
                    }
                }
            }
    }

    slint::platform::set_platform(Box::new(TestPlatform)).ok();

    let window = SKIA_WINDOW.with(|w| w.clone());
    NEXT_WINDOW_CHOICE.with(|choice| {
        *choice.borrow_mut() = Some(window.clone());
    });
    let ui = Ui::new().unwrap();
    window.set_size(slint::PhysicalSize::new(100, 100).into());

    assert_eq!(ui.get_result(), 0);
    assert_eq!(ui.get_under(), 0);
    assert_eq!(ui.get_open(), 0);

    click(&window);
    assert_eq!(ui.get_result(), 0);
    assert_eq!(ui.get_under(), 0);
    assert_eq!(ui.get_open(), 1);

    click(&window);
    assert_eq!(ui.get_result(), 1);
    assert_eq!(ui.get_under(), 0);
    assert_eq!(ui.get_open(), 1);

    click(&window);
    assert_eq!(ui.get_result(), 1);
    assert_eq!(ui.get_under(), 1);
    assert_eq!(ui.get_open(), 1);

    click(&window);
    assert_eq!(ui.get_result(), 1);
    assert_eq!(ui.get_under(), 2);
    assert_eq!(ui.get_open(), 1);
}

#[test]
fn interaction_with_dead_popup_panics() {
    fn click(window: &Rc<SkiaTestWindow>) {
        window.window().dispatch_event(slint::platform::WindowEvent::PointerPressed {
            position: slint::LogicalPosition { x: 50.0, y: 50.0 },
            button: slint::platform::PointerEventButton::Left,
        });

        window.draw_if_needed();

        window.window().dispatch_event(slint::platform::WindowEvent::PointerReleased {
            position: slint::LogicalPosition { x: 50.0, y: 50.0 },
            button: slint::platform::PointerEventButton::Left,
        });

        window.draw_if_needed();
    }

    slint::slint! {
        export component Ui inherits Window {
            in-out property <bool> condition: true;

            width: 100px;
            height: 100px;

            t := Timer {
                running: false;
                interval: 1ms;
                triggered => {
                    root.condition = false;
                    self.running = false;
                }
            }
            if condition: TouchArea {
                clicked => {
                    popup.show();
                    t.start();
                }
                popup := PopupWindow {
                    width: 100%;
                    height: 100%;
                    Rectangle {
                        width: 100%;
                        height: 100%;
                        background: green;
                    }
                    TouchArea {
                        clicked => {
                            popup.close();
                        }
                    }
                }
            }
        }
    }

    slint::platform::set_platform(Box::new(TestPlatform)).ok();

    let window = SKIA_WINDOW.with(|w| w.clone());
    NEXT_WINDOW_CHOICE.with(|choice| {
        *choice.borrow_mut() = Some(window.clone());
    });
    let ui = Ui::new().unwrap();
    window.set_size(slint::PhysicalSize::new(100, 100).into());

    assert!(ui.get_condition());

    click(&window);
    assert!(ui.get_condition());

    ui.set_condition(false);

    click(&window);
    assert!(!ui.get_condition());
}
