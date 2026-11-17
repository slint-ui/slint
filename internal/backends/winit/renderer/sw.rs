// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Delegate the rendering to the [`i_slint_renderer_software::SoftwareRenderer`]

use core::num::NonZeroU32;
use core::ops::DerefMut;
use i_slint_core::graphics::Rgb8Pixel;
use i_slint_core::platform::PlatformError;
use i_slint_core::renderer::DrawOutcome;
pub use i_slint_renderer_software::SoftwareRenderer;
use i_slint_renderer_software::{PremultipliedRgbaColor, RepaintBufferType, TargetPixel};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use winit::event_loop::ActiveEventLoop;

use super::WinitCompatibleRenderer;

pub struct WinitSoftwareRenderer {
    renderer: SoftwareRenderer,
    _context: RefCell<Option<softbuffer::Context<Arc<winit::window::Window>>>>,
    surface: RefCell<
        Option<softbuffer::Surface<Arc<winit::window::Window>, Arc<winit::window::Window>>>,
    >,
    #[cfg(target_os = "windows")]
    swap_chain: RefCell<Option<dxgi::SwapChainPresenter>>,
}

#[repr(transparent)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct SoftBufferPixel(pub u32);

impl From<SoftBufferPixel> for PremultipliedRgbaColor {
    #[inline]
    fn from(pixel: SoftBufferPixel) -> Self {
        let v = pixel.0;
        PremultipliedRgbaColor {
            red: (v >> 16) as u8,
            green: (v >> 8) as u8,
            blue: v as u8,
            alpha: (v >> 24) as u8,
        }
    }
}

impl From<PremultipliedRgbaColor> for SoftBufferPixel {
    #[inline]
    fn from(pixel: PremultipliedRgbaColor) -> Self {
        Self(
            (pixel.alpha as u32) << 24
                | ((pixel.red as u32) << 16)
                | ((pixel.green as u32) << 8)
                | (pixel.blue as u32),
        )
    }
}

impl TargetPixel for SoftBufferPixel {
    fn blend(&mut self, color: PremultipliedRgbaColor) {
        let mut x = PremultipliedRgbaColor::from(*self);
        x.blend(color);
        *self = x.into();
    }

    fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self(0xff000000 | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32))
    }

    fn background() -> Self {
        Self(0)
    }
}

impl WinitSoftwareRenderer {
    pub fn new_suspended(
        shared_backend_data: &Rc<crate::SharedBackendData>,
    ) -> Result<Box<dyn WinitCompatibleRenderer>, PlatformError> {
        if shared_backend_data.requested_graphics_api.is_some() {
            return Err("The software renderer doesn't support a graphics API selection".into());
        }
        Ok(Box::new(Self {
            renderer: SoftwareRenderer::new(),
            _context: RefCell::new(None),
            surface: RefCell::new(None),
            #[cfg(target_os = "windows")]
            swap_chain: RefCell::new(None),
        }))
    }

    fn create_surface(
        &self,
        winit_window: Arc<winit::window::Window>,
    ) -> Result<(), PlatformError> {
        let context = softbuffer::Context::new(winit_window.clone())
            .map_err(|e| format!("Error creating softbuffer context: {e}"))?;

        let surface =
            softbuffer::Surface::new(&context, winit_window).map_err(|softbuffer_error| {
                format!("Error creating softbuffer surface: {softbuffer_error}")
            })?;

        *self._context.borrow_mut() = Some(context);
        *self.surface.borrow_mut() = Some(surface);
        Ok(())
    }

    /// Whether the frame was presented through the swap chain.
    #[cfg(target_os = "windows")]
    fn render_to_swap_chain(
        &self,
        width: NonZeroU32,
        height: NonZeroU32,
    ) -> Result<bool, PlatformError> {
        let mut swap_chain = self.swap_chain.borrow_mut();
        let Some(presenter) = swap_chain.as_mut() else { return Ok(false) };
        let Err(error) = presenter.render(&self.renderer, width, height) else {
            return Ok(true);
        };
        i_slint_core::debug_log!(
            "Slint: Presenting through DXGI failed, drawing with GDI: {error}"
        );
        let winit_window = presenter.window().clone();
        *swap_chain = None;
        drop(swap_chain);
        self.create_surface(winit_window)?;
        self.renderer.set_repaint_buffer_type(RepaintBufferType::NewBuffer);
        Ok(false)
    }
}

