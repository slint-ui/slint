// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::{cell::RefCell, rc::Rc};

use i_slint_core::{api::PhysicalSize as PhysicalWindowSize, graphics::RequestedGraphicsAPI};

use crate::{FemtoVGRenderer, GraphicsBackend, WindowSurface};

use wgpu_25 as wgpu;

pub struct WGPUBackend {
    instance: RefCell<Option<wgpu::Instance>>,
    device: RefCell<Option<wgpu::Device>>,
    queue: RefCell<Option<wgpu::Queue>>,
    surface_config: RefCell<Option<wgpu::SurfaceConfiguration>>,
    surface: RefCell<Option<wgpu::Surface<'static>>>,
}

pub struct WGPUWindowSurface {
    surface_texture: wgpu::SurfaceTexture,
}

impl WindowSurface<femtovg::renderer::WGPURenderer> for WGPUWindowSurface {
    fn render_surface(&self) -> &wgpu::Texture {
        &self.surface_texture.texture
    }
}

impl GraphicsBackend for WGPUBackend {
    type Renderer = femtovg::renderer::WGPURenderer;
    type WindowSurface = WGPUWindowSurface;
    const NAME: &'static str = "WGPU";

    fn new_suspended() -> Self {
        Self {
            instance: Default::default(),
            device: Default::default(),
            queue: Default::default(),
            surface_config: Default::default(),
            surface: Default::default(),
        }
    }

    fn clear_graphics_context(&self) {
        self.surface.borrow_mut().take();
        self.queue.borrow_mut().take();
        self.device.borrow_mut().take();
    }

    fn begin_surface_rendering(
        &self,
    ) -> Result<Self::WindowSurface, Box<dyn std::error::Error + Send + Sync>> {
        let frame = self
            .surface
            .borrow()
            .as_ref()
            .unwrap()
            .get_current_texture()
            .expect("unable to get next texture from swapchain");
        Ok(WGPUWindowSurface { surface_texture: frame })
    }

    fn submit_commands(&self, commands: <Self::Renderer as femtovg::Renderer>::CommandBuffer) {
        self.queue.borrow().as_ref().unwrap().submit(Some(commands));
    }

    fn present_surface(
        &self,
        surface: Self::WindowSurface,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        surface.surface_texture.present();
        Ok(())
    }

