// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_core::api::PhysicalSize;
use i_slint_core::platform::{Platform, PlatformError};
use i_slint_core::renderer::Renderer;
use i_slint_core::window::WindowAdapter;
use i_slint_renderer_skia::{SkiaRenderer, SkiaSharedContext};
use slint_interpreter::ComponentHandle;

use crate::testing::poll_once;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

#[derive(Default)]
pub struct SkiaScreenshotBackend;

impl Platform for SkiaScreenshotBackend {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        Ok(Rc::new_cyclic(|self_weak| SkiaScreenshotWindow {
            window: i_slint_core::api::Window::new(self_weak.clone() as _),
            size: Default::default(),
            renderer: SkiaRenderer::default_software(&SkiaSharedContext::default()),
        }))
    }

    fn duration_since_start(&self) -> core::time::Duration {
        i_slint_core::animations::current_tick().into()
    }
}

pub struct SkiaScreenshotWindow {
    window: i_slint_core::api::Window,
    size: Cell<PhysicalSize>,
    renderer: SkiaRenderer,
}

impl WindowAdapter for SkiaScreenshotWindow {
    fn window(&self) -> &i_slint_core::api::Window {
        &self.window
    }

    fn size(&self) -> PhysicalSize {
        if self.size.get().width == 0 { PhysicalSize::new(64, 64) } else { self.size.get() }
    }

    fn set_size(&self, size: i_slint_core::api::WindowSize) {
        self.window.dispatch_event(i_slint_core::platform::WindowEvent::Resized {
            size: size.to_logical(self.window().scale_factor()),
        });
        self.size.set(size.to_physical(self.window().scale_factor()))
    }

    fn renderer(&self) -> &dyn Renderer {
        &self.renderer
    }

    fn update_window_properties(&self, properties: i_slint_core::window::WindowProperties<'_>) {
        if self.size.get().width == 0 {
            let c = properties.layout_constraints();
            self.size.set(c.preferred.to_physical(self.window.scale_factor()));
        }
    }
}

pub fn init_skia() {
    crate::testing::force_reference_os();

    i_slint_core::platform::set_platform(Box::new(SkiaScreenshotBackend::default()))
        .expect("platform already initialized");
}

pub struct TestCase {
    pub absolute_path: std::path::PathBuf,
    pub relative_path: std::path::PathBuf,
    pub reference_path: std::path::PathBuf,
}

pub fn run_test(testcase: TestCase) -> Result<(), Box<dyn std::error::Error>> {
    init_skia();

    let source = std::fs::read_to_string(&testcase.absolute_path)?;
    let mut compiler = slint_interpreter::Compiler::default();
    compiler.set_style("fluent".into());
    let compiled =
        poll_once(compiler.build_from_source(source, testcase.absolute_path.clone())).unwrap();

    if compiled.has_errors() {
        compiled.print_diagnostics();
        return Err(format!(
            "build error in {:?} \n {:?}",
            testcase.absolute_path,
            compiled.diagnostics().collect::<Vec<_>>()
        )
        .into());
    }

    let def = compiled.components().last().expect("There must be at least one exported component");
    let component = def.create().unwrap();
    component.show().unwrap();

    let screenshot = component.window().take_snapshot().unwrap();

    // Images are rendered a bit differently on macOs.
    // Elsewhere the tolerance covers a last-bit difference of 2 per channel: tagging the raster
    // target as sRGB puts Skia on its color managed pipeline, whose float math rounds slightly
    // differently between the Linux and the Windows build even though every conversion is sRGB to
    // sRGB and therefore a no-op.
    let base_threshold = if cfg!(target_os = "macos") { 33. } else { 4. };

    crate::testing::compare_images(
        testcase.reference_path.to_str().unwrap(),
        &screenshot,
        Default::default(),
        &crate::testing::TestCaseOptions { base_threshold, ..Default::default() },
    )?;

    Ok(())
}

