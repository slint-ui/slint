// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_core::api::{PhysicalSize as PhysicalWindowSize, Window};
use i_slint_core::graphics::RequestedGraphicsAPI;
use i_slint_core::item_rendering::DirtyRegion;
use i_slint_core::platform::PlatformError;
use std::cell::RefCell;
use std::sync::Arc;
use windows::core::Interface;
use windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL_11_0;
use windows::Win32::Graphics::Dxgi::Common::DXGI_STANDARD_MULTISAMPLE_QUALITY_PATTERN;

use windows::Win32::Foundation::{DXGI_STATUS_OCCLUDED, HANDLE, HWND, S_OK};
use windows::Win32::Graphics::Direct3D12::{
    D3D12CreateDevice, ID3D12CommandQueue, ID3D12Device, ID3D12Fence, ID3D12Resource,
    D3D12_COMMAND_LIST_TYPE_DIRECT, D3D12_COMMAND_QUEUE_DESC, D3D12_FENCE_FLAG_NONE,
    D3D12_RESOURCE_STATE_PRESENT,
};
use windows::Win32::Graphics::Dxgi::{
    Common::{DXGI_FORMAT, DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_SAMPLE_DESC},
    CreateDXGIFactory2, IDXGIFactory4, IDXGISwapChain3, DXGI_ADAPTER_FLAG, DXGI_ADAPTER_FLAG_NONE,
    DXGI_ADAPTER_FLAG_SOFTWARE, DXGI_CREATE_FACTORY_FLAGS, DXGI_PRESENT, DXGI_SWAP_CHAIN_DESC1,
    DXGI_SWAP_CHAIN_FLAG, DXGI_SWAP_EFFECT_FLIP_DISCARD, DXGI_USAGE_RENDER_TARGET_OUTPUT,
};
use windows::Win32::System::Threading::{CreateEventW, WaitForSingleObjectEx, INFINITE};

use crate::SkiaSharedContext;

trait MapToPlatformError<T> {
    fn map_platform_error(self, msg: &str) -> std::result::Result<T, PlatformError>;
}

impl<T> MapToPlatformError<T> for windows::core::Result<T> {
    fn map_platform_error(self, msg: &str) -> std::result::Result<T, PlatformError> {
        match self {
            Ok(r) => Ok(r),
            Err(hr) => Err(format!("{} failed. {:x}", msg, hr.code().0).into()),
        }
    }
}

const DEFAULT_SURFACE_FORMAT: DXGI_FORMAT = DXGI_FORMAT_R8G8B8A8_UNORM;

struct SwapChain {
    command_queue: ID3D12CommandQueue,
    swap_chain: IDXGISwapChain3,
    surfaces: Option<[skia_safe::Surface; 2]>,
    current_buffer_index: usize,
    fence: ID3D12Fence,
    fence_values: [u64; 2],
    fence_event: HANDLE,
    gr_context: skia_safe::gpu::DirectContext,
}

impl SwapChain {
    fn new(
        command_queue: ID3D12CommandQueue,
        device: &ID3D12Device,
        mut gr_context: skia_safe::gpu::DirectContext,
        window_handle: raw_window_handle::WindowHandle<'_>,
        size: PhysicalWindowSize,
        dxgi_factory: &IDXGIFactory4,
    ) -> Result<Self, PlatformError> {
        let swap_chain_desc = DXGI_SWAP_CHAIN_DESC1 {
            Width: size.width,
            Height: size.height,
            Format: DEFAULT_SURFACE_FORMAT,
            BufferCount: 2,
            BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
            SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, ..Default::default() },
            ..Default::default()
        };

        let hwnd = match window_handle.as_raw() {
            raw_window_handle::RawWindowHandle::Win32(raw_window_handle::Win32WindowHandle {
                hwnd,
                ..
            }) => HWND(hwnd.get() as _),
            _ => {
                return Err(format!("Metal surface is only supported with Win32WindowHandle").into())
            }
        };

        let swap_chain1 = unsafe {
            dxgi_factory.CreateSwapChainForHwnd(&command_queue, hwnd, &swap_chain_desc, None, None)
        }
        .map_platform_error("unable to create D3D swap chain")?;