impl super::WinitCompatibleRenderer for WinitSoftwareRenderer {
    fn render(&self, window: &i_slint_core::api::Window) -> Result<DrawOutcome, PlatformError> {
        let size = window.size();

        let Some((width, height)) = size.width.try_into().ok().zip(size.height.try_into().ok())
        else {
            // Nothing to render
            return Ok(DrawOutcome::Success);
        };

        #[cfg(target_os = "windows")]
        if self.render_to_swap_chain(width, height)? {
            return Ok(DrawOutcome::Success);
        }

        let mut borrowed_surface = self.surface.borrow_mut();
        let Some(surface) = borrowed_surface.as_mut() else {
            // Nothing to render
            return Ok(DrawOutcome::Success);
        };

        let winit_window = surface.window().clone();

        surface
            .resize(width, height)
            .map_err(|e| format!("Error resizing softbuffer surface: {e}"))?;

        let mut target_buffer = surface
            .buffer_mut()
            .map_err(|e| format!("Error retrieving softbuffer rendering buffer: {e}"))?;

        let age = target_buffer.age();
        self.renderer.set_repaint_buffer_type(match age {
            1 => RepaintBufferType::ReusedBuffer,
            2 => RepaintBufferType::SwappedBuffers,
            _ => RepaintBufferType::NewBuffer,
        });

        let region = if std::env::var_os("SLINT_LINE_BY_LINE").is_none() {
            let buffer: &mut [SoftBufferPixel] =
                bytemuck::cast_slice_mut(target_buffer.deref_mut());
            self.renderer.render(buffer, width.get() as usize)
        } else {
            // SLINT_LINE_BY_LINE is set and this is a debug mode where we also render in a Rgb565Pixel
            struct FrameBuffer<'a> {
                buffer: &'a mut [u32],
                line: Vec<i_slint_renderer_software::Rgb565Pixel>,
            }
            impl i_slint_renderer_software::LineBufferProvider for FrameBuffer<'_> {
                type TargetPixel = i_slint_renderer_software::Rgb565Pixel;
                fn process_line(
                    &mut self,
                    line: usize,
                    range: core::ops::Range<usize>,
                    render_fn: impl FnOnce(&mut [Self::TargetPixel]),
                ) {
                    let line_begin = line * self.line.len();
                    let sub = &mut self.line[..range.len()];
                    render_fn(sub);
                    for (dst, src) in self.buffer[line_begin..][range].iter_mut().zip(sub) {
                        let p = Rgb8Pixel::from(*src);
                        *dst =
                            0xff000000 | ((p.r as u32) << 16) | ((p.g as u32) << 8) | (p.b as u32);
                    }
                }
            }
            self.renderer.render_by_line(FrameBuffer {
                buffer: &mut target_buffer,
                line: vec![Default::default(); width.get() as usize],
            })
        };

        let damage = region
            .iter()
            .filter_map(|(pos, size)| {
                Some(softbuffer::Rect {
                    x: pos.x as u32,
                    y: pos.y as u32,
                    width: NonZeroU32::new(size.width)?,
                    height: NonZeroU32::new(size.height)?,
                })
            })
            .collect::<Vec<_>>();
        if !damage.is_empty() {
            winit_window.pre_present_notify();
            target_buffer
                .present_with_damage(&damage)
                .map_err(|e| format!("Error presenting softbuffer buffer: {e}"))?;
        }
        Ok(DrawOutcome::Success)
    }

    fn as_core_renderer(&self) -> &dyn i_slint_core::renderer::Renderer {
        &self.renderer
    }

    fn occluded(&self, _: bool) {
        // On X11 and Windows, the buffer is completely cleared when the window is hidden
        // and the buffer age doesn't respect that, so clean the partial rendering cache
        self.renderer.set_repaint_buffer_type(RepaintBufferType::NewBuffer);
    }

    fn resume(
        &self,
        active_event_loop: &ActiveEventLoop,
        window_attributes: winit::window::WindowAttributes,
        _window_adapter_weak: std::rc::Weak<crate::winitwindowadapter::WinitWindowAdapter>,
    ) -> Result<Arc<winit::window::Window>, PlatformError> {
        let winit_window =
            active_event_loop.create_window(window_attributes).map_err(|winit_os_error| {
                PlatformError::from(format!(
                    "Error creating native window for software rendering: {winit_os_error}"
                ))
            })?;
        let winit_window = Arc::new(winit_window);

        // Like Firefox's software WebRender, present through Direct3D 11 where it works and
        // fall back to GDI.
        #[cfg(target_os = "windows")]
        match dxgi::SwapChainPresenter::new(winit_window.clone()) {
            Ok(presenter) => {
                *self.swap_chain.borrow_mut() = Some(presenter);
                return Ok(winit_window);
            }
            Err(error) => {
                i_slint_core::debug_log!("Slint: Presenting through DXGI is unavailable: {error}")
            }
        }

        self.create_surface(winit_window.clone())?;

        Ok(winit_window)
    }

    fn suspend(&self) -> Result<(), PlatformError> {
        #[cfg(target_os = "windows")]
        drop(self.swap_chain.borrow_mut().take());
        drop(self.surface.borrow_mut().take());
        drop(self._context.borrow_mut().take());
        Ok(())
    }
}