// Compare renders within one run so font rasterization differences between platforms don't need golden images.
#[test]
fn text_alignment_anchor_stays_fixed() {
    init_skia();
    for (horizontal, x_fraction) in [("left", 0.0), ("center", 0.5), ("right", 1.0)] {
        for (vertical, y_fraction) in [("top", 0.0), ("center", 0.5), ("bottom", 1.0)] {
            for input in [false, true] {
                let item_type = if input { "TextInput" } else { "Text" };
                let selection = if input {
                    "selection-background-color: blue; selection-foreground-color: white;"
                } else {
                    ""
                };
                let prepare =
                    if input { "field.focus(); field.set-selection-offsets(1, 3);" } else { "" };
                let source = format!(
                    r#"
                    export component TestCase inherits Window {{
                        width: 180px;
                        height: 140px;
                        background: white;
                        in property <length> box-width: 80px;
                        in property <length> box-height: 40px;
                        callback prepare();
                        prepare => {{ {prepare} }}
                        field := {item_type} {{
                            x: 80.2px - {x_fraction} * root.box-width;
                            y: 60.2px - {y_fraction} * root.box-height;
                            width: root.box-width;
                            height: root.box-height;
                            text: "Hello";
                            font-size: 14px;
                            color: black;
                            horizontal-alignment: {horizontal};
                            vertical-alignment: {vertical};
                            {selection}
                        }}
                    }}
                    "#
                );
                let mut compiler = slint_interpreter::Compiler::default();
                compiler.set_style("fluent".into());
                let result =
                    poll_once(compiler.build_from_source(source, Default::default())).unwrap();
                assert!(!result.has_errors(), "{:?}", result.diagnostics().collect::<Vec<_>>());
                let definition = result.components().last().unwrap();
                for scale_factor in [1.0, 1.25, 1.5, 2.0] {
                    let component = definition.create().unwrap();
                    component.window().dispatch_event(
                        i_slint_core::platform::WindowEvent::ScaleFactorChanged { scale_factor },
                    );
                    component.show().unwrap();
                    component.invoke("prepare", &[]).unwrap();
                    let reference = component.window().take_snapshot().unwrap();
                    assert!(reference.as_slice().iter().any(|p| p.r < 128));
                    if input {
                        assert!(reference.as_slice().iter().any(|p| p.b > 200 && p.r < 50));
                    }
                    for delta in [0.25, 0.5, 0.75, 1.0] {
                        component.set_property("box-width", (80.0 + delta).into()).unwrap();
                        component.set_property("box-height", (40.0 + delta).into()).unwrap();
                        let actual = component.window().take_snapshot().unwrap();
                        let max_difference = actual
                            .as_bytes()
                            .iter()
                            .zip(reference.as_bytes())
                            .map(|(a, b)| a.abs_diff(*b))
                            .max()
                            .unwrap();
                        // Selection clips can change coverage by a few color levels as the box resizes.
                        let tolerance = if input { 4 } else { 0 };
                        assert!(
                            max_difference <= tolerance,
                            "{item_type} {horizontal}/{vertical}, scale {scale_factor}, delta {delta}: difference {max_difference}"
                        );
                    }
                    component.hide().unwrap();
                }
            }
        }
    }
}

#[test]
fn shadow_tracks_source_paint() {
    init_skia();
    crate::shadow::assert_shadow_tracks_source_paint();
}

#[test]
fn shadow_spread_preserves_adjusted_corner_radii() {
    init_skia();
    crate::shadow::assert_shadow_spread_preserves_adjusted_corner_radii();
}

/// A target that still holds an earlier frame, as a swapchain image or a scanout buffer may.
struct DirtySurface(Rc<RefCell<i_slint_renderer_skia::skia_safe::Surface>>);

impl i_slint_renderer_skia::Surface for DirtySurface {
    fn new(
        _shared_context: &SkiaSharedContext,
        _window_handle: std::sync::Arc<dyn raw_window_handle::HasWindowHandle + Sync + Send>,
        _display_handle: std::sync::Arc<dyn raw_window_handle::HasDisplayHandle + Sync + Send>,
        _size: i_slint_core::api::PhysicalSize,
        _requested_graphics_api: Option<i_slint_core::graphics::RequestedGraphicsAPI>,
        _transparent: bool,
    ) -> Result<Self, PlatformError> {
        Err("DirtySurface is only created directly".into())
    }

    fn name(&self) -> &'static str {
        "dirty"
    }

    fn render(
        &self,
        _window: &i_slint_core::api::Window,
        _size: i_slint_core::api::PhysicalSize,
        render_callback: &dyn Fn(
            &i_slint_renderer_skia::skia_safe::Canvas,
            Option<&mut i_slint_renderer_skia::skia_safe::gpu::DirectContext>,
            u8,
        ) -> Result<
            Option<i_slint_core::partial_renderer::DirtyRegion>,
            PlatformError,
        >,
        _pre_present_callback: &RefCell<Option<Box<dyn FnMut()>>>,
    ) -> Result<i_slint_core::renderer::DrawOutcome, PlatformError> {
        render_callback(self.0.borrow_mut().canvas(), None, 0)?;
        Ok(i_slint_core::renderer::DrawOutcome::Success)
    }

    fn resize_event(&self, _size: i_slint_core::api::PhysicalSize) -> Result<(), PlatformError> {
        Ok(())
    }

    fn bits_per_pixel(&self) -> Result<u8, PlatformError> {
        Ok(32)
    }
}

const DIRTY_SIZE: i32 = 8;

thread_local! {
    static DIRTY_WINDOW: RefCell<Option<Rc<DirtyWindow>>> = const { RefCell::new(None) };
}

