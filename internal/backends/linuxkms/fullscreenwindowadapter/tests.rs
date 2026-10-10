// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::*;
use i_slint_core::partial_renderer::DirtyRegion;
use i_slint_renderer_skia::software_surface::{RenderBuffer, SoftwareSurface};
use i_slint_renderer_skia::{SkiaRenderer, SkiaRendererExt, SkiaSharedContext, skia_safe};
use slint::ComponentHandle;
use std::num::NonZeroU32;

const SIZE: PhysicalWindowSize = PhysicalWindowSize::new(320, 240);

struct MemoryBuffer {
    pixels: Rc<RefCell<Vec<u8>>>,
    initialized: Cell<bool>,
}

impl RenderBuffer for MemoryBuffer {
    fn with_buffer(
        &self,
        _: &i_slint_core::api::Window,
        _: PhysicalWindowSize,
        render: &mut dyn FnMut(
            NonZeroU32,
            NonZeroU32,
            skia_safe::ColorType,
            u8,
            &mut [u8],
        ) -> Result<Option<DirtyRegion>, PlatformError>,
    ) -> Result<(), PlatformError> {
        render(
            NonZeroU32::new(SIZE.width).unwrap(),
            NonZeroU32::new(SIZE.height).unwrap(),
            skia_safe::ColorType::RGBA8888,
            u8::from(self.initialized.replace(true)),
            &mut self.pixels.borrow_mut(),
        )?;
        Ok(())
    }
}

struct MemoryRenderer(SkiaRenderer);

impl FullscreenRenderer for MemoryRenderer {
    fn as_core_renderer(&self) -> &dyn i_slint_core::renderer::Renderer {
        &self.0
    }
    fn size(&self) -> PhysicalWindowSize {
        SIZE
    }
    fn render_and_present(
        &self,
        rotation: RenderingRotation,
        cursor: &dyn Fn(&mut dyn ItemRenderer),
    ) -> Result<DrawOutcome, PlatformError> {
        self.0.render_transformed_with_post_callback(
            rotation.degrees(),
            rotation.translation_after_rotation(SIZE),
            SIZE,
            Some(cursor),
        )
    }
}

struct MemoryPlatform {
    adapter: Rc<FullscreenWindowAdapter>,
}

impl slint::platform::Platform for MemoryPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        Ok(self.adapter.clone())
    }
}

slint::slint! {
    export component CursorScene inherits Window {
        width: 320px;
        height: 240px;
        background: #304050;
        Rectangle { x: 30px; y: 25px; width: 100px; height: 60px; background: #c06030; }
    }
}

#[test]
fn incremental_cursor_frames_match_full_repaints() {
    cursor_frames_match_full_repaints(0);
}

/// A DRM dumb buffer's rows are padded to the driver's pitch, so the cursor damage has to land
/// in the right place with a stride that is wider than the screen as well.
#[test]
fn incremental_cursor_frames_match_full_repaints_with_padded_rows() {
    cursor_frames_match_full_repaints(40);
}

fn cursor_frames_match_full_repaints(extra_row_bytes: usize) {
    let row_bytes = SIZE.width as usize * 4 + extra_row_bytes;
    let pixels = Rc::new(RefCell::new(vec![0; row_bytes * SIZE.height as usize]));
    let surface: SoftwareSurface =
        MemoryBuffer { pixels: pixels.clone(), initialized: Cell::new(false) }.into();
    let renderer = SkiaRenderer::new_with_surface(&SkiaSharedContext::default(), Box::new(surface));
    let adapter = FullscreenWindowAdapter::new(
        Box::new(MemoryRenderer(renderer)),
        RenderingRotation::NoRotation,
    )
    .unwrap();
    slint::platform::set_platform(Box::new(MemoryPlatform { adapter: adapter.clone() })).unwrap();
    let scene = CursorScene::new().unwrap();
    scene.show().unwrap();
    let position = Box::pin(Property::new(None));

    for scale in [1.0, 1.5, 2.0] {
        scene.window().dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: scale });
        // Each scale represents a fresh display configuration.
        adapter
            .renderer
            .as_core_renderer()
            .mark_dirty_region(LogicalRect::from_size(euclid::size2(320., 240.)).into());
        adapter.request_redraw();
        adapter.clone().render_if_needed(position.as_ref()).unwrap();
        for (x, y) in
            [(10., 10.), (13., 12.), (100., 80.), (150.5, 90.5), (319. / scale, 239. / scale)]
        {
            position.as_ref().set(Some(LogicalPosition::new(x, y)));
            // Pointer changes must request a frame even over an unchanged background.
            assert!(
                adapter.redraw_requested.get(),
                "pointer movement did not invalidate the frame"
            );
            adapter.clone().render_if_needed(position.as_ref()).unwrap();
            let incremental = pixels.borrow().clone();
            adapter.renderer.as_core_renderer().mark_dirty_region(
                LogicalRect::from_size(euclid::size2(
                    SIZE.width as f32 / scale,
                    SIZE.height as f32 / scale,
                ))
                .into(),
            );
            adapter.request_redraw();
            adapter.clone().render_if_needed(position.as_ref()).unwrap();
            assert!(incremental == *pixels.borrow(), "cursor damage at ({x}, {y}), scale {scale}");
        }
        adapter.set_mouse_cursor(MouseCursorInner::BuiltIn(BuiltInMouseCursor::None));
        adapter.clone().render_if_needed(position.as_ref()).unwrap();
        let hidden = pixels.borrow().clone();
        adapter
            .renderer
            .as_core_renderer()
            .mark_dirty_region(LogicalRect::from_size(euclid::size2(320., 240.)).into());
        adapter.request_redraw();
        adapter.clone().render_if_needed(position.as_ref()).unwrap();
        assert!(hidden == *pixels.borrow(), "hiding the cursor left pixels behind");
        adapter.set_mouse_cursor(MouseCursorInner::default());
    }

    // Nothing may write into the padding, which is what tells apart honouring the stride from
    // treating the buffer as one contiguous run of rows.
    let pixels = pixels.borrow();
    for line in 0..SIZE.height as usize {
        let row = &pixels[line * row_bytes..][..row_bytes];
        assert!(
            row[SIZE.width as usize * 4..].iter().all(|byte| *byte == 0),
            "line {line} was drawn over its padding"
        );
    }
}

