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
    let pixels = Rc::new(RefCell::new(vec![0; (SIZE.width * SIZE.height * 4) as usize]));
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
}
