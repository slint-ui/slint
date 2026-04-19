// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use std::{cell::Cell, rc::Rc, sync::Arc};

use euclid::default::Size2D;
use image::RgbaImage;
use winit::dpi::PhysicalSize;

use servo::{DeviceIntRect, RenderingContext};

use surfman::{
    Connection, Device, Surface, SurfaceTexture, SurfaceType,
    chains::{PreserveBuffer, SwapChain},
};

use super::surfman_context::SurfmanRenderingContext;

pub struct GPURenderingContext {
    pub size: Cell<PhysicalSize<u32>>,
    pub swap_chain: SwapChain<Device>,
    pub surfman_rendering_info: SurfmanRenderingContext,
    #[cfg(target_os = "windows")]
    pub d3d11_state: super::directx::D3D11SharedState,
}

impl Drop for GPURenderingContext {
    fn drop(&mut self) {
        let device = &mut self.surfman_rendering_info.device.borrow_mut();
        let context = &mut self.surfman_rendering_info.context.borrow_mut();
        let _ = self.swap_chain.destroy(device, context);
    }
}

impl GPURenderingContext {
    pub fn new(
        size: PhysicalSize<u32>,
        wgpu_device: &wgpu::Device,
    ) -> Result<Self, surfman::Error> {
        Self::print_wgpu_backend(wgpu_device);

        let connection = Connection::new()?;

        #[cfg(not(target_os = "windows"))]
        let adapter = connection.create_adapter()?;

        #[cfg(target_os = "windows")]
        let adapter = Self::pick_synchronized_adapter(&connection, wgpu_device);

        let surfman_rendering_info = SurfmanRenderingContext::new(&connection, &adapter)?;

        let surfman_size = Size2D::new(size.width as i32, size.height as i32);

        let surface =
            surfman_rendering_info.create_surface(SurfaceType::Generic { size: surfman_size })?;

        surfman_rendering_info.bind_surface(surface)?;

        surfman_rendering_info.make_current()?;

        let swap_chain = surfman_rendering_info.create_attached_swap_chain()?;

        #[cfg(target_os = "windows")]
        let d3d11_state = Self::init_d3d11_shared_state(&surfman_rendering_info.device.borrow())?;

        Ok(Self {
            swap_chain,
            size: Cell::new(size),
            surfman_rendering_info,
            #[cfg(target_os = "windows")]
            d3d11_state,
        })
    }

    #[cfg(target_os = "windows")]
    fn pick_synchronized_adapter(
        connection: &Connection,
        wgpu_device: &wgpu::Device,
    ) -> surfman::Adapter {
        #[derive(Debug, Clone, Copy)]
        enum AdapterMode {
            Hardware,
            LowPower,
            Default,
        }

        if let Some(wgpu_luid) = unsafe {
            use slint::wgpu_28::wgpu::hal::api::Dx12;
            wgpu_device.as_hal::<Dx12>().and_then(|hal| Some(hal.raw_device().GetAdapterLuid()))
        } {
            use windows::{
                Win32::Graphics::{Direct3D11::ID3D11Device, Dxgi},
                core::{IUnknown, Interface},
            };

            // On Windows, Slint and Surfman must use the exact same physical GPU (LUID)
            // to enable zero-copy texture sharing via shared handles.
            // We iterate through Surfman's adapter presets to find the one that matches WGPU's selection.
            for mode in [AdapterMode::Hardware, AdapterMode::LowPower, AdapterMode::Default] {
                let surfman_adapter_result = match mode {
                    AdapterMode::LowPower => connection.create_low_power_adapter(),
                    AdapterMode::Hardware => connection.create_hardware_adapter(),
                    AdapterMode::Default => connection.create_adapter(),
                };

                if let Ok(surfman_adapter) = surfman_adapter_result {
                    // To verify the match, we create a temporary device and extract its D3D11 LUID.
                    if let Ok(temp_device) = connection.create_device(&surfman_adapter) {
                        let d3d11_device_ptr = temp_device.native_device().d3d11_device;
                        let d3d11_device: ID3D11Device = unsafe {
                            IUnknown::from_raw(d3d11_device_ptr as *mut _).cast().unwrap()
                        };

                        let surfman_luid = unsafe {
                            d3d11_device
                                .cast::<Dxgi::IDXGIDevice>()
                                .unwrap()
                                .GetAdapter()
                                .unwrap()
                                .GetDesc()
                                .unwrap()
                                .AdapterLuid
                        };

                        // Compare the Surfman LUID with the WGPU LUID (target).
                        if surfman_luid.HighPart == wgpu_luid.HighPart
                            && surfman_luid.LowPart == wgpu_luid.LowPart
                        {
                            eprintln!("[GPU] Synchronized with WGPU via {:?} mode", mode);
                            return surfman_adapter;
                        }
                    }
                }
            }
            eprintln!("[GPU] WARNING: No exact LUID match found; texture sharing may fail.");
        }

        connection
            .create_hardware_adapter()
            .expect("Failed to create a hardware-accelerated Surfman adapter")
    }

