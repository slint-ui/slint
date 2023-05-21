// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use i_slint_core::api::PhysicalSize as PhysicalWindowSize;
use i_slint_core::platform::PlatformError;
use std::cell::RefCell;

use raw_window_handle::HasRawWindowHandle;

use winapi::{
    shared::{dxgi, dxgi1_2, dxgi1_3, dxgi1_4, dxgiformat},
    shared::{
        dxgitype,
        guiddef::GUID,
        winerror::{DXGI_STATUS_OCCLUDED, HRESULT, S_OK},
    },
    um::{d3d12, d3dcommon},
    Interface,
};
use wio::com::ComPtr;

fn resolve_interface<T: Interface>(
    f: impl FnOnce(&GUID, &mut *mut std::ffi::c_void) -> HRESULT,
) -> Result<ComPtr<T>, HRESULT> {
    let mut ptr: *mut std::ffi::c_void = std::ptr::null_mut();
    let r = f(&T::uuidof(), &mut ptr);
    if r == S_OK {
        Ok(unsafe { ComPtr::from_raw(ptr as *mut T) })
    } else {
        Err(r)
    }
}

fn resolve_specific<T: Interface>(
    f: impl FnOnce(&mut *mut T) -> HRESULT,
) -> Result<ComPtr<T>, HRESULT> {
    let mut ptr: *mut T = std::ptr::null_mut();
    let r = f(&mut ptr);
    if r == S_OK {
        Ok(unsafe { ComPtr::from_raw(ptr) })
    } else {
        Err(r)
    }
}

trait MapToPlatformError<T> {
    fn map_platform_error(self, msg: &str) -> Result<T, PlatformError>;
}

impl<T> MapToPlatformError<T> for Result<T, HRESULT> {
    fn map_platform_error(self, msg: &str) -> Result<T, PlatformError> {
        match self {
            Ok(r) => Ok(r),
            Err(hr) => Err(format!("{} failed. {:x}", msg, hr).into()),
        }
    }
}

const DEFAULT_SURFACE_FORMAT: dxgiformat::DXGI_FORMAT = dxgiformat::DXGI_FORMAT_R8G8B8A8_UNORM;

struct SwapChain {
    command_queue: ComPtr<d3d12::ID3D12CommandQueue>,
    swap_chain: ComPtr<dxgi1_4::IDXGISwapChain3>,
    surfaces: Option<[skia_safe::Surface; 2]>,
    current_buffer_index: usize,
    fence: ComPtr<d3d12::ID3D12Fence>,
    fence_values: [u64; 2],
    fence_event: *mut std::ffi::c_void,
    gr_context: skia_safe::gpu::DirectContext,
}

