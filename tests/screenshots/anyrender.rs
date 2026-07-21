// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Snapshot tests that capture the command stream emitted by
//! [`AnyrenderItemRenderer`](i_slint_renderer_anyrender::AnyrenderItemRenderer)
//! for each `cases/*.slint` file and compare against a JSON golden in
//! `references/anyrender/`.
//!
//! Set `SLINT_UPDATE_TESTS=1` in the environment to overwrite goldens
//! with the current output instead of asserting.

use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use anyrender_serialize::{SceneArchive, SerializeConfig};
use i_slint_core::api::PhysicalSize;
use i_slint_core::platform::{Platform, PlatformError};
use i_slint_core::renderer::Renderer;
use i_slint_core::window::WindowAdapter;
use i_slint_renderer_anyrender::{AnyrenderSlintRenderer, RecordingWindowRenderer};
use slint_interpreter::ComponentHandle;

type RecordingRenderer = AnyrenderSlintRenderer<RecordingWindowRenderer>;

/// The recording renderer is created by the test and injected here, so the
/// test keeps a handle to the concrete type after core takes ownership of
/// the window adapter behind `Rc<dyn WindowAdapter>`. Every window this
/// platform creates shares it (the tests are single-window).
pub struct AnyrenderScreenshotBackend {
    renderer: Rc<RecordingRenderer>,
}

impl Platform for AnyrenderScreenshotBackend {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        Ok(Rc::new_cyclic(|self_weak| AnyrenderScreenshotWindow {
            window: i_slint_core::api::Window::new(self_weak.clone() as _),
            size: Default::default(),
            renderer: self.renderer.clone(),
        }))
    }

    fn duration_since_start(&self) -> core::time::Duration {
        core::time::Duration::from_millis(i_slint_core::animations::current_tick().0)
    }
}

pub struct AnyrenderScreenshotWindow {
    window: i_slint_core::api::Window,
    size: Cell<PhysicalSize>,
    renderer: Rc<RecordingRenderer>,
}

impl WindowAdapter for AnyrenderScreenshotWindow {
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
        &*self.renderer
    }

    fn update_window_properties(&self, properties: i_slint_core::window::WindowProperties<'_>) {
        if self.size.get().width == 0 {
            let c = properties.layout_constraints();
            self.size.set(c.preferred.to_physical(self.window.scale_factor()));
        }
    }
}

/// Set up the recording platform and return the renderer that all windows
/// created by it will share.
pub fn init_anyrender() -> Rc<RecordingRenderer> {
    crate::testing::force_reference_os();

    let renderer = Rc::new(AnyrenderSlintRenderer::new_recording());
    i_slint_core::platform::set_platform(Box::new(AnyrenderScreenshotBackend {
        renderer: renderer.clone(),
    }))
    .expect("platform already initialized");

    // Replace the fontique collection with the bundled test fonts so the
    // recorded glyph runs are deterministic across machines.
    i_slint_backend_testing::configure_test_fonts();

    renderer
}

pub struct TestCase {
    pub absolute_path: PathBuf,
    pub relative_path: PathBuf,
    pub reference_path: PathBuf,
}

pub fn run_test(testcase: TestCase) -> Result<(), Box<dyn std::error::Error>> {
    let renderer = init_anyrender();

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

    let scene = renderer.record()?;
    let size = component.window().size();
    let archive = SceneArchive::from_scene(&scene, &SerializeConfig::new()).map_err(
        |e| -> Box<dyn std::error::Error> { format!("scene serialization failed: {e}").into() },
    )?;

    compare_command_stream(&testcase, &archive, size)?;
    Ok(())
}