    #[cfg(feature = "unstable-wgpu-25")]
    fn with_graphics_api<R>(
        &self,
        callback: impl FnOnce(Option<i_slint_core::api::GraphicsAPI<'_>>) -> R,
    ) -> Result<R, i_slint_core::platform::PlatformError> {
        let instance = self.instance.borrow().clone();
        let device = self.device.borrow().clone();
        let queue = self.queue.borrow().clone();
        if let (Some(instance), Some(device), Some(queue)) = (instance, device, queue) {
            Ok(callback(Some(i_slint_core::graphics::create_graphics_api_wgpu_25(
                instance, device, queue,
            ))))
        } else {
            Ok(callback(None))
        }
    }

    #[cfg(not(feature = "unstable-wgpu-25"))]
    fn with_graphics_api<R>(
        &self,
        callback: impl FnOnce(Option<i_slint_core::api::GraphicsAPI<'_>>) -> R,
    ) -> Result<R, i_slint_core::platform::PlatformError> {
        Ok(callback(None))
    }

    fn resize(
        &self,
        width: std::num::NonZeroU32,
        height: std::num::NonZeroU32,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut surface_config = self.surface_config.borrow_mut();
        let Some(surface_config) = surface_config.as_mut() else {
            // When the backend dispatches a resize event while the renderer is suspended, ignore resize requests.
            return Ok(());
        };

        // Prefer FIFO modes over possible Mailbox setting for frame pacing and better energy efficiency.
        surface_config.present_mode = wgpu::PresentMode::AutoVsync;
        surface_config.width = width.get();
        surface_config.height = height.get();

        let mut device = self.device.borrow_mut();
        let device = device.as_mut().unwrap();

        self.surface.borrow_mut().as_mut().unwrap().configure(device, surface_config);
        Ok(())
    }
}

impl FemtoVGRenderer<WGPUBackend> {
    pub fn set_window_handle(
        &self,
        window_handle: Box<dyn wgpu::WindowHandle>,
        size: PhysicalWindowSize,
        requested_graphics_api: Option<RequestedGraphicsAPI>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (instance, adapter, device, queue, surface) = match requested_graphics_api {
            #[cfg(feature = "unstable-wgpu-25")]
            Some(RequestedGraphicsAPI::WGPU25(
                i_slint_core::graphics::wgpu_25::WGPUConfiguration::Manual {
                    instance,
                    adapter,
                    device,
                    queue,
                },
            )) => {
                let surface = instance.create_surface(window_handle).unwrap();
                (instance, adapter, device, queue, surface)
            }
            #[cfg(feature = "unstable-wgpu-25")]
            Some(RequestedGraphicsAPI::WGPU25(
                i_slint_core::graphics::wgpu_25::WGPUConfiguration::Automatic(wgpu25_settings),
            )) => {
                // wgpu uses async here, but the returned future is ready on first poll on all platforms except WASM,
                // which we don't support right now.
                let instance = poll_once(async {
                    wgpu::util::new_instance_with_webgpu_detection(&wgpu::InstanceDescriptor {
                        backends: wgpu25_settings.backends,
                        flags: wgpu25_settings.instance_flags,
                        backend_options: wgpu25_settings.backend_options,
                    })
                    .await
                })
                .expect("internal error: wgpu instance creation is not expected to be async");

                let surface = instance.create_surface(window_handle).unwrap();

                // wgpu uses async here, but the returned future is ready on first poll on all platforms except WASM,
                // which we don't support right now.
                let adapter = poll_once(async {
                    match wgpu::util::initialize_adapter_from_env(&instance, Some(&surface)) {
                        Ok(adapter) => Ok(adapter),
                        Err(_) => {
                            instance
                                .request_adapter(&wgpu::RequestAdapterOptions {
                                    power_preference: wgpu25_settings.power_preference,
                                    force_fallback_adapter: false,
                                    compatible_surface: Some(&surface),
                                })
                                .await
                        }
                    }
                    .expect("Failed to find an appropriate adapter")
                })
                .expect("internal error: wgpu adapter creation is not expected to be async");

                let (device, queue) = poll_once(async {
                    adapter
                        .request_device(&wgpu::DeviceDescriptor {
                            label: wgpu25_settings.device_label.as_deref(),
                            required_features: wgpu25_settings.device_required_features,
                            // Make sure we use the texture resolution limits from the adapter, so we can support images the size of the swapchain.
                            required_limits: wgpu25_settings
                                .device_required_limits
                                .using_resolution(adapter.limits()),
                            memory_hints: wgpu25_settings.device_memory_hints,
                            trace: wgpu::Trace::default(),
                        })
                        .await
                        .expect("Failed to create device")
                })
                .expect("internal error: wgpu device creation is not expected to be async");

                (instance, adapter, device, queue, surface)
            }
            None => {
                let backends = wgpu::Backends::from_env().unwrap_or_default();
                let dx12_shader_compiler = wgpu::Dx12Compiler::from_env().unwrap_or_default();
                let gles_minor_version = wgpu::Gles3MinorVersion::from_env().unwrap_or_default();

                // wgpu uses async here, but the returned future is ready on first poll on all platforms except WASM,
                // which we don't support right now.
                let instance = poll_once(async {
                    wgpu::util::new_instance_with_webgpu_detection(&wgpu::InstanceDescriptor {
                        backends,
                        flags: wgpu::InstanceFlags::from_build_config().with_env(),
                        backend_options: wgpu::BackendOptions {
                            dx12: wgpu::Dx12BackendOptions {
                                shader_compiler: dx12_shader_compiler,
                            },
                            gl: wgpu::GlBackendOptions {
                                gles_minor_version,
                                fence_behavior: wgpu::GlFenceBehavior::default(),
                            },
                            noop: wgpu::NoopBackendOptions::default(),
                        },
                    })
                    .await
                })
                .expect("internal error: wgpu instance creation is not expected to be async");

                let surface = instance.create_surface(window_handle).unwrap();

                // wgpu uses async here, but the returned future is ready on first poll on all platforms except WASM,
                // which we don't support right now.
                let adapter = poll_once(async {
                    wgpu::util::initialize_adapter_from_env_or_default(&instance, Some(&surface))
                        .await
                        .expect("Failed to find an appropriate adapter")
                })
                .expect("internal error: wgpu adapter creation is not expected to be async");

                let (device, queue) = poll_once(async {
                    adapter
                        .request_device(&wgpu::DeviceDescriptor {
                            label: None,
                            required_features: wgpu::Features::empty(),
                            // Make sure we use the texture resolution limits from the adapter, so we can support images the size of the swapchain.
                            required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                                .using_resolution(adapter.limits()),
                            memory_hints: wgpu::MemoryHints::MemoryUsage,
                            trace: wgpu::Trace::default(),
                        })
                        .await
                        .expect("Failed to create device")
                })
                .expect("internal error: wgpu device creation is not expected to be async");
                (instance, adapter, device, queue, surface)
            }
            Some(_) => {
                return Err(
                    "The FemtoVG WGPU renderer does not implement renderer selection by graphics API"
                        .into(),
                );
            }
        };

        let mut surface_config =
            surface.get_default_config(&adapter, size.width, size.height).unwrap();

        let swapchain_capabilities = surface.get_capabilities(&adapter);
        let swapchain_format = swapchain_capabilities
            .formats
            .iter()
            .find(|f| !f.is_srgb())
            .copied()
            .unwrap_or_else(|| swapchain_capabilities.formats[0]);
        surface_config.format = swapchain_format;
        surface.configure(&device, &surface_config);

        *self.graphics_backend.instance.borrow_mut() = Some(instance.clone());
        *self.graphics_backend.device.borrow_mut() = Some(device.clone());
        *self.graphics_backend.queue.borrow_mut() = Some(queue.clone());
        *self.graphics_backend.surface_config.borrow_mut() = Some(surface_config);
        *self.graphics_backend.surface.borrow_mut() = Some(surface);

        let wgpu_renderer = femtovg::renderer::WGPURenderer::new(device, queue);
        let femtovg_canvas = femtovg::Canvas::new_with_text_context(
            wgpu_renderer,
            crate::fonts::FONT_CACHE.with(|cache| cache.borrow().text_context.clone()),
        )
        .unwrap();

        let canvas = Rc::new(RefCell::new(femtovg_canvas));
        self.reset_canvas(canvas);
        Ok(())
    }
}

// Helper function to poll a future once. Remove once the suspension API uses async.
fn poll_once<F: std::future::Future>(future: F) -> Option<F::Output> {
    struct DummyWaker();
    impl std::task::Wake for DummyWaker {
        fn wake(self: std::sync::Arc<Self>) {}
    }

    let waker = std::sync::Arc::new(DummyWaker()).into();
    let mut ctx = std::task::Context::from_waker(&waker);

    let future = std::pin::pin!(future);

    match future.poll(&mut ctx) {
        std::task::Poll::Ready(result) => Some(result),
        std::task::Poll::Pending => None,
    }
}
