// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_core::api::PhysicalSize;
use i_slint_core::platform::{Platform, PlatformError};
use i_slint_core::renderer::Renderer;
use i_slint_core::window::WindowAdapter;
use i_slint_renderer_skia::{SkiaRenderer, SkiaSharedContext};
use slint_interpreter::ComponentHandle;

use std::cell::Cell;
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
        core::time::Duration::from_millis(i_slint_core::animations::current_tick().0)
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

fn poll_once<F: std::future::Future>(future: F) -> Option<F::Output> {
    let mut ctx = std::task::Context::from_waker(std::task::Waker::noop());
    let future = std::pin::pin!(future);
    match future.poll(&mut ctx) {
        std::task::Poll::Ready(result) => Some(result),
        std::task::Poll::Pending => None,
    }
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
                let prepare = if input {
                    "field.focus(); field.set-selection-offsets(1, 3);"
                } else {
                    ""
                };
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
                let result = poll_once(compiler.build_from_source(source, Default::default())).unwrap();
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
                        let max_difference = actual.as_bytes().iter().zip(reference.as_bytes())
                            .map(|(a, b)| a.abs_diff(*b)).max().unwrap();
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