/// Compare the captured command list to a JSON golden. With
/// `SLINT_UPDATE_TESTS=1` set, overwrite the golden instead of asserting.
///
/// On mismatch, render both the golden (using the current archive's
/// resources, which are deterministic for a given case) and the current
/// commands through anyrender_vello_cpu, write them as PNGs in /tmp, and
/// include the paths in the error message for human visual diff.
fn compare_command_stream(
    testcase: &TestCase,
    archive: &SceneArchive,
    size: PhysicalSize,
) -> Result<(), Box<dyn std::error::Error>> {
    let reference_path = &testcase.reference_path;
    let mut commands_json = serde_json::to_value(&archive.commands)?;
    quantize_floats(&mut commands_json);
    let serialized = serde_json::to_string_pretty(&commands_json)?;
    let update = std::env::var_os("SLINT_UPDATE_TESTS").is_some();

    if update || !reference_path.exists() {
        if let Some(parent) = reference_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(reference_path, serialized.as_bytes())?;
        if !update {
            return Err(format!(
                "missing reference {} — wrote it; rerun without SLINT_UPDATE_TESTS to verify",
                reference_path.display()
            )
            .into());
        }
        return Ok(());
    }

    // .gitattributes pins the goldens to LF, but normalize anyway in case a
    // clone converted them to CRLF.
    let golden_text = std::fs::read_to_string(reference_path)?.replace("\r\n", "\n");
    if golden_text == serialized {
        return Ok(());
    }

    // Mismatch — render expected/actual PNGs to help the human diff.
    let stem = testcase.relative_path.with_extension("");
    let stem_str = stem.to_string_lossy().replace(['/', '\\'], "-");
    let actual_png = PathBuf::from(format!("/tmp/anyrender-{stem_str}-actual.png"));
    let expected_png = PathBuf::from(format!("/tmp/anyrender-{stem_str}-expected.png"));

    let mut hint = String::new();

    let actual_scene = archive.to_scene().map_err(|e| -> Box<dyn std::error::Error> {
        format!("could not reconstitute current scene: {e}").into()
    })?;
    if let Err(e) = render_scene_to_png(&actual_scene, size, &actual_png) {
        hint.push_str(&format!("\n(failed to render actual PNG: {e})"));
    } else {
        hint.push_str(&format!("\n  actual:   {}", actual_png.display()));
    }

    match serde_json::from_str::<Vec<anyrender_serialize::SerializableRenderCommand>>(&golden_text)
    {
        Ok(golden_commands) => {
            // Reuse the current archive's fonts/images — they are deterministic
            // for a given .slint case, so resource IDs in the golden resolve.
            let golden_archive = SceneArchive {
                manifest: archive.manifest.clone(),
                commands: golden_commands,
                fonts: archive.fonts.clone(),
                images: archive.images.clone(),
            };
            match golden_archive.to_scene() {
                Ok(expected_scene) => {
                    if let Err(e) = render_scene_to_png(&expected_scene, size, &expected_png) {
                        hint.push_str(&format!("\n(failed to render expected PNG: {e})"));
                    } else {
                        hint.push_str(&format!("\n  expected: {}", expected_png.display()));
                    }
                }
                Err(e) => {
                    hint.push_str(&format!(
                        "\n(could not reconstruct golden scene from current resources: {e})"
                    ));
                }
            }
        }
        Err(e) => {
            hint.push_str(&format!("\n(could not parse golden JSON: {e})"));
        }
    }

    Err(format!(
        "command stream mismatch for {}:\n{}\nrendered for visual diff:{}\n\
         (set SLINT_UPDATE_TESTS=1 to overwrite the golden)",
        reference_path.display(),
        line_diff(&golden_text, &serialized),
        hint,
    )
    .into())
}

fn render_scene_to_png(
    scene: &anyrender::recording::Scene,
    size: PhysicalSize,
    out_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    use anyrender::ImageRenderer;
    use anyrender_vello_cpu::VelloCpuImageRenderer;

    let width = size.width.max(1);
    let height = size.height.max(1);

    let mut image_renderer = VelloCpuImageRenderer::new(width, height);
    let mut buf: Vec<u8> = Vec::with_capacity((width * height * 4) as usize);
    let scene_clone = scene.clone();
    image_renderer.render_to_vec(
        |painter| {
            <_ as anyrender::PaintScene>::append_scene(
                painter,
                scene_clone,
                kurbo::Affine::IDENTITY,
            );
        },
        &mut buf,
    );

    unpremultiply_rgba(&mut buf);

    let img = image::RgbaImage::from_raw(width, height, buf)
        .ok_or("vello_cpu image buffer size mismatch")?;
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    img.save(out_path)?;
    Ok(())
}

/// Premultiplied RGBA → straight alpha, e.g. for saving as PNG.
fn unpremultiply_rgba(buf: &mut [u8]) {
    for px in buf.chunks_exact_mut(4) {
        let a = px[3] as u32;
        if a > 0 && a < 255 {
            px[0] = ((px[0] as u32 * 255 / a).min(255)) as u8;
            px[1] = ((px[1] as u32 * 255 / a).min(255)) as u8;
            px[2] = ((px[2] as u32 * 255 / a).min(255)) as u8;
        }
    }
}