        let swap_chain: IDXGISwapChain3 =
            swap_chain1.cast().map_platform_error("unable to cast swap chain 1 to v3")?;

        let fence = unsafe { device.CreateFence(0, D3D12_FENCE_FLAG_NONE) }
            .map_platform_error("unable to create D3D12 fence")?;

        let fence_values = [0, 0];

        let fence_event = unsafe { CreateEventW(None, false, false, None) }
            .map_platform_error("error creating fence event")?;

        let current_buffer_index = unsafe { swap_chain.GetCurrentBackBufferIndex() } as usize;

        let surfaces = Some(Self::create_surfaces(
            &swap_chain,
            &mut gr_context,
            size.width as _,
            size.height as _,
        )?);

        Ok(Self {
            command_queue,
            swap_chain,
            surfaces,
            current_buffer_index,
            fence,
            fence_event,
            fence_values,
            gr_context,
        })
    }

    fn render_and_present<T>(
        &mut self,
        callback: impl FnOnce(&mut skia_safe::Surface, &mut skia_safe::gpu::DirectContext, u8) -> T,
        pre_present_callback: &RefCell<Option<Box<dyn FnMut()>>>,
    ) -> Result<T, PlatformError> {
        let current_fence_value = self.fence_values[self.current_buffer_index];

        self.current_buffer_index = unsafe { self.swap_chain.GetCurrentBackBufferIndex() } as usize;
        self.wait_for_buffer(self.current_buffer_index)?;

        self.fence_values[self.current_buffer_index] = current_fence_value + 1;

        let surface = &mut (*self.surfaces.as_mut().unwrap())[self.current_buffer_index];

        // TODO: pass correct buffer age
        let result = callback(surface, &mut self.gr_context, 0);

        let info = Default::default();
        self.gr_context.flush_surface_with_access(
            surface,
            skia_safe::surface::BackendSurfaceAccess::Present,
            &info,
        );
        self.gr_context.submit(None);

        if let Some(pre_present_callback) = pre_present_callback.borrow_mut().as_mut() {
            pre_present_callback();
        }

        let present_result = unsafe { self.swap_chain.Present(1, DXGI_PRESENT(0)) };
        if present_result != S_OK && present_result != DXGI_STATUS_OCCLUDED {
            return Err(format!("Error presenting d3d swap chain: {:x}", present_result.0).into());
        }

        unsafe {
            self.command_queue.Signal(&self.fence, self.fence_values[self.current_buffer_index])
        }
        .map_platform_error("error setting up completion signal for d3d12 command queue")?;

        Ok(result)
    }

    fn create_surfaces(
        swap_chain: &IDXGISwapChain3,
        gr_context: &mut skia_safe::gpu::DirectContext,
        width: i32,
        height: i32,
    ) -> Result<[skia_safe::Surface; 2], PlatformError> {
        let mut make_surface = |buffer_index| {
            let buffer: ID3D12Resource = unsafe { swap_chain.GetBuffer(buffer_index) }
                .map_err(|hr| format!("unable to retrieve swap chain back buffer: {hr}"))?;

            debug_assert_eq!(unsafe { buffer.GetDesc().Width }, width as u64);
            debug_assert_eq!(unsafe { buffer.GetDesc().Height }, height as u32);

            let texture_info = skia_safe::gpu::d3d::TextureResourceInfo {
                resource: buffer,
                alloc: None,
                resource_state: D3D12_RESOURCE_STATE_PRESENT,
                format: DEFAULT_SURFACE_FORMAT,
                sample_count: 1,
                level_count: 1,
                sample_quality_pattern: DXGI_STANDARD_MULTISAMPLE_QUALITY_PATTERN,
                protected: skia_safe::gpu::Protected::No,
            };
            let backend_texture =
                skia_safe::gpu::BackendRenderTarget::new_d3d((width, height), &texture_info);

            skia_safe::gpu::surfaces::wrap_backend_render_target(
                gr_context,
                &backend_texture,
                skia_safe::gpu::SurfaceOrigin::TopLeft,
                skia_safe::ColorType::RGBA8888,
                None,
                None,
            )
            .ok_or_else(|| format!("unable to create d3d skia backend render target"))
        };

        Ok([make_surface(0)?, make_surface(1)?])
    }

    fn resize(
        &mut self,
        width: u32,
        height: u32,
    ) -> Result<(), i_slint_core::platform::PlatformError> {
        self.gr_context.flush_submit_and_sync_cpu();

        self.wait_for_buffer(0)?;
        self.wait_for_buffer(1)?;

        drop(self.surfaces.take());

        unsafe {
            self.swap_chain.ResizeBuffers(
                0,
                width,
                height,
                DEFAULT_SURFACE_FORMAT,
                DXGI_SWAP_CHAIN_FLAG(0),
            )
        }
        .map_platform_error("Error resizing swap chain buffers")?;

        self.surfaces = Some(Self::create_surfaces(
            &self.swap_chain,
            &mut self.gr_context,
            width as i32,
            height as i32,
        )?);
        Ok(())
    }

    fn wait_for_buffer(&mut self, buffer_index: usize) -> Result<(), PlatformError> {
        if unsafe { self.fence.GetCompletedValue() } < self.fence_values[buffer_index] {
            unsafe {
                self.fence.SetEventOnCompletion(self.fence_values[buffer_index], self.fence_event)
            }
            .map_platform_error("error setting event on command queue completion")?;

            unsafe {
                WaitForSingleObjectEx(self.fence_event, INFINITE, false);
            }
        }
        Ok(())
    }
}