#[cfg(target_os = "windows")]
mod dxgi {
    //! Presents software rendered frames through a DXGI flip model swap chain.
    //!
    //! GDI blits into the surface that DWM composes from, and nothing marks the end of a frame.
    //! A composition during a blit shows the top of the new frame over the bottom of the old one.
    //! A flip model present hands DWM whole frames instead.
    //! As in Chromium's software output, Slint draws into a CPU readable staging texture
    //! of a WARP device, which needs no graphics driver.
    //! The damage is then copied into the swap chain.

    use core::num::NonZeroU32;
    use i_slint_core::api::{PhysicalPosition, PhysicalSize};
    use i_slint_renderer_software::{RepaintBufferType, SoftwareRenderer};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use std::cell::RefCell;
    use std::rc::{Rc, Weak};
    use std::sync::Arc;
    use windows::Win32::Foundation::{DXGI_STATUS_OCCLUDED, HMODULE, HWND, RECT};
    use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_WARP;
    use windows::Win32::Graphics::Direct3D11::{
        D3D11_BOX, D3D11_CPU_ACCESS_READ, D3D11_CPU_ACCESS_WRITE, D3D11_CREATE_DEVICE_BGRA_SUPPORT,
        D3D11_CREATE_DEVICE_SINGLETHREADED, D3D11_MAP_READ_WRITE, D3D11_MAPPED_SUBRESOURCE,
        D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING, D3D11CreateDevice,
        ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D,
    };
    use windows::Win32::Graphics::Dxgi::Common::{
        DXGI_ALPHA_MODE_IGNORE, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_UNKNOWN, DXGI_SAMPLE_DESC,
    };
    use windows::Win32::Graphics::Dxgi::{
        CreateDXGIFactory1, DXGI_MWA_NO_ALT_ENTER, DXGI_MWA_NO_WINDOW_CHANGES, DXGI_PRESENT,
        DXGI_PRESENT_PARAMETERS, DXGI_SCALING_STRETCH, DXGI_SWAP_CHAIN_DESC1, DXGI_SWAP_CHAIN_FLAG,
        DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL, DXGI_USAGE_RENDER_TARGET_OUTPUT, IDXGIAdapter,
        IDXGIFactory2, IDXGIOutput, IDXGISwapChain1,
    };
    use windows::core::{Error, Result};

    use super::SoftBufferPixel;

    /// Shared by all windows, as in Chromium's software output.
    struct Device {
        device: ID3D11Device,
        context: ID3D11DeviceContext,
        factory: IDXGIFactory2,
    }

