// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// This module bridges ANGLE's D3D11 rendering surface to wgpu's DX12 pipeline.
// The core problem: Servo renders via OpenGL (through ANGLE, which uses D3D11 under the hood),
// but Slint composites via wgpu (which uses DX12). We need to get the rendered pixels from
// one API to the other each frame without a CPU round-trip.
//
// The path is: ANGLE D3D11 surface → shared NT handle → DX12 resource → wgpu texture.
// A keyed mutex on the shared texture serialises access between the two devices.

use std::ffi;

use winit::dpi::PhysicalSize;

use slint::wgpu_28::wgpu::{
    AddressMode, BindGroupEntry, BindingResource, Device, Extent3d, FilterMode, Queue, Texture,
    TextureDescriptor, TextureDimension, TextureFormat, TextureUsages, TextureView,
    TextureViewDescriptor, wgc::api::Dx12,
};

use windows::{
    Win32::{
        Foundation,
        Graphics::{
            Direct3D11::{self, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D},
            Direct3D12,
            Dxgi::{self, Common, IDXGIKeyedMutex},
        },
    },
    core::{self, Interface},
};

// Cached shared texture handle from a surfman/ANGLE surface.
// ANGLE reuses the same backing texture (and therefore the same share handle) for a given
// surface across frames, so we cache the opened resource to avoid re-opening it every time.
pub struct CachedTexture {
    pub share_handle: usize,
    pub texture: ID3D11Texture2D,
    pub mutex: IDXGIKeyedMutex,
}

// Size-dependent GPU state that is recreated when the surface dimensions change.
// Contains the shared D3D11↔DX12 texture, the wgpu flip target, and the pre-recorded
// render bundle for the vertical-flip blit.
pub struct D3D11SizeDependentState {
    pub size: PhysicalSize<u32>,
    // Shared texture visible to both D3D11 (ANGLE side) and DX12 (wgpu side).
    // Created once and reused each frame; recreated only on resize.
    pub d3d11_dx12_texture: ID3D11Texture2D,
    pub d3d11_dx12_mutex: IDXGIKeyedMutex,
    // wgpu render target for the vertical-flip blit (OpenGL is bottom-left origin,
    // wgpu/DX is top-left). This is what Slint ultimately composites.
    pub wgpu_flip_target: Texture,
    pub wgpu_flip_target_view: TextureView,
    // Pre-recorded render bundle for the flip blit — avoids re-recording each frame.
    pub flip_bundle: wgpu::RenderBundle,
    // Cached source textures keyed by share handle, avoiding repeated OpenSharedResource calls.
    pub cached_src_textures: Vec<CachedTexture>,
}

// Long-lived D3D11 state shared across frames and resizes.
// Contains ANGLE's D3D11 device/context, EGL function pointers, and the wgpu flip pipeline.
pub struct D3D11SharedState {
    // D3D11 device and context obtained from ANGLE — these are ANGLE's internal objects,
    // not ones we created. We borrow them via EGL device query extensions.
    pub d3d11_device: ID3D11Device,
    pub d3d11_ctx: ID3D11DeviceContext,
    pub flip_pipeline: wgpu::RenderPipeline,
    pub sampler: wgpu::Sampler,
    pub size_dependent: Option<D3D11SizeDependentState>,
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

// Fullscreen-triangle flip shader. Positions and UVs are derived entirely from the vertex
// index — no vertex buffer required. The Y UV coordinate is inverted to correct for
// OpenGL's bottom-left origin vs. DX's top-left origin.
const FLIP_SHADER_WGSL: &str = "
    struct VertexOutput {
        @builtin(position) position: vec4<f32>,
        @location(0) uv: vec2<f32>,
    };

    @vertex
    fn vs_main(@builtin(vertex_index) vi: u32) -> VertexOutput {
        var out: VertexOutput;
        let uv = vec2<f32>(vec2<u32>((vi << 1u) & 2u, vi & 2u));
        out.uv = uv; // ANGLE/DX shared surface is already in the correct orientation
        out.position = vec4<f32>(uv * 2.0 - vec2<f32>(1.0), 0.0, 1.0);
        return out;
    }