/// What a [`FlushRecorder`] saw, and how it draws.
#[derive(Default)]
struct FlushLog {
    flushes: Cell<u32>,
    frames: Cell<u32>,
    /// Flushes that put a pending frame on the screen.
    shown: Cell<u32>,
    pending: Cell<bool>,
    skip: Cell<bool>,
    /// Fails every frame after this many.
    fail_after: Cell<Option<u32>>,
}

/// Draws like [`MemoryRenderer`], and records the flushes the adapter asks for.
struct FlushRecorder {
    renderer: MemoryRenderer,
    log: Rc<FlushLog>,
}

impl FlushRecorder {
    fn new(log: Rc<FlushLog>) -> Self {
        let pixels = Rc::new(RefCell::new(vec![0; SIZE.width as usize * 4 * SIZE.height as usize]));
        let surface: SoftwareSurface =
            MemoryBuffer { pixels, initialized: Cell::new(false) }.into();
        let renderer =
            SkiaRenderer::new_with_surface(&SkiaSharedContext::default(), Box::new(surface));
        Self { renderer: MemoryRenderer(renderer), log }
    }
}

impl FullscreenRenderer for FlushRecorder {
    fn as_core_renderer(&self) -> &dyn i_slint_core::renderer::Renderer {
        self.renderer.as_core_renderer()
    }
    fn size(&self) -> PhysicalWindowSize {
        self.renderer.size()
    }
    fn render_and_present(
        &self,
        rotation: RenderingRotation,
        cursor: &dyn Fn(&mut dyn ItemRenderer),
    ) -> Result<DrawOutcome, PlatformError> {
        if self.log.skip.get() {
            return Ok(DrawOutcome::Skipped);
        }
        if self.log.fail_after.get().is_some_and(|frames| self.log.frames.get() >= frames) {
            return Err("drawing failed".into());
        }
        let outcome = self.renderer.render_and_present(rotation, cursor)?;
        self.log.frames.set(self.log.frames.get() + 1);
        self.log.pending.set(true);
        Ok(outcome)
    }
    fn flush_pending_frame(&self) -> Result<(), PlatformError> {
        self.log.flushes.set(self.log.flushes.get() + 1);
        if self.log.pending.replace(false) {
            self.log.shown.set(self.log.shown.get() + 1);
        }
        Ok(())
    }
}

