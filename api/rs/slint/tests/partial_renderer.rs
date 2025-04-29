// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_renderer_skia::skia_safe;
use i_slint_renderer_skia::SkiaRenderer;
use i_slint_renderer_skia::SkiaSharedContext;
use slint::platform::software_renderer::{
    MinimalSoftwareWindow, PremultipliedRgbaColor, SoftwareRenderer, TargetPixel,
};
use slint::platform::{PlatformError, WindowAdapter};
use slint::{Model, PhysicalPosition, PhysicalSize, SharedPixelBuffer};
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

#[derive(Clone, Copy, Default)]
struct TestPixel(bool);

impl TargetPixel for TestPixel {
    fn blend(&mut self, _color: PremultipliedRgbaColor) {
        *self = Self(true);
    }

    fn from_rgb(_red: u8, _green: u8, _blue: u8) -> Self {
        Self(true)
    }
}

#[track_caller]
fn do_test_render_region(renderer: &SoftwareRenderer, x: i32, y: i32, x2: i32, y2: i32) {
    let mut buffer = vec![TestPixel(false); 500 * 500];
    let r = renderer.render(buffer.as_mut_slice(), 500);
    assert_eq!(r.bounding_box_size(), PhysicalSize { width: (x2 - x) as _, height: (y2 - y) as _ });
    assert_eq!(r.bounding_box_origin(), PhysicalPosition { x, y });

    let mut has_one_pixel = false;
    for py in 0..500 {
        for px in 0..500 {
            let in_bounding_box = (x..x2).contains(&(px as i32)) && (y..y2).contains(&(py as i32));
            if !in_bounding_box {
                assert!(!buffer[py * 500 + px].0, "Something written outside of bounding box in  {px},{py}   - (x={x},y={y},x2={x2},y2={y2})")
            } else if buffer[py * 500 + px].0 {
                has_one_pixel = true;
            }
        }
    }
    assert!(has_one_pixel, "Nothing was rendered");
}

struct SkiaTestWindow {
    window: slint::Window,
    renderer: SkiaRenderer,
    needs_redraw: Cell<bool>,
    size: Cell<slint::PhysicalSize>,
    render_buffer: Rc<SkiaTestSoftwareBuffer>,
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
            render_buffer,
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