    @group(0) @binding(0) var t_diffuse: texture_2d<f32>;
    @group(0) @binding(1) var s_diffuse: sampler;

    @fragment
    fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
        return textureSample(t_diffuse, s_diffuse, in.uv);
    }
";

// Grants the consumer (DX12) read access on the shared NT handle.
// We only need read access since DX12/wgpu only samples from the shared texture.
const DXGI_SHARED_RESOURCE_READ: u32 = 0x80000000;

impl super::GPURenderingContext {
    // Imports the current ANGLE D3D11 surface as a wgpu texture for Slint compositing.
    //
    // This is the per-frame hot path. It:
    // 1. Queries the ANGLE EGL surface for its D3D11 share handle.
    // 2. Opens (and caches) the shared D3D11 texture from that handle.
    // 3. Copies the source texture into a D3D11↔DX12 shared texture via keyed mutex.
    // 4. Runs a pre-recorded wgpu render pass to flip the texture vertically.
    // 5. Returns the flip target texture for Slint to composite.
    pub fn get_wgpu_texture_from_directx(
        &self,
        wgpu_device: &Device,
        wgpu_queue: &Queue,
    ) -> Result<Texture, DirectXTextureError> {
        let device = &self.surfman_rendering_info.device.borrow();
        let mut context = self.surfman_rendering_info.context.borrow_mut();

        device.make_context_current(&context).map_err(DirectXTextureError::Surfman)?;

        let size = self.size.get();

        unsafe {
            // Access wgpu's underlying DX12 device

            let hal_device =
                wgpu_device.as_hal::<Dx12>().ok_or(DirectXTextureError::WgpuNotDx12)?;
            let dx12_device = hal_device.raw_device().clone();

            // Lazy-initialise the shared D3D11 state (once per lifetime)

            let mut state = self.d3d11_state.borrow_mut();

            if state.is_none() {
                *state = Some(Self::init_d3d11_shared_state(device, wgpu_device)?);
            }

            let state_ref = state.as_mut().unwrap();

            // Recreate size-dependent resources on resize
            let needs_resize = state_ref.size_dependent.as_ref().map_or(true, |s| s.size != size);
            if needs_resize {
                state_ref.size_dependent = Some(Self::init_size_dependent_state(
                    state_ref,
                    size,
                    wgpu_device,
                    &dx12_device,
                )?);
            }
            let dep_state = state_ref.size_dependent.as_mut().unwrap();

            let surface = device
                .unbind_surface_from_context(&mut *context)
                .map_err(DirectXTextureError::Surfman)?
                .ok_or(DirectXTextureError::NoSurface)?;

            let share_handle =
                surface.d3d11_share_handle().ok_or(DirectXTextureError::NullShareHandle)?
                    as *mut ffi::c_void;

            let cached = Self::get_or_open_cached_src_texture(
                &state_ref.d3d11_device,
                share_handle,
                &mut dep_state.cached_src_textures,
            )?;

            // Protocol (key=0 throughout):
            //   1. Acquire src  — waits until ANGLE has released the keyed mutex after flushing GL work.
            //   2. Acquire dst  — guarantees DX12/wgpu has finished reading the shared texture.
            //   3. CopyResource — D3D11 GPU-side copy src → dst (no CPU stall).
            //   4. Flush        — ensures the copy command is submitted before release.
            //   5. Release dst  — signals DX12/wgpu that the shared texture is ready.
            //   6. Release src  — signals ANGLE it may render the next frame.

            cached.mutex.AcquireSync(0, u32::MAX)?;
            dep_state.d3d11_dx12_mutex.AcquireSync(0, u32::MAX)?;

            state_ref.d3d11_ctx.CopyResource(&dep_state.d3d11_dx12_texture, &cached.texture);
            state_ref.d3d11_ctx.Flush();

            dep_state.d3d11_dx12_mutex.ReleaseSync(0)?;
            cached.mutex.ReleaseSync(0)?;

            let _ = device.bind_surface_to_context(&mut *context, surface).map_err(
                |(err, mut surface)| {
                    let _ = device.destroy_surface(&mut *context, &mut surface);
                    DirectXTextureError::Surfman(err)
                },
            );

            let mut encoder =
                wgpu_device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
            {
                let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: None,
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &dep_state.wgpu_flip_target_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                            store: wgpu::StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                rpass.execute_bundles(std::iter::once(&dep_state.flip_bundle));
            }
            wgpu_queue.submit(Some(encoder.finish()));

            Ok(dep_state.wgpu_flip_target.clone())
        }
    }