impl SwapChain {
    fn new(
        command_queue: ComPtr<d3d12::ID3D12CommandQueue>,
        device: &ComPtr<d3d12::ID3D12Device>,
        mut gr_context: skia_safe::gpu::DirectContext,
        window_handle: raw_window_handle::WindowHandle<'_>,
        size: PhysicalWindowSize,
        dxgi_factory: &ComPtr<dxgi1_4::IDXGIFactory4>,
    ) -> Result<Self, PlatformError> {
        let swap_chain_desc = dxgi1_2::DXGI_SWAP_CHAIN_DESC1 {
            Width: size.width,
            Height: size.height,
            Format: DEFAULT_SURFACE_FORMAT,
            BufferCount: 2,
            BufferUsage: dxgitype::DXGI_USAGE_RENDER_TARGET_OUTPUT,
            SwapEffect: dxgi::DXGI_SWAP_EFFECT_FLIP_DISCARD,
            SampleDesc: dxgitype::DXGI_SAMPLE_DESC { Count: 1, ..Default::default() },
            ..Default::default()
        };

        let hwnd = match window_handle.raw_window_handle() {
            raw_window_handle::RawWindowHandle::Win32(raw_window_handle::Win32WindowHandle {
                hwnd,
                ..
            }) => hwnd,
            _ => {
                return Err(format!("Metal surface is only supported with Win32WindowHandle").into())
            }
        };

        let swap_chain1 = resolve_specific(|ptr| unsafe {
            dxgi_factory.CreateSwapChainForHwnd(
                command_queue.as_raw() as _,
                hwnd as _,
                &swap_chain_desc,
                std::ptr::null(),
                std::ptr::null_mut(),
                ptr,
            )
        })
        .map_platform_error("unable to create D3D swap chain")?;

        let swap_chain: ComPtr<dxgi1_4::IDXGISwapChain3> =
            swap_chain1.cast().map_platform_error("unable to cast swap chain 1 to v3")?;

        let fence = resolve_interface(|iid, ptr| unsafe {
            device.CreateFence(0, d3d12::D3D12_FENCE_FLAG_NONE, iid, ptr)
        })
        .map_platform_error("unable to create D3D12 fence")?;

        let fence_values = [0, 0];

        let fence_event = unsafe {
            winapi::um::synchapi::CreateEventW(std::ptr::null_mut(), 0, 0, std::ptr::null())
        };

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
        callback: impl FnOnce(&mut skia_safe::Surface, &mut skia_safe::gpu::DirectContext) -> T,
    ) -> Result<T, PlatformError> {
        let current_fence_value = self.fence_values[self.current_buffer_index];

        self.current_buffer_index = unsafe { self.swap_chain.GetCurrentBackBufferIndex() } as usize;
        self.wait_for_buffer(self.current_buffer_index)?;

        self.fence_values[self.current_buffer_index] = current_fence_value + 1;

        let surface = &mut (*self.surfaces.as_mut().unwrap())[self.current_buffer_index];

        let result = callback(surface, &mut self.gr_context);

        let info = Default::default();
        surface.flush_with_access_info(skia_safe::surface::BackendSurfaceAccess::Present, &info);

        self.gr_context.submit(None);

        let present_result = unsafe { self.swap_chain.Present(1, 0) };
        if present_result != S_OK && present_result != DXGI_STATUS_OCCLUDED {
            return Err(format!("Error presenting d3d swap chain: {:x}", present_result).into());
        }

        let signal_result = unsafe {
            self.command_queue
                .Signal(self.fence.as_raw() as _, self.fence_values[self.current_buffer_index])
        };
        if signal_result != S_OK {
            return Err(format!(
                "error setting up completion signal for d3d12 command queue: {:x}",
                signal_result
            )
            .into());
        }

        Ok(result)
    }

    fn create_surfaces(
        swap_chain: &ComPtr<dxgi1_4::IDXGISwapChain3>,
        gr_context: &mut skia_safe::gpu::DirectContext,
        width: i32,
        height: i32,
    ) -> Result<[skia_safe::Surface; 2], PlatformError> {
        let mut make_surface = |buffer_index| {
            let buffer: ComPtr<d3d12::ID3D12Resource> = resolve_interface(|iid, ptr| unsafe {
                swap_chain.GetBuffer(buffer_index, iid, ptr)
            })
            .map_err(|hr| format!("unable to retrieve swap chain back buffer: {hr}"))?;

            debug_assert_eq!(unsafe { buffer.GetDesc().Width }, width as u64);
            debug_assert_eq!(unsafe { buffer.GetDesc().Height }, height as u32);

            let texture_info = skia_safe::gpu::d3d::TextureResourceInfo {
                resource: buffer,
                alloc: None,
                resource_state: d3d12::D3D12_RESOURCE_STATE_PRESENT,
                format: DEFAULT_SURFACE_FORMAT,
                sample_count: 1,
                level_count: 1,
                sample_quality_pattern: dxgitype::DXGI_STANDARD_MULTISAMPLE_QUALITY_PATTERN,
                protected: skia_safe::gpu::Protected::No,
            };
            let backend_texture =
                skia_safe::gpu::BackendRenderTarget::new_d3d((width, height), &texture_info);

            skia_safe::Surface::from_backend_render_target(
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
            let resize_result =
                self.swap_chain.ResizeBuffers(0, width, height, DEFAULT_SURFACE_FORMAT, 0);
            if resize_result != S_OK {
                return Err(
                    format!("Error resizing swap chain buffers: {:x}", resize_result).into()
                );
            }
        }

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
            let set_completion_result = unsafe {
                self.fence.SetEventOnCompletion(self.fence_values[buffer_index], self.fence_event)
            };
            if set_completion_result != S_OK {
                return Err(format!(
                    "error setting event on command queue completion: {:x}",
                    set_completion_result
                )
                .into());
            }
            unsafe {
                winapi::um::synchapi::WaitForSingleObjectEx(
                    self.fence_event,
                    winapi::um::winbase::INFINITE,
                    0,
                );
            }
        }
        Ok(())
    }
}