    thread_local! {
        static DEVICE: RefCell<Weak<Device>> = const { RefCell::new(Weak::new()) };
    }

    impl Device {
        fn shared() -> Result<Rc<Self>> {
            DEVICE.with_borrow_mut(|shared| {
                if let Some(device) = shared.upgrade()
                    && unsafe { device.device.GetDeviceRemovedReason() }.is_ok()
                {
                    return Ok(device);
                }
                let mut device = None;
                let mut context = None;
                unsafe {
                    D3D11CreateDevice(
                        None::<&IDXGIAdapter>,
                        D3D_DRIVER_TYPE_WARP,
                        HMODULE::default(),
                        D3D11_CREATE_DEVICE_SINGLETHREADED | D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                        None,
                        D3D11_SDK_VERSION,
                        Some(&mut device),
                        None,
                        Some(&mut context),
                    )?;
                }
                let (Some(device), Some(context)) = (device, context) else {
                    return Err(Error::empty());
                };
                let factory = unsafe { CreateDXGIFactory1()? };
                let device = Rc::new(Self { device, context, factory });
                *shared = Rc::downgrade(&device);
                Ok(device)
            })
        }
    }

    pub struct SwapChainPresenter {
        window: Arc<winit::window::Window>,
        device: Rc<Device>,
        swap_chain: IDXGISwapChain1,
        size: (u32, u32),
        staging: Option<ID3D11Texture2D>,
        present_whole_frame: bool,
    }

    impl SwapChainPresenter {
        pub fn new(window: Arc<winit::window::Window>) -> Result<Self> {
            let hwnd = match window.window_handle().map(|handle| handle.as_raw()) {
                Ok(RawWindowHandle::Win32(handle)) => {
                    HWND(handle.hwnd.get() as *mut core::ffi::c_void)
                }
                _ => return Err(Error::empty()),
            };
            let device = Device::shared()?;
            let size = window.inner_size();
            let desc = DXGI_SWAP_CHAIN_DESC1 {
                Width: size.width.max(1),
                Height: size.height.max(1),
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
                BufferCount: 2,
                Scaling: DXGI_SCALING_STRETCH,
                SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
                AlphaMode: DXGI_ALPHA_MODE_IGNORE,
                ..Default::default()
            };
            let swap_chain = unsafe {
                device.factory.CreateSwapChainForHwnd(
                    &device.device,
                    hwnd,
                    &desc,
                    None,
                    None::<&IDXGIOutput>,
                )?
            };
            // As wgpu does, keep DXGI from handling Alt+Enter and the window's messages.
            unsafe {
                device.factory.MakeWindowAssociation(
                    hwnd,
                    DXGI_MWA_NO_WINDOW_CHANGES | DXGI_MWA_NO_ALT_ENTER,
                )?
            };
            Ok(Self {
                window,
                device,
                swap_chain,
                size: (desc.Width, desc.Height),
                staging: None,
                present_whole_frame: true,
            })
        }

        pub fn window(&self) -> &Arc<winit::window::Window> {
            &self.window
        }