/// A platform whose clock only moves when the test moves it.
struct ClockPlatform {
    adapter: Rc<FullscreenWindowAdapter>,
    time: Rc<Cell<std::time::Duration>>,
}

impl slint::platform::Platform for ClockPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        Ok(self.adapter.clone())
    }
    fn duration_since_start(&self) -> std::time::Duration {
        self.time.get()
    }
}

slint::slint! {
    export component AnimatedScene inherits Window {
        width: 320px;
        height: 240px;
        in property <length> offset;
        Rectangle {
            x: root.offset;
            width: 50px;
            height: 50px;
            background: #c06030;
            animate x { duration: 100ms; }
        }
    }
}

#[test]
fn pending_frames_are_flushed_once_no_frame_follows() {
    let log = Rc::new(FlushLog::default());
    let adapter = FullscreenWindowAdapter::new(
        Box::new(FlushRecorder::new(log.clone())),
        RenderingRotation::NoRotation,
    )
    .unwrap();
    let time = Rc::new(Cell::new(std::time::Duration::ZERO));
    slint::platform::set_platform(Box::new(ClockPlatform {
        adapter: adapter.clone(),
        time: time.clone(),
    }))
    .unwrap();
    let scene = AnimatedScene::new().unwrap();
    scene.show().unwrap();
    let position = Box::pin(Property::new(None));
    // One iteration of the event loop.
    let render = || {
        slint::platform::update_timers_and_animations();
        adapter.request_redraw();
        adapter.clone().render_if_needed(position.as_ref()).unwrap();
        adapter.flush_unless_frame_follows().unwrap();
    };

    log.skip.set(true);
    render();
    assert_eq!(log.flushes.get(), 0, "a frame that leaves a redraw pending is not flushed");

    log.skip.set(false);
    render();
    assert_eq!(log.flushes.get(), 1, "a still frame is flushed");

    scene.set_offset(200.);
    render();
    assert_eq!(log.flushes.get(), 1, "a frame starting an animation is not flushed");

    time.set(std::time::Duration::from_millis(50));
    render();
    assert_eq!(log.flushes.get(), 1, "a frame in the middle of an animation is not flushed");

    time.set(std::time::Duration::from_millis(500));
    render();
    assert_eq!(log.flushes.get(), 2, "the frame after the animation is flushed");

    let _timer = slint::Timer::default();
    _timer.start(slint::TimerMode::Repeated, std::time::Duration::ZERO, || {});
    render();
    assert_eq!(log.flushes.get(), 3, "a frame is flushed while a timer is always due");
}

/// The backend opens no devices with libseat and libinput off.
#[cfg(not(any(feature = "libseat", feature = "libinput")))]
mod event_loop {
    use super::*;

    slint::slint! {
        export component EndlessAnimation inherits Window {
            width: 320px;
            height: 240px;
            in property <length> offset;
            Rectangle {
                x: root.offset;
                width: 50px;
                height: 50px;
                background: #c06030;
                animate x { duration: 3600s; }
            }
        }
    }

    thread_local! {
        static EVENT_LOOP_LOG: Rc<FlushLog> = Rc::default();
    }

    /// Sets up the linuxkms backend to draw with a [`FlushRecorder`] logging into the returned log.
    fn event_loop_backend() -> Rc<FlushLog> {
        let backend =
            crate::Backend::build(crate::BackendBuilder::default()).unwrap().with_renderer_factory(
                |_, _| Ok(Box::new(FlushRecorder::new(EVENT_LOOP_LOG.with(Rc::clone)))),
            );
        slint::platform::set_platform(Box::new(backend)).unwrap();
        EVENT_LOOP_LOG.with(Rc::clone)
    }

    /// One test only, since a process sets up one event loop proxy, see `set_platform`.
    #[test]
    fn event_loop_flushes_still_frames_and_when_it_ends() {
        let log = event_loop_backend();
        // The still frame and two animated ones, then an error ends the loop.
        log.fail_after.set(Some(3));
        let scene = EndlessAnimation::new().unwrap();
        scene.show().unwrap();
        let weak_scene = scene.as_weak();
        slint::Timer::single_shot(std::time::Duration::from_millis(100), move || {
            weak_scene.unwrap().set_offset(200.);
        });

        assert!(slint::run_event_loop().is_err());

        assert_eq!(log.frames.get(), 3);
        assert_eq!(log.shown.get(), 2, "only the still frame and the last frame were flushed");
    }
}