/// This surface renders into the given window using Direct 3D. The provided display
/// argument is ignored, as it has no meaning on Windows.
pub struct D3DSurface {
    swap_chain: RefCell<SwapChain>,
}

impl super::Surface for D3DSurface {
    fn new(
        _shared_context: &SkiaSharedContext,
        window_handle: Arc<dyn raw_window_handle::HasWindowHandle + Send + Sync>,
        _display_handle: Arc<dyn raw_window_handle::HasDisplayHandle + Send + Sync>,
        size: PhysicalWindowSize,
        requested_graphics_api: Option<RequestedGraphicsAPI>,
    ) -> Result<Self, i_slint_core::platform::PlatformError> {
        if requested_graphics_api
            .map_or(false, |api| !matches!(api, RequestedGraphicsAPI::Direct3D))
        {
            return Err(format!("Requested non-Direct3D rendering with Direct3D renderer").into());
        }

        let factory_flags = 0;
        /*
        let factory_flags = dxgi1_3::DXGI_CREATE_FACTORY_DEBUG;

        {
            let maybe_debug_interface: Result<
                ComPtr<winapi::um::d3d12sdklayers::ID3D12Debug>,
                HRESULT,
            > = resolve_interface(|iid, ptr| unsafe { d3d12::D3D12GetDebugInterface(iid, ptr) });
            if let Ok(debug) = maybe_debug_interface {
                unsafe { debug.EnableDebugLayer() };
            }
        }
        */

        let dxgi_factory: IDXGIFactory4 =
            unsafe { CreateDXGIFactory2(DXGI_CREATE_FACTORY_FLAGS(factory_flags)) }
                .map_platform_error("unable to create DXGIFactory4")?;

        let mut software_adapter_index = None;
        let use_warp = std::env::var("SLINT_D3D_USE_WARP").is_ok();

        let adapter = {
            let mut i = 0;
            loop {
                let adapter = match unsafe { dxgi_factory.EnumAdapters1(i) } {
                    Ok(adapter) => adapter,
                    Err(_) => break None,
                };

                let Ok(desc) = (unsafe { adapter.GetDesc1() }) else {
                    continue;
                };

                let adapter_is_warp = (DXGI_ADAPTER_FLAG(desc.Flags as i32)
                    & DXGI_ADAPTER_FLAG_SOFTWARE)
                    != DXGI_ADAPTER_FLAG_NONE;

                if adapter_is_warp {
                    if software_adapter_index.is_none() {
                        software_adapter_index = Some(i);
                    }

                    if !use_warp {
                        i += 1;
                        // Select warp only if explicitly opted in via SLINT_D3D_USE_WARP
                        continue;
                    }

                    // found warp adapter, requested warp? give it a try below
                } else if use_warp {
                    // Don't select a non-warp adapter when warp is requested
                    i += 1;
                    continue;
                }

                // Check to see whether the adapter supports Direct3D 12, but don't
                // create the actual device yet.
                if unsafe {
                    D3D12CreateDevice(
                        &adapter,
                        D3D_FEATURE_LEVEL_11_0,
                        std::ptr::null_mut::<Option<ID3D12Device>>(),
                    )
                }
                .is_ok()
                {
                    break Some(adapter);
                }

                i += 1;
            }
        };

        let adapter = adapter.map_or_else(
            || {
                let software_adapter_index = software_adapter_index
                    .ok_or_else(|| format!("unable to locate D3D software adapter"))?;
                unsafe { dxgi_factory.EnumAdapters1(software_adapter_index) }
                    .map_err(|hr| format!("unable to create D3D software adapter: {hr}"))
            },
            |adapter| Ok(adapter),
        )?;

        let mut device: Option<ID3D12Device> = None;
        unsafe { D3D12CreateDevice(&adapter, D3D_FEATURE_LEVEL_11_0, &mut device) }
            .map_platform_error("error calling D3D12CreateDevice")?;
        let device = device.unwrap();

        let queue: ID3D12CommandQueue = {
            let desc = D3D12_COMMAND_QUEUE_DESC {
                Type: D3D12_COMMAND_LIST_TYPE_DIRECT,
                ..Default::default()
            };

            unsafe { device.CreateCommandQueue(&desc) }
                .map_platform_error("Creating command queue")?
        };

        let backend_context = skia_safe::gpu::d3d::BackendContext {
            adapter,
            device: device.clone(),
            queue: queue.clone(),
            memory_allocator: None,
            protected_context: skia_safe::gpu::Protected::No,
        };

        let gr_context = unsafe { skia_safe::gpu::DirectContext::new_d3d(&backend_context, None) }
            .ok_or_else(|| format!("unable to create Skia D3D DirectContext"))?;

        let window_handle = window_handle
            .window_handle()
            .map_err(|e| format!("error obtaining window handle for skia d3d renderer: {e}"))?;

        let swap_chain = RefCell::new(SwapChain::new(
            queue,
            &device,
            gr_context,
            window_handle,
            size,
            &dxgi_factory,
        )?);

        Ok(Self { swap_chain })
    }