    fn last_dirty_region_bounding_box_size(&self) -> Option<slint::LogicalSize> {
        self.render_buffer.last_dirty_region.borrow().as_ref().map(|r| {
            let size = r.bounding_rect().size;
            slint::LogicalSize::new(size.width as _, size.height as _)
        })
    }
    fn last_dirty_region_bounding_box_origin(&self) -> Option<slint::LogicalPosition> {
        self.render_buffer.last_dirty_region.borrow().as_ref().map(|r| {
            let origin = r.bounding_rect().origin;
            slint::LogicalPosition::new(origin.x as _, origin.y as _)
        })
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
    last_dirty_region: RefCell<Option<i_slint_core::item_rendering::DirtyRegion>>,
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
            Option<i_slint_core::item_rendering::DirtyRegion>,
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
fn simple() {
    slint::slint! {
        export component Ui inherits Window {
            in property <color> c: yellow;
            background: black;
            Rectangle {
                x: 1phx;
                y: 80phx;
                width: 15phx;
                height: 17phx;
                background: red;
            }
            Rectangle {
                x: 10phx;
                y: 19phx;
                Rectangle {
                    x: 5phx;
                    y: 80phx;
                    width: 12phx;
                    height: 13phx;
                    background: c;
                }
                Rectangle {
                    x: 50phx;
                    y: 8phx;
                    width: 15phx;
                    height: 17phx;
                    background: c;
                }
            }
        }
    }

    slint::platform::set_platform(Box::new(TestPlatform)).ok();
    let ui = Ui::new().unwrap();
    let window = WINDOW.with(|x| x.clone());
    window.set_size(slint::PhysicalSize::new(180, 260));
    ui.show().unwrap();
    assert!(window.draw_if_needed(|renderer| {
        do_test_render_region(renderer, 0, 0, 180, 260);
    }));
    assert!(!window.draw_if_needed(|_| { unreachable!() }));
    ui.set_c(slint::Color::from_rgb_u8(45, 12, 13));
    assert!(window.draw_if_needed(|renderer| {
        do_test_render_region(renderer, 10 + 5, 19 + 8, 10 + 50 + 15, 19 + 80 + 13);
    }));
    ui.set_c(slint::Color::from_rgb_u8(45, 12, 13));
    assert!(!window.draw_if_needed(|_| { unreachable!() }));
}

#[test]
fn visibility() {
    slint::slint! {
        export component Ui inherits Window {
            in property <bool> c : true;
            background: black;
            Rectangle {
                x: 10phx;
                y: 19phx;
                Rectangle {
                    x: 5phx;
                    y: 80phx;
                    width: 12phx;
                    height: 13phx;
                    background: red;
                    visible: c;
                }
                Rectangle {
                    x: 50phx;
                    y: 8phx;
                    width: 15phx;
                    height: 17phx;
                    background: gray;
                    visible: !c;
                }
            }
        }
    }

    slint::platform::set_platform(Box::new(TestPlatform)).ok();
    let ui = Ui::new().unwrap();
    let window = WINDOW.with(|x| x.clone());
    window.set_size(slint::PhysicalSize::new(180, 260));
    ui.show().unwrap();
    assert!(window.draw_if_needed(|renderer| {
        do_test_render_region(renderer, 0, 0, 180, 260);
    }));
    assert!(!window.draw_if_needed(|_| { unreachable!() }));
    ui.set_c(false);
    assert!(window.draw_if_needed(|renderer| {
        do_test_render_region(renderer, 10 + 5, 19 + 8, 10 + 50 + 15, 19 + 80 + 13);
    }));
    assert!(!window.draw_if_needed(|_| { unreachable!() }));
    ui.set_c(true);
    assert!(window.draw_if_needed(|renderer| {
        do_test_render_region(renderer, 10 + 5, 19 + 8, 10 + 50 + 15, 19 + 80 + 13);
    }));
    assert!(!window.draw_if_needed(|_| { unreachable!() }));
}

#[test]
fn if_condition() {
    slint::slint! {
        export component Ui inherits Window {
            in property <bool> c : true;
            background: black;
            if c: Rectangle {
                x: 45px;
                y: 45px;
                background: pink;
                width: 32px;
                height: 3px;
            }
        }
    }

    slint::platform::set_platform(Box::new(TestPlatform)).ok();
    let ui = Ui::new().unwrap();
    let window = WINDOW.with(|x| x.clone());
    window.set_size(slint::PhysicalSize::new(180, 260));
    ui.show().unwrap();
    assert!(window.draw_if_needed(|renderer| {
        do_test_render_region(renderer, 0, 0, 180, 260);
    }));
    assert!(!window.draw_if_needed(|_| { unreachable!() }));
    ui.set_c(false);
    assert!(window.draw_if_needed(|renderer| {
        // Currently we redraw when a condition becomes false because we don't track the position otherwise
        do_test_render_region(renderer, 0, 0, 180, 260);
    }));
    assert!(!window.draw_if_needed(|_| { unreachable!() }));
    ui.set_c(true);
    assert!(window.draw_if_needed(|renderer| {
        do_test_render_region(renderer, 45, 45, 45 + 32, 45 + 3);
    }));
    assert!(!window.draw_if_needed(|_| { unreachable!() }));
}

#[test]
fn list_view() {
    slint::slint! {
        // We can't rely on the style as they are all different so implement our own in very basic terms
        component ListView inherits Flickable {
            out property <length> visible-width <=> self.width;
            out property <length> visible-height <=> self.height;
            @children
        }
        export component Ui inherits Window {
            width: 300px;
            height: 300px;
            in property <[int]> model;
            ListView {
                x: 20px; y: 10px; width: 100px; height: 90px;
                for x in model: Rectangle {
                    background: x == 1 ? red : blue;
                    height: 10px;
                    width: 25px;
                }

            }
        }
    }

    slint::platform::set_platform(Box::new(TestPlatform)).ok();
    let ui = Ui::new().unwrap();
    let window = WINDOW.with(|x| x.clone());
    window.set_size(slint::PhysicalSize::new(300, 300));
    ui.show().unwrap();
    assert!(window.draw_if_needed(|renderer| {
        do_test_render_region(renderer, 0, 0, 300, 300);
    }));
    assert!(!window.draw_if_needed(|_| { unreachable!() }));
    let model = std::rc::Rc::new(slint::VecModel::from(vec![0]));
    ui.set_model(model.clone().into());

    const LV_X: i32 = 20;
    const LV_Y: i32 = 10;

    assert!(window.draw_if_needed(|renderer| {
        do_test_render_region(renderer, LV_X, LV_Y, LV_X + 25, LV_Y + 10);
    }));
    assert!(!window.draw_if_needed(|_| { unreachable!() }));

    model.insert(0, 1);
    assert!(window.draw_if_needed(|renderer| {
        do_test_render_region(renderer, LV_X, LV_Y, LV_X + 25, LV_Y + 20);
    }));
    assert!(!window.draw_if_needed(|_| { unreachable!() }));
    model.set_row_data(1, 1);
    assert!(window.draw_if_needed(|renderer| {
        do_test_render_region(renderer, LV_X, LV_Y + 10, LV_X + 25, LV_Y + 20);
    }));
    assert!(!window.draw_if_needed(|_| { unreachable!() }));
    model.set_vec(vec![0, 0]);
    assert!(window.draw_if_needed(|renderer| {
        // Currently, when ItemTree are removed, we redraw the whole window.
        do_test_render_region(renderer, 0, 0, 300, 300);
    }));
    assert!(!window.draw_if_needed(|_| { unreachable!() }));
    model.remove(1);
    assert!(window.draw_if_needed(|renderer| {
        // Currently, when ItemTree are removed, we redraw the whole window.
        do_test_render_region(renderer, 0, 0, 300, 300);
    }));
    assert!(!window.draw_if_needed(|_| { unreachable!() }));
}

#[test]
/// test for #6932
fn scale_factor() {
    slint::slint! {
        export component Ui inherits Window {
        }
    }

    slint::platform::set_platform(Box::new(TestPlatform)).ok();
    let ui = Ui::new().unwrap();
    let window = WINDOW.with(|x| x.clone());
    window.set_size(slint::PhysicalSize::new(500, 500));
    window.dispatch_event(slint::platform::WindowEvent::ScaleFactorChanged { scale_factor: 1.33 });
    ui.show().unwrap();
    assert!(window.draw_if_needed(|renderer| {
        do_test_render_region(renderer, 0, 0, 500, 500);
    }));
}

#[test]
fn rotated_image() {
    slint::slint! {
        export component Ui inherits Window {
            in property <angle> rotation <=> i.rotation-angle;
            in property <length> x-pos <=> i.x;
            background: black;
            i := Image {
                x: 50px;
                y: 50px;
                width: 50px;
                height: 150px;
                source: @image-url("../../../logo/slint-logo-full-dark.png");
            }
        }
    }

    slint::platform::set_platform(Box::new(TestPlatform)).ok();

    let window = SKIA_WINDOW.with(|w| w.clone());
    NEXT_WINDOW_CHOICE.with(|choice| {
        *choice.borrow_mut() = Some(window.clone());
    });
    let ui = Ui::new().unwrap();
    window.set_size(slint::PhysicalSize::new(250, 250).into());
    ui.show().unwrap();

    assert!(window.draw_if_needed());
    assert_eq!(
        window.last_dirty_region_bounding_box_size(),
        Some(slint::LogicalSize { width: 250., height: 250. })
    );
    assert_eq!(
        window.last_dirty_region_bounding_box_origin(),
        Some(slint::LogicalPosition { x: 0., y: 0. })
    );

    assert!(!window.draw_if_needed());

    ui.set_x_pos(51.);

    assert!(window.draw_if_needed());
    assert_eq!(
        window.last_dirty_region_bounding_box_size(),
        Some(slint::LogicalSize { width: 51., height: 150. })
    );
    assert_eq!(
        window.last_dirty_region_bounding_box_origin(),
        Some(slint::LogicalPosition { x: 50., y: 50. })
    );

    ui.set_rotation(90.);

    assert!(window.draw_if_needed());
    assert_eq!(
        window.last_dirty_region_bounding_box_size(),
        Some(slint::LogicalSize { width: 150., height: 150. })
    );
    assert_eq!(
        window.last_dirty_region_bounding_box_origin(),
        Some(slint::LogicalPosition { x: 1., y: 50. })
    );
}

#[test]
fn window_background() {
    slint::slint! {
        export component Ui inherits Window {
            in property <color> c: yellow;
            background: c;
        }
    }

    slint::platform::set_platform(Box::new(TestPlatform)).ok();
    let ui = Ui::new().unwrap();
    let window = WINDOW.with(|x| x.clone());
    window.set_size(slint::PhysicalSize::new(180, 260));
    ui.show().unwrap();
    assert!(window.draw_if_needed(|renderer| {
        do_test_render_region(renderer, 0, 0, 180, 260);
    }));
    assert!(!window.draw_if_needed(|_| { unreachable!() }));
    ui.set_c(slint::Color::from_rgb_u8(45, 12, 13));
    assert!(window.draw_if_needed(|renderer| {
        do_test_render_region(renderer, 0, 0, 180, 260);
    }));
    ui.set_c(slint::Color::from_rgb_u8(45, 12, 13));
    assert!(!window.draw_if_needed(|_| { unreachable!() }));
}

#[test]
fn touch_area_doesnt_cause_redraw() {
    slint::slint! {
        export component Ui inherits Window {
            in property <color> c: yellow;
            in property <length> touch-area-1-x <=> ta1.x;
            in property <length> touch-area-2-x <=> ta2.x;
            in property <color> sole-pixel-color: red;
            background: black;
            ta1 := TouchArea {
                x: 10px;
                y: 0px;
                width: 20px;
                height: 40px;
                Rectangle {
                    x: 1phx;
                    y: 20phx;
                    width: 15phx;
                    height: 17phx;
                    background: c;
                }
            }
            ta2 := TouchArea {
                x: 10px;
                y: 0px;
                width: 20px;
                height: 40px;
            }
            sole-pixel := Rectangle {
                x: 60px;
                y: 0px;
                width: 1px;
                height: 1px;
                background: sole-pixel-color;
            }
        }
    }

    slint::platform::set_platform(Box::new(TestPlatform)).ok();
    let ui = Ui::new().unwrap();
    let window = WINDOW.with(|x| x.clone());
    window.set_size(slint::PhysicalSize::new(180, 260));
    ui.show().unwrap();
    assert!(window.draw_if_needed(|renderer| {
        do_test_render_region(renderer, 0, 0, 180, 260);
    }));
    assert!(!window.draw_if_needed(|_| { unreachable!() }));
    ui.set_c(slint::Color::from_rgb_u8(45, 12, 13));
    assert!(window.draw_if_needed(|renderer| {
        do_test_render_region(renderer, 10 + 1, 20, 10 + 1 + 15, 20 + 17);
    }));
    assert!(!window.draw_if_needed(|_| { unreachable!() }));
    ui.set_touch_area_1_x(20.);
    assert!(window.draw_if_needed(|renderer| {
        do_test_render_region(renderer, 10 + 1, 20, 10 + 1 + 15 + 10, 20 + 17);
    }));
    assert!(!window.draw_if_needed(|_| { unreachable!() }));
    ui.set_touch_area_2_x(20.);
    ui.set_sole_pixel_color(slint::Color::from_rgb_u8(45, 12, 13));
    // Moving the touch area should not cause it to be redrawn.
    assert!(window.draw_if_needed(|renderer| {
        do_test_render_region(renderer, 60, 0, 61, 1);
    }));
}

#[test]
fn shadow_redraw_beyond_geometry() {
    slint::slint! {
        export component Ui inherits Window {
            in property <length> x-pos: 10px;
            Rectangle {
                x: root.x-pos;
                y: 10px;
                width: 20px;
                height: 20px;
                drop-shadow-blur: 5px;
                drop-shadow-offset-x: 15px;
                drop-shadow-offset-y: 5px;
                drop-shadow-color: red;
            }
        }
    }

    slint::platform::set_platform(Box::new(TestPlatform)).ok();

    let window = SKIA_WINDOW.with(|w| w.clone());
    NEXT_WINDOW_CHOICE.with(|choice| {
        *choice.borrow_mut() = Some(window.clone());
    });
    let ui = Ui::new().unwrap();
    window.set_size(slint::PhysicalSize::new(250, 250).into());
    ui.show().unwrap();

    assert!(window.draw_if_needed());
    assert_eq!(
        window.last_dirty_region_bounding_box_size(),
        Some(slint::LogicalSize { width: 250., height: 250. })
    );
    assert_eq!(
        window.last_dirty_region_bounding_box_origin(),
        Some(slint::LogicalPosition { x: 0., y: 0. })
    );

    assert!(!window.draw_if_needed());

    ui.set_x_pos(20.);

    assert!(window.draw_if_needed());

    let shadow_width = /* rect width */ 20. + 2. * /* blur */ 5.;
    let move_delta = 10.;
    let shadow_height = /* rect height */ 20. + 2. * /*blur */ 5.;

    let old_shadow_x = /* rect x */ 10. + /* shadow offset */ 15. - /* blur */ 5.;
    let old_shadow_y = /* rect y */ 10. + /* shadow offset */ 5. - /* blur */ 5.;

    assert_eq!(
        window.last_dirty_region_bounding_box_size(),
        Some(slint::LogicalSize { width: shadow_width + move_delta, height: shadow_height })
    );
    assert_eq!(
        window.last_dirty_region_bounding_box_origin(),
        Some(slint::LogicalPosition { x: old_shadow_x, y: old_shadow_y })
    );
}

#[test]
fn text_alignment() {
    slint::slint! {
        export component Ui inherits Window {
            in property <color> c: green;
            Text {
                x: 10px;
                y: 10px;
                width: 200px;
                height: 50px;
                text: "Ok";
                color: c;
            }
        }
    }

    slint::platform::set_platform(Box::new(TestPlatform)).ok();

    let window = SKIA_WINDOW.with(|w| w.clone());
    NEXT_WINDOW_CHOICE.with(|choice| {
        *choice.borrow_mut() = Some(window.clone());
    });
    let ui = Ui::new().unwrap();
    window.set_size(slint::PhysicalSize::new(250, 250).into());
    ui.show().unwrap();

    assert!(window.draw_if_needed());
    assert_eq!(
        window.last_dirty_region_bounding_box_size(),
        Some(slint::LogicalSize { width: 250., height: 250. })
    );
    assert_eq!(
        window.last_dirty_region_bounding_box_origin(),
        Some(slint::LogicalPosition { x: 0., y: 0. })
    );

    assert!(!window.draw_if_needed());

    ui.set_c(slint::Color::from_rgb_u8(45, 12, 13));

    assert!(window.draw_if_needed());
    assert_eq!(
        window.last_dirty_region_bounding_box_size(),
        Some(slint::LogicalSize { width: 200., height: 50. })
    );
    assert_eq!(
        window.last_dirty_region_bounding_box_origin(),
        Some(slint::LogicalPosition { x: 10., y: 10. })
    );
}
