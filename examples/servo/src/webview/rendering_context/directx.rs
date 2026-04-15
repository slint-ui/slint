// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use euclid::default::Size2D;
use glow::HasContext;
use std::cell::RefCell;
use winit::dpi::PhysicalSize;

use slint::wgpu_28::wgpu::{
    Device, Extent3d, Texture, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
    wgc::api::Dx12,
};

use windows::{
    Win32::{
        Foundation,
        Graphics::{
            Direct3D11::{self, ID3D11Device, ID3D11Texture2D},
            Direct3D12,
            Dxgi::{self, Common},
        },
    },
    core::{self, Interface},
};

struct D3D11SizeDependentState {
    d3d11_shared_texture: ID3D11Texture2D,
    wgpu_texture: Texture,
}

pub struct D3D11SharedState {
    d3d11_device: ID3D11Device,
    /// Recreated whenever the surface size changes; wrapped in a `RefCell` because
    /// `get_wgpu_texture_from_directx` holds a shared `&self` borrow while needing
    /// to swap this out on resize.
    size_dependent: RefCell<Option<D3D11SizeDependentState>>,
}

#[derive(thiserror::Error, Debug)]
pub enum DirectXTextureError {
    #[error("{0:?}")]
    Surfman(surfman::Error),
    #[error("No surface returned when the surface was unbound from the context")]
    NoSurface,
    #[error("Wgpu is not using the dx12 backend")]
    WgpuNotDx12,
    #[error("d3d11_share_handle() returned null — surface not D3D11-backed")]
    NullShareHandle,
    #[error("{0}")]
    OpenGL(String),
    #[error("{0}")]
    Windows(#[from] core::Error),
}

impl From<DirectXTextureError> for surfman::Error {
    fn from(error: DirectXTextureError) -> Self {
        match error {
            DirectXTextureError::Surfman(e) => e,
            e => {
                eprintln!("[GPU] DirectX error: {:?}", e);
                surfman::Error::DeviceOpenFailed
            }
        }
    }
}

impl super::GPURenderingContext {
    pub fn get_wgpu_texture_from_directx(
        &self,
        wgpu_device: &Device,
    ) -> Result<Texture, DirectXTextureError> {
        let device = &self.surfman_rendering_info.device.borrow();
        let mut context = self.surfman_rendering_info.context.borrow_mut();

        let size = self.size.get();

        let state = &self.d3d11_state;

        let needs_recreate = state.size_dependent.borrow().as_ref().map_or(true, |s| {
            let tex = s.wgpu_texture.size();
            tex.width != size.width || tex.height != size.height
        });
        if needs_recreate {
            *state.size_dependent.borrow_mut() =
                Some(Self::init_size_dependent_state(&state.d3d11_device, size, wgpu_device)?);
        }

        let size_dep = state.size_dependent.borrow();
        let dep_state = size_dep.as_ref().unwrap();

        let surface_texture = unsafe {
            let texture_size = Size2D::new(size.width as i32, size.height as i32);

            let raw = dep_state.d3d11_shared_texture.clone().into_raw();
            let texture_comptr = wio::com::ComPtr::from_raw(raw as *mut _);

            // Wrap our D3D11 texture in a transient EGL pbuffer via EGL_D3D_TEXTURE_ANGLE.
            device
                .create_surface_texture_from_texture(&mut *context, &texture_size, texture_comptr)
                .map_err(DirectXTextureError::Surfman)?
        };

        let gl_texture = device
            .surface_texture_object(&surface_texture)
            .ok_or_else(|| DirectXTextureError::OpenGL("No GL texture".into()))?;

        self.blit_gl_to_texture(gl_texture, size)?;

        let mut inner_surface = device
            .destroy_surface_texture(&mut *context, surface_texture)
            .map_err(|(err, _)| DirectXTextureError::Surfman(err))?;

        device
            .destroy_surface(&mut *context, &mut inner_surface)
            .map_err(DirectXTextureError::Surfman)?;

        let wgpu_texture = dep_state.wgpu_texture.clone();
        drop(size_dep);
        Ok(wgpu_texture)
    }