    fn name(&self) -> &'static str {
        "d3d"
    }

    fn resize_event(
        &self,
        size: PhysicalWindowSize,
    ) -> Result<(), i_slint_core::platform::PlatformError> {
        self.swap_chain.borrow_mut().resize(size.width, size.height)
    }

    fn render(
        &self,
        _window: &Window,
        _size: PhysicalWindowSize,
        callback: &dyn Fn(
            &skia_safe::Canvas,
            Option<&mut skia_safe::gpu::DirectContext>,
            u8,
        ) -> Option<DirtyRegion>,
        pre_present_callback: &RefCell<Option<Box<dyn FnMut()>>>,
    ) -> Result<(), i_slint_core::platform::PlatformError> {
        self.swap_chain.borrow_mut().render_and_present(
            |surface, gr_context, buffer_age| {
                callback(surface.canvas(), Some(gr_context), buffer_age);
            },
            pre_present_callback,
        )
    }

    fn bits_per_pixel(&self) -> Result<u8, i_slint_core::platform::PlatformError> {
        let desc = unsafe { self.swap_chain.borrow().swap_chain.GetDesc() }
            .map_platform_error("error getting swap chain description")?;
        Ok(match desc.BufferDesc.Format {
            DEFAULT_SURFACE_FORMAT => 32,
            fmt @ _ => {
                return Err(
                    format!("Skia D3D Renderer: Unsupported buffer format found {fmt:?}").into()
                )
            }
        })
    }
}