        pub fn render(
            &mut self,
            renderer: &SoftwareRenderer,
            width: NonZeroU32,
            height: NonZeroU32,
        ) -> Result<()> {
            let size = (width.get(), height.get());
            if size != self.size {
                self.staging = None;
                unsafe {
                    self.swap_chain.ResizeBuffers(
                        0,
                        size.0,
                        size.1,
                        DXGI_FORMAT_UNKNOWN,
                        DXGI_SWAP_CHAIN_FLAG(0),
                    )?
                };
                self.size = size;
                self.present_whole_frame = true;
            }
            let staging = match &self.staging {
                Some(staging) => {
                    renderer.set_repaint_buffer_type(RepaintBufferType::ReusedBuffer);
                    staging.clone()
                }
                None => {
                    let staging = self.create_staging()?;
                    renderer.set_repaint_buffer_type(RepaintBufferType::NewBuffer);
                    self.staging = Some(staging.clone());
                    staging
                }
            };

            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            unsafe {
                self.device.context.Map(&staging, 0, D3D11_MAP_READ_WRITE, 0, Some(&mut mapped))?
            };
            let stride = mapped.RowPitch as usize / core::mem::size_of::<SoftBufferPixel>();
            let length = stride * size.1 as usize;
            // A 2D texture's depth pitch spans all of its rows.
            if (mapped.DepthPitch as usize) < length * core::mem::size_of::<SoftBufferPixel>()
                || stride < size.0 as usize
            {
                unsafe { self.device.context.Unmap(&staging, 0) };
                return Err(Error::empty());
            }
            // SAFETY: the mapping holds `DepthPitch` bytes, at least `length` pixels, and stays
            // valid until the `Unmap` below.
            let pixels = unsafe {
                core::slice::from_raw_parts_mut(mapped.pData.cast::<SoftBufferPixel>(), length)
            };
            let region = renderer.render(pixels, stride);
            unsafe { self.device.context.Unmap(&staging, 0) };

            let mut dirty = damage(region.iter(), size);
            if dirty.is_empty() && !self.present_whole_frame {
                return Ok(());
            }
            if self.present_whole_frame {
                dirty = vec![RECT { left: 0, top: 0, right: size.0 as i32, bottom: size.1 as i32 }];
            }
            let back_buffer: ID3D11Texture2D = unsafe { self.swap_chain.GetBuffer(0)? };
            for rect in &dirty {
                let source = D3D11_BOX {
                    left: rect.left as u32,
                    top: rect.top as u32,
                    front: 0,
                    right: rect.right as u32,
                    bottom: rect.bottom as u32,
                    back: 1,
                };
                unsafe {
                    self.device.context.CopySubresourceRegion(
                        &back_buffer,
                        0,
                        source.left,
                        source.top,
                        0,
                        &staging,
                        0,
                        Some(&source),
                    )
                };
            }
            drop(back_buffer);

            // With two flip model buffers, DXGI copies the previous frame's dirty rectangles
            // into this buffer, so the parts outside the current ones are up to date.
            // A whole frame is presented without rectangles.
            let parameters = DXGI_PRESENT_PARAMETERS {
                DirtyRectsCount: if self.present_whole_frame { 0 } else { dirty.len() as u32 },
                pDirtyRects: if self.present_whole_frame {
                    core::ptr::null_mut()
                } else {
                    dirty.as_mut_ptr()
                },
                pScrollRect: core::ptr::null_mut(),
                pScrollOffset: core::ptr::null_mut(),
            };
            let result = unsafe { self.swap_chain.Present1(0, DXGI_PRESENT(0), &parameters) };
            result.ok()?;
            // A hidden window's present isn't shown, so the first one after it covers the
            // whole frame.
            self.present_whole_frame = result == DXGI_STATUS_OCCLUDED;
            Ok(())
        }

        fn create_staging(&self) -> Result<ID3D11Texture2D> {
            let desc = D3D11_TEXTURE2D_DESC {
                Width: self.size.0,
                Height: self.size.1,
                MipLevels: 1,
                ArraySize: 1,
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                Usage: D3D11_USAGE_STAGING,
                BindFlags: 0,
                CPUAccessFlags: (D3D11_CPU_ACCESS_READ.0 | D3D11_CPU_ACCESS_WRITE.0) as u32,
                MiscFlags: 0,
            };
            let mut staging = None;
            unsafe { self.device.device.CreateTexture2D(&desc, None, Some(&mut staging))? };
            staging.ok_or_else(Error::empty)
        }
    }

    /// Cut to the swap chain's size, because Direct3D skips a copy that reaches past it.
    fn damage(
        region: impl Iterator<Item = (PhysicalPosition, PhysicalSize)>,
        (width, height): (u32, u32),
    ) -> Vec<RECT> {
        region
            .map(|(origin, size)| RECT {
                left: origin.x.clamp(0, width as i32),
                top: origin.y.clamp(0, height as i32),
                right: (origin.x + size.width as i32).clamp(0, width as i32),
                bottom: (origin.y + size.height as i32).clamp(0, height as i32),
            })
            .filter(|rect| rect.left < rect.right && rect.top < rect.bottom)
            .collect()
    }
}