    fn print_wgpu_backend(wgpu_device: &wgpu::Device) {
        let backend = unsafe {
            use slint::wgpu_28::wgpu::hal::api;

            #[cfg(target_os = "windows")]
            {
                use api::{Dx12, Gles, Vulkan};
                if wgpu_device.as_hal::<Dx12>().is_some() {
                    "DirectX 12"
                } else if wgpu_device.as_hal::<Vulkan>().is_some() {
                    "Vulkan"
                } else if wgpu_device.as_hal::<Gles>().is_some() {
                    "OpenGL"
                } else {
                    "Unknown"
                }
            }
            #[cfg(any(target_os = "linux", target_os = "android"))]
            {
                use api::{Gles, Vulkan};
                if wgpu_device.as_hal::<Vulkan>().is_some() {
                    "Vulkan"
                } else if wgpu_device.as_hal::<Gles>().is_some() {
                    "OpenGL"
                } else {
                    "Unknown"
                }
            }
            #[cfg(target_vendor = "apple")]
            {
                use api::Metal;
                if wgpu_device.as_hal::<Metal>().is_some() { "Metal" } else { "Unknown" }
            }
        };
        eprintln!("[GPU] Active WGPU backend: {}", backend);
    }

    /// Imports Metal surface as a WGPU texture for rendering on macOS/iOS.
    /// Unbinds the surface, converts to WGPU texture, then rebinds it.
    #[cfg(target_vendor = "apple")]
    pub fn get_wgpu_texture_from_metal(
        &self,
        wgpu_device: &wgpu::Device,
        wgpu_queue: &wgpu::Queue,
    ) -> Result<wgpu::Texture, surfman::Error> {
        use super::metal::WPGPUTextureFromMetal;

        let device = &self.surfman_rendering_info.device.borrow();
        let mut context = self.surfman_rendering_info.context.borrow_mut();

        let surface = device.unbind_surface_from_context(&mut context)?.unwrap();

        let size = self.size.get();

        let wgpu_texture = WPGPUTextureFromMetal::new(size, wgpu_device).get(
            wgpu_device,
            wgpu_queue,
            device,
            &surface,
        );

        let _ =
            device.bind_surface_to_context(&mut context, surface).map_err(|(err, mut surface)| {
                let _ = device.destroy_surface(&mut context, &mut surface);
                err
            });

        Ok(wgpu_texture)
    }
}

impl RenderingContext for GPURenderingContext {
    fn prepare_for_rendering(&self) {
        self.surfman_rendering_info.prepare_for_rendering();
    }

    fn read_to_image(&self, source_rectangle: DeviceIntRect) -> Option<RgbaImage> {
        self.surfman_rendering_info.read_to_image(source_rectangle)
    }

    fn size(&self) -> PhysicalSize<u32> {
        self.size.get()
    }

    fn resize(&self, size: PhysicalSize<u32>) {
        if self.size.get() == size {
            return;
        }

        self.size.set(size);

        let mut device = self.surfman_rendering_info.device.borrow_mut();
        let mut context = self.surfman_rendering_info.context.borrow_mut();
        let size = Size2D::new(size.width as i32, size.height as i32);
        let _ = self.swap_chain.resize(&mut *device, &mut *context, size);
    }

    fn present(&self) {
        let mut device = self.surfman_rendering_info.device.borrow_mut();
        let mut context = self.surfman_rendering_info.context.borrow_mut();
        let _ = self.swap_chain.swap_buffers(&mut *device, &mut *context, PreserveBuffer::No);
    }

    fn make_current(&self) -> std::result::Result<(), surfman::Error> {
        self.surfman_rendering_info.make_current()
    }

    fn gleam_gl_api(&self) -> Rc<dyn gleam::gl::Gl> {
        self.surfman_rendering_info.gleam_gl.clone()
    }

    fn glow_gl_api(&self) -> Arc<glow::Context> {
        self.surfman_rendering_info.glow_gl.clone()
    }

    fn create_texture(&self, surface: Surface) -> Option<(SurfaceTexture, u32, Size2D<i32>)> {
        self.surfman_rendering_info.create_texture(surface)
    }

    fn destroy_texture(&self, surface_texture: SurfaceTexture) -> Option<Surface> {
        self.surfman_rendering_info.destroy_texture(surface_texture)
    }

    fn connection(&self) -> Option<Connection> {
        self.surfman_rendering_info.connection()
    }
}