struct DirtyWindow {
    window: i_slint_core::api::Window,
    renderer: SkiaRenderer,
    target: Rc<RefCell<i_slint_renderer_skia::skia_safe::Surface>>,
}

impl WindowAdapter for DirtyWindow {
    fn window(&self) -> &i_slint_core::api::Window {
        &self.window
    }

    fn size(&self) -> PhysicalSize {
        PhysicalSize::new(DIRTY_SIZE as u32, DIRTY_SIZE as u32)
    }

    fn renderer(&self) -> &dyn Renderer {
        &self.renderer
    }
}

struct DirtyTargetBackend;

impl Platform for DirtyTargetBackend {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        use i_slint_renderer_skia::skia_safe;
        let mut surface = skia_safe::surfaces::raster_n32_premul((DIRTY_SIZE, DIRTY_SIZE))
            .ok_or("Error creating a raster surface")?;
        surface.canvas().clear(skia_safe::Color::RED);
        let target = Rc::new(RefCell::new(surface));
        let window = Rc::new_cyclic(|self_weak| DirtyWindow {
            window: i_slint_core::api::Window::new(self_weak.clone() as _),
            renderer: SkiaRenderer::new_with_surface(
                &SkiaSharedContext::default(),
                Box::new(DirtySurface(target.clone())),
            ),
            target,
        });
        DIRTY_WINDOW.with(|dirty_window| *dirty_window.borrow_mut() = Some(window.clone()));
        Ok(window)
    }
}

#[test]
fn transparent_gradient_background_replaces_the_previous_frame() {
    i_slint_core::platform::set_platform(Box::new(DirtyTargetBackend))
        .expect("platform already initialized");

    let source = r#"
        export component TestCase inherits Window {
            background: @linear-gradient(90deg, transparent 0%, #0000ff80 100%);
        }
    "#;
    let result = poll_once(
        slint_interpreter::Compiler::default().build_from_source(source.into(), Default::default()),
    )
    .unwrap();
    assert!(!result.has_errors(), "{:?}", result.diagnostics().collect::<Vec<_>>());
    let component = result.components().last().unwrap().create().unwrap();
    component.show().unwrap();

    let window = DIRTY_WINDOW.with(|dirty_window| dirty_window.borrow().clone()).unwrap();
    assert!(matches!(window.renderer.render(), Ok(i_slint_core::renderer::DrawOutcome::Success)));

    let image = window.target.borrow_mut().image_snapshot();
    let pixels = image.peek_pixels().unwrap();
    let red = (0..DIRTY_SIZE)
        .flat_map(|y| (0..DIRTY_SIZE).map(move |x| (x, y)))
        .filter(|&(x, y)| pixels.get_color((x, y)).r() > 0)
        .count();
    assert_eq!(red, 0, "pixels showing the previous frame");
}

// An SVG magnified after it was drawn is rasterized again, like one magnified from the start (#10903).
#[test]
fn svg_rasterized_again_when_magnified() {
    init_skia();
    let mut compiler = slint_interpreter::Compiler::default();
    compiler.set_style("fluent".into());
    let source = r#"
        export component TestCase inherits Window {
            width: 64px;
            height: 64px;
            background: white;
            in property <image> svg;
            in property <float> scale: 1;
            Image {
                x: 0px;
                y: 0px;
                width: 16px;
                height: 8px;
                source: root.svg;
                transform-scale-x: root.scale;
                transform-scale-y: 2 * root.scale;
                transform-origin: { x: 0px, y: 0px };
            }
        }
    "#;
    let result = poll_once(compiler.build_from_source(source.into(), Default::default())).unwrap();
    assert!(!result.has_errors(), "{:?}", result.diagnostics().collect::<Vec<_>>());
    let definition = result.components().last().unwrap();
    let svg = slint_interpreter::Image::load_from_path(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../logo/slint-logo-small-light.svg"),
    )
    .unwrap();

    let magnified_from_start = definition.create().unwrap();
    magnified_from_start.set_property("svg", slint_interpreter::Value::Image(svg.clone())).unwrap();
    magnified_from_start.set_property("scale", 4.0.into()).unwrap();
    magnified_from_start.show().unwrap();
    let expected = magnified_from_start.window().take_snapshot().unwrap();

    let magnified_later = definition.create().unwrap();
    magnified_later.set_property("svg", slint_interpreter::Value::Image(svg)).unwrap();
    magnified_later.show().unwrap();
    magnified_later.window().take_snapshot().unwrap();
    magnified_later.set_property("scale", 4.0.into()).unwrap();
    let actual = magnified_later.window().take_snapshot().unwrap();

    assert!(actual.as_bytes() == expected.as_bytes(), "the magnified SVG wasn't rasterized again");
}