    // One-time initialisation of the D3D11 shared state.
    // Retrieves ANGLE's D3D11 device and context via EGL device query extensions,
    // caches EGL function pointers, and creates the wgpu flip pipeline and sampler.
    #[allow(unsafe_op_in_unsafe_fn)]
    unsafe fn init_d3d11_shared_state(
        device: &surfman::Device,
        wgpu_device: &Device,
    ) -> Result<D3D11SharedState, DirectXTextureError> {
        let native_device = device.native_device();
        let d3d11_device_ptr = native_device.d3d11_device;

        if d3d11_device_ptr.is_null() {
            return Err(DirectXTextureError::OpenGL(
                "Failed to query ANGLE's D3D11 device — native_device.d3d11_device is null".into(),
            ));
        }

        let d3d11_device: ID3D11Device = {
            // surfman's `native_device()` implicitly increments the COM reference count.
            // We use `from_raw` to take ownership of this incremented refcount.
            let unknown = core::IUnknown::from_raw(d3d11_device_ptr as *mut _);
            unknown.cast()?
        };
        let d3d11_ctx = d3d11_device.GetImmediateContext()?;

        let shader = wgpu_device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("D3D11 Flip Shader"),
            source: wgpu::ShaderSource::Wgsl(FLIP_SHADER_WGSL.into()),
        });

        let flip_pipeline = wgpu_device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("D3D11 Flip Pipeline"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: TextureFormat::Rgba8Unorm,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let sampler = wgpu_device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Nearest,
            min_filter: FilterMode::Nearest,
            ..Default::default()
        });

        Ok(D3D11SharedState {
            d3d11_device,
            d3d11_ctx,
            flip_pipeline,
            sampler,
            size_dependent: None,
        })
    }

    // Creates size-dependent resources: the D3D11↔DX12 shared texture, the wgpu flip target,
    // and the pre-recorded render bundle for the flip blit.
    // Called on first frame and whenever the surface dimensions change.
    #[allow(unsafe_op_in_unsafe_fn)]
    unsafe fn init_size_dependent_state(
        state_ref: &D3D11SharedState,
        size: PhysicalSize<u32>,
        wgpu_device: &Device,
        dx12_device: &Direct3D12::ID3D12Device,
    ) -> Result<D3D11SizeDependentState, DirectXTextureError> {
        // This texture is the bridge between ANGLE (D3D11) and wgpu (DX12).
        // SHARED_NTHANDLE enables cross-API sharing; SHARED_KEYEDMUTEX serialises access.

        let shared_texture_desc = Direct3D11::D3D11_TEXTURE2D_DESC {
            Width: size.width,
            Height: size.height,
            MipLevels: 1,
            ArraySize: 1,
            Format: Common::DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: Common::DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            Usage: Direct3D11::D3D11_USAGE_DEFAULT,
            BindFlags: Direct3D11::D3D11_BIND_SHADER_RESOURCE.0 as u32,
            CPUAccessFlags: 0,
            MiscFlags: (Direct3D11::D3D11_RESOURCE_MISC_SHARED_NTHANDLE.0
                | Direct3D11::D3D11_RESOURCE_MISC_SHARED_KEYEDMUTEX.0)
                as u32,
        };

        let mut dx12_texture_ptr: Option<ID3D11Texture2D> = None;
        state_ref.d3d11_device.CreateTexture2D(
            &shared_texture_desc,
            None,
            Some(&mut dx12_texture_ptr),
        )?;
        let d3d11_dx12_texture = dx12_texture_ptr.unwrap();
        let d3d11_dx12_mutex: IDXGIKeyedMutex = d3d11_dx12_texture.cast()?;

        // NT handles must be explicitly closed after OpenSharedHandle consumes them,
        // unlike legacy DXGI handles which are reference-counted.

        let nt_handle = d3d11_dx12_texture.cast::<Dxgi::IDXGIResource1>()?.CreateSharedHandle(
            None,
            DXGI_SHARED_RESOURCE_READ,
            None,
        )?;

        let mut dx12_resource_ptr: Option<Direct3D12::ID3D12Resource> = None;
        dx12_device.OpenSharedHandle(nt_handle, &mut dx12_resource_ptr)?;
        let dx12_resource = dx12_resource_ptr.unwrap();
        Foundation::CloseHandle(nt_handle)?;

        // A drop callback ensures the DX12 resource is properly released when wgpu
        // drops the texture, preventing GPU memory leaks (inspired by the Vulkan
        // implementation's cleanup pattern in gpu_rendering_context.rs).

        let extent = Extent3d { width: size.width, height: size.height, depth_or_array_layers: 1 };

        let shared_wgpu_texture = wgpu_device.create_texture_from_hal::<Dx12>(
            <Dx12 as wgpu_hal::Api>::Device::texture_from_raw(
                dx12_resource,
                TextureFormat::Bgra8Unorm,
                TextureDimension::D2,
                extent,
                1,
                1,
            ),
            &TextureDescriptor {
                label: Some("D3D11↔DX12 Shared Texture"),
                size: extent,
                format: TextureFormat::Bgra8Unorm,
                dimension: TextureDimension::D2,
                mip_level_count: 1,
                sample_count: 1,
                usage: TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
        );

        // Separate render target for the flip blit — this is what Slint composites.

        let wgpu_flip_target = wgpu_device.create_texture(&TextureDescriptor {
            label: Some("D3D11 Flip Target"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let wgpu_flip_target_view = wgpu_flip_target.create_view(&TextureViewDescriptor::default());

        // Pre-record the flip blit as a render bundle to avoid re-recording each frame.

        let flip_bind_group = wgpu_device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &state_ref.flip_pipeline.get_bind_group_layout(0),
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(
                        &shared_wgpu_texture.create_view(&TextureViewDescriptor::default()),
                    ),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Sampler(&state_ref.sampler),
                },
            ],
            label: Some("D3D11 Flip Bind Group"),
        });

        let mut bundle_enc =
            wgpu_device.create_render_bundle_encoder(&wgpu::RenderBundleEncoderDescriptor {
                label: Some("D3D11 Flip Bundle"),
                color_formats: &[Some(TextureFormat::Rgba8Unorm)],
                depth_stencil: None,
                sample_count: 1,
                multiview: None,
            });
        bundle_enc.set_pipeline(&state_ref.flip_pipeline);
        bundle_enc.set_bind_group(0, &flip_bind_group, &[]);
        bundle_enc.draw(0..3, 0..1);
        let flip_bundle = bundle_enc.finish(&wgpu::RenderBundleDescriptor { label: None });

        Ok(D3D11SizeDependentState {
            size,
            d3d11_dx12_texture,
            d3d11_dx12_mutex,
            wgpu_flip_target,
            wgpu_flip_target_view,
            flip_bundle,
            cached_src_textures: Vec::new(),
        })
    }

    #[allow(unsafe_op_in_unsafe_fn)]
    unsafe fn get_or_open_cached_src_texture<'a>(
        d3d11_device: &ID3D11Device,
        share_handle: *mut ffi::c_void,
        cache: &'a mut Vec<CachedTexture>,
    ) -> Result<&'a CachedTexture, DirectXTextureError> {
        let share_ptr = share_handle as usize;
        if let Some(index) = cache.iter().position(|c| c.share_handle == share_ptr) {
            return Ok(&cache[index]);
        }

        let mut src_ptr_res: Option<ID3D11Texture2D> = None;
        d3d11_device.OpenSharedResource(Foundation::HANDLE(share_handle), &mut src_ptr_res)?;
        let texture = src_ptr_res.ok_or(DirectXTextureError::NullShareHandle)?;
        let mutex: IDXGIKeyedMutex = texture.cast()?;

        cache.push(CachedTexture { share_handle: share_ptr, texture, mutex });

        Ok(cache.last().unwrap())
    }
}
