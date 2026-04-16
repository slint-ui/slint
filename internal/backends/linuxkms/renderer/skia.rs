// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::sync::Arc;

use crate::display::RenderingRotation;
use crate::drmoutput::DrmOutput;
use i_slint_core::api::{PhysicalSize as PhysicalWindowSize, Window};
use i_slint_core::item_rendering::ItemRenderer;
use i_slint_core::partial_renderer::DirtyRegion;
use i_slint_core::platform::PlatformError;
use i_slint_renderer_skia::SkiaRendererExt;
use i_slint_renderer_skia::{SkiaRenderer, SkiaSharedContext, skia_safe};

pub struct SkiaRendererAdapter {
    renderer: i_slint_renderer_skia::SkiaRenderer,
    presenter: Arc<dyn crate::display::Presenter>,
    size: PhysicalWindowSize,
    /// Keep the DRM output alive for the Vulkan renderer. The fd passed to
    /// vkAcquireDrmDisplayEXT must remain open for display ownership.
    _drm_output: Option<DrmOutput>,
}

const SKIA_SUPPORTED_DRM_FOURCC_FORMATS: &[drm::buffer::DrmFourcc] = &[
    // Preferred formats
    drm::buffer::DrmFourcc::Xrgb8888,
    drm::buffer::DrmFourcc::Argb8888,
    // drm::buffer::DrmFourcc::Bgra8888,
    // drm::buffer::DrmFourcc::Rgba8888,

    // 16-bit formats
    drm::buffer::DrmFourcc::Rgb565,
    // drm::buffer::DrmFourcc::Bgr565,

    // // 4444 formats
    // drm::buffer::DrmFourcc::Argb4444,
    // drm::buffer::DrmFourcc::Abgr4444,
    // drm::buffer::DrmFourcc::Rgba4444,
    // drm::buffer::DrmFourcc::Bgra4444,

    // // Single channel formats
    // drm::buffer::DrmFourcc::Gray8,
    // drm::buffer::DrmFourcc::C8,
    // drm::buffer::DrmFourcc::R8,
    // drm::buffer::DrmFourcc::R16,

    // // Dual channel formats
    // drm::buffer::DrmFourcc::Gr88,
    // drm::buffer::DrmFourcc::Rg88,
    // drm::buffer::DrmFourcc::Gr1616,
    // drm::buffer::DrmFourcc::Rg1616,

    // // 10-bit formats
    // drm::buffer::DrmFourcc::Xrgb2101010,
    // drm::buffer::DrmFourcc::Argb2101010,
    // drm::buffer::DrmFourcc::Abgr2101010,
    // drm::buffer::DrmFourcc::Rgba1010102,
    // drm::buffer::DrmFourcc::Bgra1010102,
    // drm::buffer::DrmFourcc::Rgbx1010102,
    // drm::buffer::DrmFourcc::Bgrx1010102,
];

impl SkiaRendererAdapter {
    #[cfg(enable_skia_wgpu)]
    pub fn new_wgpu(
        device_opener: &crate::DeviceOpener,
        requested_graphics_api: Option<&i_slint_core::graphics::RequestedGraphicsAPI>,
    ) -> Result<Box<dyn crate::fullscreenwindowadapter::FullscreenRenderer>, PlatformError> {
        let drm_output = DrmOutput::new(device_opener)?;

        #[cfg(skia_wgpu_28)]
        let (surface_target, size) = drm_output.wgpu_28_surface_target()?;
        #[cfg(skia_wgpu_27)]
        let (surface_target, size) = drm_output.wgpu_27_surface_target()?;

        #[cfg(skia_wgpu_28)]
        let skia_wgpu_surface =
            Box::new(i_slint_renderer_skia::wgpu_28_surface::WGPUSurface::new_with_surface(
                surface_target,
                size,
                requested_graphics_api.cloned(),
            )?);
        #[cfg(skia_wgpu_27)]
        let skia_wgpu_surface =
            Box::new(i_slint_renderer_skia::wgpu_27_surface::WGPUSurface::new_with_surface(
                surface_target,
                size,
                requested_graphics_api.cloned(),
            )?);

        let renderer = Box::new(Self {
            renderer: SkiaRenderer::new_with_surface(
                &SkiaSharedContext::default(),
                skia_wgpu_surface,
            ),
            // TODO: For wgpu we don't have a page flip event handling mechanism yet, so drive it with a timer.
            presenter: crate::display::noop_presenter::NoopPresenter::new(),
            size,
            _drm_output: Some(drm_output),
        });

        eprintln!("Using Skia renderer with wgpu");

        Ok(renderer)
    }