/// This surface renders into the given window using Direct 3D. The provided display
/// arugment is ignored, as it has no meaning on Windows.
pub struct D3DSurface {
    swap_chain: RefCell<SwapChain>,
}

impl super::Surface for D3DSurface {
    fn new(
        window_handle: raw_window_handle::WindowHandle<'_>,
        _display_handle: raw_window_handle::DisplayHandle<'_>,
        size: PhysicalWindowSize,
    ) -> Result<Self, i_slint_core::platform::PlatformError> {
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

        let dxgi_factory: ComPtr<dxgi1_4::IDXGIFactory4> = resolve_interface(|iid, ptr| unsafe {
            dxgi1_3::CreateDXGIFactory2(factory_flags, iid, ptr)
        })
        .map_err(|hr| format!("unable to create DXGIFactory4: {hr}"))?;

        let mut software_adapter_index = None;
        let use_warp = std::env::var("SLINT_D3D_USE_WARP").is_ok();

        let adapter = {
            let mut i = 0;
            loop {
                let adapter =
                    match resolve_specific(|ptr| unsafe { dxgi_factory.EnumAdapters1(i, ptr) }) {
                        Ok(adapter) => adapter,
                        Err(_) => break None,
                    };

                let mut desc = dxgi::DXGI_ADAPTER_DESC1::default();
                unsafe { adapter.GetDesc1(&mut desc) };

                let adapter_is_warp =
                    (desc.Flags & dxgi::DXGI_ADAPTER_FLAG_SOFTWARE) != dxgi::DXGI_ADAPTER_FLAG_NONE;

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
                    d3d12::D3D12CreateDevice(
                        adapter.as_raw() as _,
                        d3dcommon::D3D_FEATURE_LEVEL_11_0,
                        &d3d12::ID3D12Device::uuidof(),
                        std::ptr::null_mut(),
                    )
                } == S_OK
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
                resolve_specific(|ptr| unsafe {
                    dxgi_factory.EnumAdapters1(software_adapter_index, ptr)
                })
                .map_err(|hr| format!("unable to create D3D software adapter: {hr}"))
            },
            |adapter| Ok(adapter),
        )?;

        let device: ComPtr<d3d12::ID3D12Device> = resolve_interface(|iid, ptr| unsafe {
            d3d12::D3D12CreateDevice(
                adapter.as_raw() as _,
                d3dcommon::D3D_FEATURE_LEVEL_11_0,
                iid,
                ptr,
            )
        })
        .map_platform_error("error calling D3D12CreateDevice")?;

        let queue: ComPtr<d3d12::ID3D12CommandQueue> = {
            let desc = d3d12::D3D12_COMMAND_QUEUE_DESC {
                Type: d3d12::D3D12_COMMAND_LIST_TYPE_DIRECT,
                Priority: d3d12::D3D12_COMMAND_QUEUE_PRIORITY_NORMAL as _,
                Flags: d3d12::D3D12_COMMAND_QUEUE_FLAG_NONE,
                NodeMask: 0,
            };

            resolve_interface(|iid, ptr| unsafe { device.CreateCommandQueue(&desc, iid, ptr) })
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
        _size: PhysicalWindowSize,
        callback: &dyn Fn(&mut skia_safe::Canvas, &mut skia_safe::gpu::DirectContext),
    ) -> Result<(), i_slint_core::platform::PlatformError> {
        self.swap_chain
            .borrow_mut()
            .render_and_present(|surface, gr_context| callback(surface.canvas(), gr_context))
    }

    fn bits_per_pixel(&self) -> Result<u8, i_slint_core::platform::PlatformError> {
        let mut desc = dxgi::DXGI_SWAP_CHAIN_DESC::default();
        unsafe { self.swap_chain.borrow().swap_chain.GetDesc(&mut desc) };
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
