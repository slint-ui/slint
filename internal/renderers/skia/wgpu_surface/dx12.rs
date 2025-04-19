// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_core::api::PhysicalSize as PhysicalWindowSize;

use windows::Win32::Foundation::{DXGI_STATUS_OCCLUDED, HANDLE, HWND, S_OK};
use windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL_11_0;
use windows::Win32::Graphics::Direct3D12::{
    D3D12CreateDevice, ID3D12CommandQueue, ID3D12Device, ID3D12Fence, ID3D12Resource,
    D3D12_COMMAND_LIST_TYPE_DIRECT, D3D12_COMMAND_QUEUE_DESC, D3D12_FENCE_FLAG_NONE,
    D3D12_RESOURCE_STATE_PRESENT,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_STANDARD_MULTISAMPLE_QUALITY_PATTERN;
use windows::Win32::Graphics::Dxgi::{
    Common::{DXGI_FORMAT, DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_SAMPLE_DESC},
    CreateDXGIFactory2, IDXGIFactory4, IDXGISwapChain3, DXGI_ADAPTER_FLAG, DXGI_ADAPTER_FLAG_NONE,
    DXGI_ADAPTER_FLAG_SOFTWARE, DXGI_CREATE_FACTORY_FLAGS, DXGI_PRESENT, DXGI_SWAP_CHAIN_DESC1,
    DXGI_SWAP_CHAIN_FLAG, DXGI_SWAP_EFFECT_FLIP_DISCARD, DXGI_USAGE_RENDER_TARGET_OUTPUT,
};
use windows::Win32::System::Threading::{CreateEventW, WaitForSingleObjectEx, INFINITE};

use wgpu_25 as wgpu;

pub unsafe fn make_dx12_surface(
    size: PhysicalWindowSize,
    gr_context: &mut skia_safe::gpu::DirectContext,
    frame: &wgpu_25::SurfaceTexture,
) -> Option<skia_safe::Surface> {
    frame.texture.as_hal::<wgpu::wgc::api::Dx12, _, _>(
        |dx12_texture: Option<&wgpu::hal::dx12::Texture>| {
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
        },
    )
}

pub unsafe fn import_dx12_texture(
    canvas: &skia_safe::Canvas,
    texture: wgpu::Texture,
) -> Option<skia_safe::Image> {
    texture.as_hal::<wgpu::wgc::api::Dx12, _, _>(
        |dx12_texture: Option<&wgpu::hal::dx12::Texture>| {
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
        },
    )
}

fn find_matching_adapter(device: &ID3D12Device) -> Option<skia_safe::gpu::d3d::IDXGIAdapter1> {
    unsafe {
        let dxgi_factory: IDXGIFactory4 =
            unsafe { CreateDXGIFactory2(DXGI_CREATE_FACTORY_FLAGS(0)) }.unwrap();

        let mut index = 0;
        loop {
            let adapter_result = dxgi_factory.EnumAdapters1(index);
            if let Err(err) = adapter_result {
                break;
            }

            let adapter = adapter_result.unwrap();

            let mut test_device: Option<ID3D12Device> = None;

            if D3D12CreateDevice(&adapter, D3D_FEATURE_LEVEL_11_0, &mut test_device).is_ok() {
                if let Some(test_dev) = test_device {
                    use windows::core::Interface;
                    // Compare IUnknown identity
                    if test_dev
                        .cast::<windows::core::IUnknown>()
                        .unwrap()
                        .eq(&device.cast::<windows::core::IUnknown>().unwrap())
                    {
                        return Some(adapter);
                    }
                }
            }

            index += 1;
        }

        None
    }
}

pub unsafe fn make_dx12_context(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Option<skia_safe::gpu::DirectContext> {
    let backend = unsafe {
        queue.as_hal::<wgpu::wgc::api::Dx12, _, _>(
            |maybe_dx12_queue: Option<&wgpu::hal::dx12::Queue>| {
                maybe_dx12_queue.map(|dx12_queue| {
                    let dx12_queue_raw = dx12_queue.as_raw();
                    let mut dx12_device_old: Option<
                        windows_58::Win32::Graphics::Direct3D12::ID3D12Device,
                    > = None;
                    dx12_queue_raw.GetDevice(&mut dx12_device_old as _).unwrap();
                    let dx12_device_old = dx12_device_old.unwrap();
                    let dx12_device = windows_core::Interface::from_raw(
                        windows_core_58::Interface::into_raw(dx12_device_old),
                    );

                    skia_safe::gpu::d3d::BackendContext {
                        adapter: find_matching_adapter(&dx12_device).unwrap(),
                        device: dx12_device,
                        queue: windows_core::Interface::from_raw(
                            windows_core_58::Interface::into_raw(dx12_queue_raw.clone()),
                        ),
                        memory_allocator: None,
                        protected_context: skia_safe::gpu::Protected::No,
                    }
                })
            },
        )
    };

    skia_safe::gpu::DirectContext::new_d3d(&backend.unwrap(), None)
}