    #[cfg(feature = "renderer-skia-opengl")]
    #[allow(clippy::arc_with_non_send_sync)]
    pub fn new_opengl(
        device_opener: &crate::DeviceOpener,
        _requested_graphics_api: Option<&i_slint_core::graphics::RequestedGraphicsAPI>,
    ) -> Result<Box<dyn crate::fullscreenwindowadapter::FullscreenRenderer>, PlatformError> {
        let drm_output = DrmOutput::new(device_opener)?;
        let display = Arc::new(crate::display::gbmdisplay::GbmDisplay::new(drm_output)?);

        let (width, height) = display.drm_output.size();
        let size = i_slint_core::api::PhysicalSize::new(width, height);

        let skia_gl_surface =
            i_slint_renderer_skia::opengl_surface::OpenGLSurface::new_with_config(
                display.clone(),
                display.clone(),
                size,
                None,
                display.config_template_builder(),
                Some(&|config| display.filter_gl_config(config)),
            )?;

        let renderer = Box::new(Self {
            renderer: SkiaRenderer::new_with_surface(
                &SkiaSharedContext::default(),
                Box::new(skia_gl_surface),
            ),
            presenter: display.clone(),
            size,
            _drm_output: None,
        });

        renderer.renderer.set_pre_present_callback(Some(Box::new({
            move || {
                // Make sure the in-flight font-buffer from the previous swap_buffers call has been
                // posted to the screen.
                display.drm_output.wait_for_page_flip();
            }
        })));

        eprintln!("Using Skia OpenGL renderer");

        Ok(renderer)
    }

    pub fn new_software(
        device_opener: &crate::DeviceOpener,
        _requested_graphics_api: Option<&i_slint_core::graphics::RequestedGraphicsAPI>,
    ) -> Result<Box<dyn crate::fullscreenwindowadapter::FullscreenRenderer>, PlatformError> {
        let display =
            crate::display::swdisplay::new(device_opener, SKIA_SUPPORTED_DRM_FOURCC_FORMATS)?;

        let skia_software_surface: i_slint_renderer_skia::software_surface::SoftwareSurface =
            DrmDumbBufferAccess { display: display.clone() }.into();

        let (width, height) = display.size();
        let size = i_slint_core::api::PhysicalSize::new(width, height);

        let renderer = Box::new(Self {
            renderer: SkiaRenderer::new_with_surface(
                &SkiaSharedContext::default(),
                Box::new(skia_software_surface),
            ),
            presenter: display.as_presenter(),
            size,
            _drm_output: None,
        });

        eprintln!("Using Skia Software renderer");

        Ok(renderer)
    }

    pub fn new_try_wgpu_then_opengl_then_software(
        device_opener: &crate::DeviceOpener,
        requested_graphics_api: Option<&i_slint_core::graphics::RequestedGraphicsAPI>,
    ) -> Result<Box<dyn crate::fullscreenwindowadapter::FullscreenRenderer>, PlatformError> {
        #[allow(unused_assignments)]
        let mut result = Err("No skia renderer available".to_string().into());

        #[cfg(enable_skia_wgpu)]
        {
            result = Self::new_wgpu(device_opener, requested_graphics_api);
        }

        #[cfg(feature = "renderer-skia-opengl")]
        if result.is_err() {
            result = Self::new_opengl(device_opener, requested_graphics_api);
        }

        if result.is_err() {
            result = Self::new_software(device_opener, requested_graphics_api);
        }

        result
    }
}

impl crate::fullscreenwindowadapter::FullscreenRenderer for SkiaRendererAdapter {
    fn as_core_renderer(&self) -> &dyn i_slint_core::renderer::Renderer {
        &self.renderer
    }

