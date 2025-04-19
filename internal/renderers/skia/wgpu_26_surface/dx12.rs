// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_core::api::PhysicalSize as PhysicalWindowSize;

use windows::Win32::Graphics::Direct3D12::{ID3D12Resource, D3D12_RESOURCE_STATE_PRESENT};
use windows::Win32::Graphics::Dxgi::Common::DXGI_STANDARD_MULTISAMPLE_QUALITY_PATTERN;
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_FORMAT_R8G8B8A8_UNORM_SRGB,
};

use wgpu_26 as wgpu;

pub unsafe fn make_dx12_surface(
    size: PhysicalWindowSize,
    gr_context: &mut skia_safe::gpu::DirectContext,
    frame: &wgpu::SurfaceTexture,
) -> Option<skia_safe::Surface> {
    let dx12_texture = frame.texture.as_hal::<wgpu::wgc::api::Dx12>();

    let texture_info = skia_safe::gpu::d3d::TextureResourceInfo {
        resource: windows_core::Interface::from_raw(windows_core_58::Interface::into_raw(
            dx12_texture.unwrap().raw_resource().clone(),
        )),
        alloc: None,
        resource_state: D3D12_RESOURCE_STATE_PRESENT,
        format: DXGI_FORMAT_R8G8B8A8_UNORM,
        sample_count: 1,
        level_count: 1,
        sample_quality_pattern: DXGI_STANDARD_MULTISAMPLE_QUALITY_PATTERN,
        protected: skia_safe::gpu::Protected::No,
    };

    let backend_render_target = skia_safe::gpu::BackendRenderTarget::new_d3d(
        (size.width as i32, size.height as i32),
        &texture_info,
    );

    skia_safe::gpu::surfaces::wrap_backend_render_target(
        gr_context,
        &backend_render_target,
        skia_safe::gpu::SurfaceOrigin::TopLeft,
        skia_safe::ColorType::RGBA8888,
        None,
        None,
    )
}

#[allow(non_snake_case)]
pub unsafe fn import_dx12_texture(
    canvas: &skia_safe::Canvas,
    texture: wgpu::Texture,
) -> Option<skia_safe::Image> {
    let dx12_texture = texture.as_hal::<wgpu::wgc::api::Dx12>();

    let resource: ID3D12Resource = windows_core::Interface::from_raw(
        windows_core_58::Interface::into_raw(dx12_texture.unwrap().raw_resource().clone()),
    );

    let dxgi_texture_format = resource.GetDesc().Format;

    let color_type = match dxgi_texture_format {
        DXGI_FORMAT_R8G8B8A8_UNORM => skia_safe::ColorType::RGBA8888,
        DXGI_FORMAT_R8G8B8A8_UNORM_SRGB => skia_safe::ColorType::SRGBA8888,
        _ => return None,
    };

    let texture_info = skia_safe::gpu::d3d::TextureResourceInfo {
        resource,
        alloc: None,
        resource_state: D3D12_RESOURCE_STATE_PRESENT,
        format: dxgi_texture_format,
        sample_count: 1,
        level_count: 1,
        sample_quality_pattern: DXGI_STANDARD_MULTISAMPLE_QUALITY_PATTERN,
        protected: skia_safe::gpu::Protected::No,
    };
    let size = texture.size();

    let backend_texture = skia_safe::gpu::BackendTexture::new_d3d(
        (size.width as i32, size.height as i32),
        &texture_info,
    );

    Some(
        skia_safe::image::Image::from_texture(
            canvas.recording_context().as_mut().unwrap(),
            &backend_texture,
            skia_safe::gpu::SurfaceOrigin::TopLeft,
            color_type,
            skia_safe::AlphaType::Unpremul,
            None,
        )
        .unwrap(),
    )
}

pub unsafe fn make_dx12_context(
    adapter: &wgpu::Adapter,
    _device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Option<skia_safe::gpu::DirectContext> {
    let backend = unsafe {
        let maybe_dx12_queue = queue.as_hal::<wgpu::wgc::api::Dx12>();
        let dx12_adapter = adapter.as_hal::<wgpu::wgc::api::Dx12>().unwrap();

        maybe_dx12_queue.map(|dx12_queue| {
            let dx12_queue_raw = dx12_queue.as_raw();
            let mut dx12_device_old: Option<windows_58::Win32::Graphics::Direct3D12::ID3D12Device> =
                None;
            dx12_queue_raw.GetDevice(&mut dx12_device_old as _).unwrap();
            let dx12_device_old = dx12_device_old.unwrap();
            let dx12_device = windows_core::Interface::from_raw(
                windows_core_58::Interface::into_raw(dx12_device_old),
            );

            let idxgiadapter_1: windows_58::Win32::Graphics::Dxgi::IDXGIAdapter1 =
                dx12_adapter.as_raw().clone().into();

            skia_safe::gpu::d3d::BackendContext {
                adapter: windows_core::Interface::from_raw(windows_core_58::Interface::into_raw(
                    idxgiadapter_1,
                )),
                device: dx12_device,
                queue: windows_core::Interface::from_raw(windows_core_58::Interface::into_raw(
                    dx12_queue_raw.clone(),
                )),
                memory_allocator: None,
                protected_context: skia_safe::gpu::Protected::No,
            }
        })
    };

    skia_safe::gpu::DirectContext::new_d3d(&backend.unwrap(), None)
}