/// Quantize every floating point value in the serialized command stream to
/// four decimal places: JSON numbers directly, and the coordinates inside
/// the SVG path strings ("shape"/"clip" entries) by round-tripping them
/// through kurbo::BezPath. kurbo's rounded-rectangle arc conversion is not
/// ULP-stable across architectures (x86-64 and aarch64 differ in the last
/// digits of the bezier control points), so full-precision goldens are not
/// portable. 1e-4 device pixels is orders of magnitude above that noise and
/// far below anything visually meaningful.
///
/// Applied before writing goldens and before comparing against them, so
/// goldens regenerated on any platform are identical.
fn quantize_floats(value: &mut serde_json::Value) {
    fn q(value: f64) -> f64 {
        let rounded = (value * 1e4).round() / 1e4;
        // Normalize -0: tiny values can round to zero from either side.
        if rounded == 0. { 0. } else { rounded }
    }

    match value {
        serde_json::Value::Number(number) => {
            // Integer numbers (resource ids, glyph ids, ...) are left alone.
            if number.is_f64() {
                *number = serde_json::Number::from_f64(q(number.as_f64().unwrap()))
                    .expect("quantized float is finite");
            }
        }
        serde_json::Value::Array(values) => values.iter_mut().for_each(quantize_floats),
        serde_json::Value::Object(map) => {
            for (key, entry) in map.iter_mut() {
                if let ("shape" | "clip", serde_json::Value::String(svg_path)) =
                    (key.as_str(), &mut *entry)
                {
                    // Shapes and clips are SVG path strings with the bezier
                    // coordinates - where the cross-platform instability
                    // actually lives - embedded in the string.
                    let q_point = |p: kurbo::Point| kurbo::Point::new(q(p.x), q(p.y));
                    let path = kurbo::BezPath::from_svg(svg_path)
                        .expect("recorded shapes must be valid SVG paths");
                    let quantized: kurbo::BezPath = path
                        .elements()
                        .iter()
                        .map(|element| match *element {
                            kurbo::PathEl::MoveTo(p) => kurbo::PathEl::MoveTo(q_point(p)),
                            kurbo::PathEl::LineTo(p) => kurbo::PathEl::LineTo(q_point(p)),
                            kurbo::PathEl::QuadTo(p1, p2) => {
                                kurbo::PathEl::QuadTo(q_point(p1), q_point(p2))
                            }
                            kurbo::PathEl::CurveTo(p1, p2, p3) => {
                                kurbo::PathEl::CurveTo(q_point(p1), q_point(p2), q_point(p3))
                            }
                            kurbo::PathEl::ClosePath => kurbo::PathEl::ClosePath,
                        })
                        .collect();
                    *svg_path = quantized.to_svg();
                } else {
                    quantize_floats(entry);
                }
            }
        }
        serde_json::Value::Null | serde_json::Value::Bool(_) | serde_json::Value::String(_) => {}
    }
}

fn line_diff(expected: &str, actual: &str) -> String {
    let mut out = String::new();
    let mut diffs = 0;
    for (i, (e, a)) in expected.lines().zip(actual.lines()).enumerate() {
        if e != a {
            out.push_str(&format!("L{i:>5} - {e}\nL{i:>5} + {a}\n"));
            diffs += 1;
            if diffs >= 20 {
                out.push_str("... (truncated)\n");
                break;
            }
        }
    }
    let expected_lines = expected.lines().count();
    let actual_lines = actual.lines().count();
    if expected_lines != actual_lines {
        out.push_str(&format!("(line count: expected {expected_lines}, actual {actual_lines})\n"));
    }
    if out.is_empty() {
        out.push_str("(no line-level diff but content differs)\n");
    }
    out
}

fn poll_once<F: std::future::Future>(future: F) -> Option<F::Output> {
    let mut ctx = std::task::Context::from_waker(std::task::Waker::noop());
    let future = std::pin::pin!(future);
    match future.poll(&mut ctx) {
        std::task::Poll::Ready(result) => Some(result),
        std::task::Poll::Pending => None,
    }
}
