// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

//! This module provides function(s) to integrate a bevy App into a Slint application.
//!
//! The integration's entry point is [`run_bevy_app_with_slint()`], which will launch the
//! bevy [`App`] in a thread separate from the main thread and supply textures of the rendered
//! scenes via channels.

use std::sync::Arc;

use slint::wgpu_26::wgpu;

use bevy::{
    prelude::*,
    render::{
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        render_graph::{self, NodeRunError, RenderGraph, RenderGraphContext, RenderLabel},
        renderer::RenderContext,
        settings::RenderCreation,
        RenderApp, RenderPlugin,
    },
};

/// This enum describes the two kinds of message the Slint application send to the bevy integration thread.
pub enum ControlMessage {
    /// Send this message when you don't need a previously received texture anymore.
    ReleaseFrontBufferTexture { texture: wgpu::Texture },
    /// Send this message to adjust the size of the scene textures.
    ResizeBuffers { width: u32, height: u32 },
}

/// Initializes Bevy and Slint, spawns a bevy [`App`], and supplies textures of the rendered scenes via channels.
///
/// Use the `bevy_app_pre_default_plugins_callback` callback to add any plugins to the app before the default plugins.
/// Use the `bevy_main` callback to add systems, plugins, etc. to your app and call [`App::run()`].
///
/// If successful, this function returns two channels:
/// - Use the receiver channel to obtain textures for use in the Slint UI. These textures have the scene of your default
///   camera rendered into.
/// - Use the [`ControlMessage`] sender channel to return textures that you don't need anymore, as well as to inform the
///   renderer to resize the texture if needed.
///
/// *Note*: At the moment only one single camera is supported.
pub async fn run_bevy_app_with_slint(
    bevy_app_pre_default_plugins_callback: impl FnOnce(&mut App) + Send + 'static,
    bevy_main: impl FnOnce(App) + Send + 'static,
) -> Result<
    (smol::channel::Receiver<wgpu::Texture>, smol::channel::Sender<ControlMessage>),
    slint::PlatformError,