    fn render_and_present(
        &self,
        rotation: RenderingRotation,
        draw_mouse_cursor_callback: &dyn Fn(&mut dyn ItemRenderer),
    ) -> Result<(), PlatformError> {
        self.renderer.render_transformed_with_post_callback(
            rotation.degrees(),
            rotation.translation_after_rotation(self.size),
            self.size,
            Some(&|item_renderer| {
                draw_mouse_cursor_callback(item_renderer);
            }),
        )?;
        self.presenter.present()?;
        Ok(())
    }
    fn size(&self) -> i_slint_core::api::PhysicalSize {
        self.size
    }
}
struct DrmDumbBufferAccess {
    display: Arc<dyn crate::display::swdisplay::SoftwareBufferDisplay>,
}

impl i_slint_renderer_skia::software_surface::RenderBuffer for DrmDumbBufferAccess {
    fn with_buffer(
        &self,
        _window: &Window,
        size: PhysicalWindowSize,
        render_callback: &mut dyn FnMut(
            std::num::NonZeroU32,
            std::num::NonZeroU32,
            skia_safe::ColorType,
            u8,
            &mut [u8],
        ) -> Result<
            Option<DirtyRegion>,
            i_slint_core::platform::PlatformError,
        >,
    ) -> Result<(), i_slint_core::platform::PlatformError> {
        let Some((width, height)) = size.width.try_into().ok().zip(size.height.try_into().ok())
        else {
            // Nothing to render
            return Ok(());
        };

        self.display.map_back_buffer(&mut |pixels, age, format| {
            render_callback(
                width,
                height,
                match format {
                    drm::buffer::DrmFourcc::Xrgb8888 => skia_safe::ColorType::BGRA8888,

                    // Note: We use AlphaType::Opaque in software_surface. Might need fixing if
                    // we want to support Argb8888 proper.
                    drm::buffer::DrmFourcc::Argb8888 => skia_safe::ColorType::BGRA8888,

                    drm::buffer::DrmFourcc::Rgba8888 => skia_safe::ColorType::RGBA8888,

                    drm::buffer::DrmFourcc::Bgra8888 => skia_safe::ColorType::BGRA8888,

                    drm::buffer::DrmFourcc::Rgb565 => skia_safe::ColorType::RGB565,

                    drm::buffer::DrmFourcc::Bgr565 => skia_safe::ColorType::RGB565,

                    drm::buffer::DrmFourcc::Argb4444 => skia_safe::ColorType::ARGB4444,

                    drm::buffer::DrmFourcc::Abgr4444 => skia_safe::ColorType::ARGB4444,

                    drm::buffer::DrmFourcc::Rgba4444 => skia_safe::ColorType::ARGB4444,

                    drm::buffer::DrmFourcc::Bgra4444 => skia_safe::ColorType::ARGB4444,

                    drm::buffer::DrmFourcc::C8 => skia_safe::ColorType::Gray8,

                    drm::buffer::DrmFourcc::R8 => skia_safe::ColorType::R8UNorm,

                    drm::buffer::DrmFourcc::R16 => skia_safe::ColorType::Unknown,

                    drm::buffer::DrmFourcc::Gr88 => skia_safe::ColorType::R8G8UNorm,

                    drm::buffer::DrmFourcc::Rg88 => skia_safe::ColorType::R8G8UNorm,

                    drm::buffer::DrmFourcc::Gr1616 => skia_safe::ColorType::R16G16UNorm,

                    drm::buffer::DrmFourcc::Rg1616 => skia_safe::ColorType::R16G16UNorm,

                    drm::buffer::DrmFourcc::Xrgb2101010 => skia_safe::ColorType::RGB101010x,

                    drm::buffer::DrmFourcc::Argb2101010 => skia_safe::ColorType::RGBA1010102,

                    drm::buffer::DrmFourcc::Abgr2101010 => skia_safe::ColorType::BGRA1010102,

                    drm::buffer::DrmFourcc::Rgba1010102 => skia_safe::ColorType::RGBA1010102,

                    drm::buffer::DrmFourcc::Bgra1010102 => skia_safe::ColorType::BGRA1010102,

                    drm::buffer::DrmFourcc::Rgbx1010102 => skia_safe::ColorType::RGB101010x,

                    drm::buffer::DrmFourcc::Bgrx1010102 => skia_safe::ColorType::BGR101010x,
                    _ => return Err(format!(
                        "Unsupported frame buffer format {format} used with skia software renderer"
                    )
                    .into()),
                },
                age,
                pixels,
            )?;
            Ok(())
        })
    }
}