    fn blit_gl_to_texture(
        &self,
        gl_texture: glow::Texture,
        size: PhysicalSize<u32>,
    ) -> Result<(), DirectXTextureError> {
        let gl = &self.surfman_rendering_info.glow_gl;
        unsafe {
            let draw_framebuffer = gl.create_framebuffer().map_err(DirectXTextureError::OpenGL)?;
            gl.bind_framebuffer(glow::DRAW_FRAMEBUFFER, Some(draw_framebuffer));
            gl.framebuffer_texture_2d(
                glow::DRAW_FRAMEBUFFER,
                glow::COLOR_ATTACHMENT0,
                glow::TEXTURE_2D,
                Some(gl_texture),
                0,
            );
            gl.bind_framebuffer(glow::READ_FRAMEBUFFER, None);
            let (w, h) = (size.width as i32, size.height as i32);
            gl.blit_framebuffer(0, 0, w, h, 0, h, w, 0, glow::COLOR_BUFFER_BIT, glow::NEAREST);
            gl.flush();
            gl.bind_framebuffer(glow::DRAW_FRAMEBUFFER, None);
            gl.delete_framebuffer(draw_framebuffer);
        }
        Ok(())
    }

    pub(crate) fn init_d3d11_shared_state(
        device: &surfman::Device,
    ) -> Result<D3D11SharedState, DirectXTextureError> {
        let native_device = device.native_device();
        if native_device.d3d11_device.is_null() {
            return Err(DirectXTextureError::OpenGL("ANGLE D3D11 device is null".into()));
        }
        let d3d11_device: ID3D11Device =
            unsafe { core::IUnknown::from_raw(native_device.d3d11_device as *mut _).cast()? };

        Ok(D3D11SharedState { d3d11_device, size_dependent: std::cell::RefCell::new(None) })
    }

    fn init_size_dependent_state(
        d3d11_device: &ID3D11Device,
        size: PhysicalSize<u32>,
        wgpu_device: &Device,
    ) -> Result<D3D11SizeDependentState, DirectXTextureError> {
        let (d3d11_shared_texture, wgpu_texture) =
            unsafe {
                let mut dx12_texture_ptr: Option<ID3D11Texture2D> = None;

                d3d11_device.CreateTexture2D(
                    &Direct3D11::D3D11_TEXTURE2D_DESC {
                        Width: size.width,
                        Height: size.height,
                        MipLevels: 1,
                        ArraySize: 1,
                        CPUAccessFlags: 0,
                        Format: Common::DXGI_FORMAT_R8G8B8A8_UNORM,
                        SampleDesc: Common::DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                        Usage: Direct3D11::D3D11_USAGE_DEFAULT,
                        BindFlags: (Direct3D11::D3D11_BIND_RENDER_TARGET.0
                            | Direct3D11::D3D11_BIND_SHADER_RESOURCE.0)
                            as u32,
                        MiscFlags: (Direct3D11::D3D11_RESOURCE_MISC_SHARED.0
                            | Direct3D11::D3D11_RESOURCE_MISC_SHARED_NTHANDLE.0)
                            as u32,
                    },
                    None,
                    Some(&mut dx12_texture_ptr),
                )?;

                let d3d11_dx12_texture = dx12_texture_ptr.unwrap();

                let nt_handle = d3d11_dx12_texture
                    .cast::<Dxgi::IDXGIResource1>()?
                    .CreateSharedHandle(None, Dxgi::DXGI_SHARED_RESOURCE_READ.0, None)?;

                let hal_device =
                    wgpu_device.as_hal::<Dx12>().ok_or(DirectXTextureError::WgpuNotDx12)?;
                let dx12_device = hal_device.raw_device().clone();

                let mut dx12_resource_ptr: Option<Direct3D12::ID3D12Resource> = None;
                dx12_device.OpenSharedHandle(nt_handle, &mut dx12_resource_ptr)?;
                let dx12_resource = dx12_resource_ptr.unwrap();
                Foundation::CloseHandle(nt_handle)?;

                let extent =
                    Extent3d { width: size.width, height: size.height, depth_or_array_layers: 1 };

                let wgpu_texture = wgpu_device.create_texture_from_hal::<Dx12>(
                    <Dx12 as wgpu_hal::Api>::Device::texture_from_raw(
                        dx12_resource,
                        TextureFormat::Rgba8Unorm,
                        TextureDimension::D2,
                        extent,
                        1,
                        1,
                    ),
                    &TextureDescriptor {
                        label: Some("servo webview shared texture"),
                        size: extent,
                        format: TextureFormat::Rgba8Unorm,
                        dimension: TextureDimension::D2,
                        mip_level_count: 1,
                        sample_count: 1,
                        usage: TextureUsages::TEXTURE_BINDING | TextureUsages::RENDER_ATTACHMENT,
                        view_formats: &[],
                    },
                );

                (d3d11_dx12_texture, wgpu_texture)
            };

        Ok(D3D11SizeDependentState { d3d11_shared_texture, wgpu_texture })
    }
}