> {
    let backends = wgpu::Backends::from_env().unwrap_or_default();
    let dx12_shader_compiler = wgpu::Dx12Compiler::from_env().unwrap_or_default();
    let gles_minor_version = wgpu::Gles3MinorVersion::from_env().unwrap_or_default();

    let instance = wgpu::util::new_instance_with_webgpu_detection(&wgpu::InstanceDescriptor {
        backends,
        flags: wgpu::InstanceFlags::from_build_config().with_env(),
        backend_options: wgpu::BackendOptions {
            dx12: wgpu::Dx12BackendOptions { shader_compiler: dx12_shader_compiler },
            gl: wgpu::GlBackendOptions {
                gles_minor_version,
                fence_behavior: wgpu::GlFenceBehavior::default(),
            },
            noop: wgpu::NoopBackendOptions::default(),
        },
        memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
    })
    .await;

    let (render_device, render_queue, adapter_info, adapter) =
        bevy::render::renderer::initialize_renderer(
            &instance,
            &bevy::render::settings::WgpuSettings::default(),
            &wgpu::RequestAdapterOptions::default(),
            None,
        )
        .await;

    let selector =
        slint::BackendSelector::new().require_wgpu_26(slint::wgpu_26::WGPUConfiguration::Manual {
            instance: instance.clone(),
            adapter: (**adapter.0).clone(),
            device: render_device.wgpu_device().clone(),
            queue: (**render_queue.0).clone(),
        });
    selector.select()?;

    let (control_message_sender, control_message_receiver) =
        smol::channel::bounded::<ControlMessage>(2);
    let (bevy_front_buffer_sender, bevy_front_buffer_receiver) =
        smol::channel::bounded::<wgpu::Texture>(2);

    let wgpu_device = render_device.wgpu_device().clone();

    let create_texture = move |label, width, height| {
        wgpu_device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb, // Can only render to SRGB texture - https://github.com/bevyengine/bevy/issues/15201
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
    };

    let front_buffer = create_texture("Front Buffer", 640, 480);
    let back_buffer = create_texture("Back Buffer", 640, 480);
    let inflight_buffer = create_texture("Back Buffer", 640, 480);

    let mut buffer_width = 640;
    let mut buffer_height = 480;

    let _bevy_thread = std::thread::spawn(move || {
        let runner = move |mut app: bevy::app::App| {
            app.finish();
            app.cleanup();

            let mut next_texture_view_id: u32 = 0;

            loop {
                let mut next_back_buffer = match control_message_receiver.recv_blocking() {
                    Ok(ControlMessage::ReleaseFrontBufferTexture { texture }) => texture,
                    Ok(ControlMessage::ResizeBuffers { width, height }) => {
                        buffer_width = width;
                        buffer_height = height;
                        continue;
                    }
                    Err(_) => break,
                };

                if next_back_buffer.width() != buffer_width
                    || next_back_buffer.height() != buffer_height
                {
                    next_back_buffer = create_texture("back buffer", buffer_width, buffer_height);
                }

                let texture_view = next_back_buffer.create_view(&wgpu::TextureViewDescriptor {
                    label: Some("bevy back buffer texture view"),
                    format: None,
                    dimension: None,
                    ..Default::default()
                });
                let texture_view_handle =
                    bevy::camera::ManualTextureViewHandle(next_texture_view_id);
                next_texture_view_id += 1;
                {
                    let world = app.world_mut();

                    let mut back_buffer = world.get_resource_mut::<BackBuffer>().unwrap();
                    back_buffer.0 = Some(next_back_buffer.clone());

                    let mut manual_texture_views = world
                        .get_resource_mut::<bevy::render::texture::ManualTextureViews>()
                        .unwrap();
                    manual_texture_views.clear();
                    manual_texture_views.insert(
                        texture_view_handle,
                        bevy::render::texture::ManualTextureView {
                            texture_view: texture_view.into(),
                            size: (next_back_buffer.width(), next_back_buffer.height()).into(),
                            format: bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
                        },
                    );
                    let mut cameras = world.query::<&mut Camera>();
                    if let Some(mut c) = cameras.iter_mut(world).next() {
                        c.target = bevy::camera::RenderTarget::TextureView(texture_view_handle);
                    }
                }

                app.update();
            }

            bevy::app::AppExit::Success
        };

        let mut app = App::new();
        app.set_runner(runner);
        app.insert_resource(BackBuffer(None));
        bevy_app_pre_default_plugins_callback(&mut app);
        app.add_plugins(
            DefaultPlugins.set(ImagePlugin::default_nearest()).set(RenderPlugin {
                render_creation: RenderCreation::manual(
                    render_device,
                    render_queue,
                    adapter_info,
                    adapter,
                    bevy::render::renderer::RenderInstance(Arc::new(
                        bevy::render::WgpuWrapper::new(instance),
                    )),
                ),
                ..default()
            }), //.disable::<bevy::winit::WinitPlugin>(),
        );
        app.add_plugins(SlintRenderToTexturePlugin(bevy_front_buffer_sender));
        app.add_plugins(ExtractResourcePlugin::<BackBuffer>::default());

        bevy_main(app)
    });

    control_message_sender
        .send_blocking(ControlMessage::ReleaseFrontBufferTexture { texture: back_buffer })
        .unwrap();
    control_message_sender
        .send_blocking(ControlMessage::ReleaseFrontBufferTexture { texture: inflight_buffer })
        .unwrap();
    control_message_sender
        .send_blocking(ControlMessage::ReleaseFrontBufferTexture { texture: front_buffer })
        .unwrap();

    Ok((bevy_front_buffer_receiver, control_message_sender))
}

#[derive(Resource, Deref)]
struct FrontBufferReturnSender(smol::channel::Sender<wgpu::Texture>);
/// Plugin for Render world part of work
struct SlintRenderToTexturePlugin(smol::channel::Sender<wgpu::Texture>);
impl Plugin for SlintRenderToTexturePlugin {
    fn build(&self, app: &mut App) {
        let render_app = app.sub_app_mut(RenderApp);

        let mut graph = render_app.world_mut().resource_mut::<RenderGraph>();
        graph.add_node(SlintSwapChain, SlintSwapChainDriver);
        graph.add_node_edge(bevy::render::graph::CameraDriverLabel, SlintSwapChain);

        render_app.insert_resource(FrontBufferReturnSender(self.0.clone()));
    }
}

#[derive(Clone, Resource, ExtractResource, Deref, DerefMut)]
struct BackBuffer(pub Option<wgpu::Texture>);

#[derive(Debug, PartialEq, Eq, Clone, Hash, RenderLabel)]
struct SlintSwapChain;

#[derive(Default)]
struct SlintSwapChainDriver;

impl render_graph::Node for SlintSwapChainDriver {
    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        _render_context: &mut RenderContext,
        world: &World,
    ) -> Result<(), NodeRunError> {
        let front_buffer_sender = world.get_resource::<FrontBufferReturnSender>().unwrap();
        let back_buffer = world.get_resource::<BackBuffer>().unwrap();

        if let Some(bb) = &back_buffer.0 {
            // silently ignore errors when the sender is closed. Reporting an error would just result in bevy panicing,
            // while a closed channel is indicating a shutdown condition.
            front_buffer_sender.0.send_blocking(bb.clone()).ok();
        }

        Ok(())
    }
}
