// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use std::rc::Rc;

use smol::channel;
use url::Url;
use winit::dpi::PhysicalSize;

use euclid::Size2D;

use slint::{ComponentHandle, SharedString, language::ColorScheme};

use servo::{Servo, ServoBuilder, Theme, WebViewBuilder, webrender_api::units::DevicePixel};

use crate::{
    MyApp, Palette, WebviewLogic,
    webview::{AppDelegate, ServoRenderingAdapter, SlintServoAdapter, Waker, WebViewEvents},
};

/// A web browser component powered by the Servo engine.
///
/// `WebView` provides a high-level interface for embedding a full-featured web browser
/// into Slint applications. It handles the initialization and lifecycle management of
/// the Servo browser engine, rendering pipeline, and event handling.
///
/// # Architecture
///
/// The WebView orchestrates several subsystems:
/// - **Rendering**: Platform-specific GPU or software rendering
/// - **Event Loop**: Async Servo event processing
/// - **UI Integration**: Bidirectional communication with Slint
/// - **Input Handling**: Mouse, touch, and keyboard events
///
/// # Platform Differences
///
/// - **Non-Android platforms**: Uses GPU-accelerated rendering via WGPU
/// - **Android**: Falls back to software rendering
pub struct WebView {}

impl WebView {
    /// Creates and initializes a new WebView instance.
    ///
    /// This method sets up the complete web browser infrastructure including:
    /// - Servo browser engine initialization
    /// - Rendering context (GPU or software)
    /// - Event loop for async operations
    /// - UI event handlers for user interactions
    ///
    /// # Arguments
    ///
    /// * `app` - The Slint application instance to integrate with
    /// * `initial_url` - The URL to load when the browser starts
    /// * `device` - WGPU device for GPU rendering (non-Android only)
    /// * `queue` - WGPU command queue for GPU operations (non-Android only)
    ///
    /// # Panics
    ///
    /// Panics if:
    /// - The initial URL cannot be parsed
    /// - GPU rendering context creation fails (on non-Android platforms)
    /// - Servo event loop task cannot be spawned
    pub fn new(
        app: MyApp,
        initial_url: SharedString,
        device: slint::wgpu_28::wgpu::Device,
        queue: slint::wgpu_28::wgpu::Queue,
    ) {
        let (waker_sender, waker_receiver) = channel::unbounded::<()>();

        let adapter = Rc::new(SlintServoAdapter::new(
            waker_sender.clone(),
            waker_receiver.clone(),
            device,
            queue,
        ));

        let state_weak = Rc::downgrade(&adapter);
        let state = super::adapter::upgrade_adapter(&state_weak);

        let (rendering_adapter, physical_size) = Self::init_rendering_adapter(&app, state.clone());

        let servo = Self::init_servo_builder(state.clone(), rendering_adapter.clone());

        Self::init_webview(
            &app,
            physical_size,
            initial_url,
            state.clone(),
            servo,
            rendering_adapter,
        );

        Self::spin_servo_event_loop(adapter.clone());

        WebViewEvents::new(&app, adapter.clone());
    }

    /// Initializes the rendering adapter based on platform capabilities.
    ///
    /// Creates either a GPU-accelerated or software rendering context depending on
    /// the platform and availability. The viewport size is extracted from the Slint UI.
    ///
    /// # Returns
    ///
    /// A tuple containing:
    /// - The rendering adapter (GPU or software)
    /// - The physical size of the viewport in pixels
    fn init_rendering_adapter(
        app: &MyApp,
        adapter: Rc<SlintServoAdapter>,
    ) -> (Rc<Box<dyn ServoRenderingAdapter>>, PhysicalSize<u32>) {
        let width = app.global::<WebviewLogic>().get_viewport_width();
        let height = app.global::<WebviewLogic>().get_viewport_height();

        let size: Size2D<f32, DevicePixel> = Size2D::new(width, height);
        let physical_size = PhysicalSize::new(size.width as u32, size.height as u32);

        let rendering_adapter = super::rendering_context::try_create_gpu_context(
            adapter.wgpu_device(),
            adapter.wgpu_queue(),
            physical_size,
        )
        .unwrap();

        let rendering_adapter_rc = Rc::new(rendering_adapter);

        (rendering_adapter_rc, physical_size)
    }

    /// Initializes and builds the Servo browser engine instance.
    ///
    /// Configures Servo with the rendering context and event loop waker for
    /// async operation integration.
    ///
    /// # Arguments
    ///
    /// * `adapter` - The Slint-Servo adapter for state management
    /// * `rendering_adapter` - The rendering backend to use
    ///
    /// # Returns
    ///
    /// A configured Servo instance ready for use
    fn init_servo_builder(
        adapter: Rc<SlintServoAdapter>,
        rendering_adapter: Rc<Box<dyn ServoRenderingAdapter>>,
    ) -> Servo {
        let waker = Waker::new(adapter.waker_sender());
        let event_loop_waker = Box::new(waker);
        let rendering_context = rendering_adapter.get_rendering_context();

        ServoBuilder::new(rendering_context).event_loop_waker(event_loop_waker).build()
    }

    /// Initializes the Servo WebView with the initial URL and configuration.
    ///
    /// Sets up the WebView with:
    /// - Initial URL to load
    /// - Viewport size
    /// - Delegate for frame update callbacks
    /// - Theme (light/dark mode) based on Slint settings
    ///
    /// # Arguments
    ///
    /// * `app` - The Slint application instance
    /// * `physical_size` - Initial viewport dimensions
    /// * `initial_url` - URL to navigate to on startup
    /// * `adapter` - The Slint-Servo adapter
    /// * `servo` - The Servo engine instance
    /// * `rendering_adapter` - The rendering backend
    fn init_webview(
        app: &MyApp,
        physical_size: PhysicalSize<u32>,
        initial_url: SharedString,
        adapter: Rc<SlintServoAdapter>,
        servo: Servo,
        rendering_adapter: Rc<Box<dyn ServoRenderingAdapter>>,
    ) {
        app.global::<WebviewLogic>().set_current_url(initial_url.clone());

        let url = Url::parse(&initial_url).expect("Failed to parse url");

        let delegate = Rc::new(AppDelegate::new(app, adapter.clone()));

        let webview =
            WebViewBuilder::new(&servo).url(url).size(physical_size).delegate(delegate).build();

        webview.show(true);

        let color_scheme = app.global::<Palette>().get_color_scheme();
        let theme = if color_scheme == ColorScheme::Dark { Theme::Dark } else { Theme::Light };

        webview.notify_theme_change(theme);

        adapter.set_inner(servo, webview, rendering_adapter);
    }

    /// Spawns the async event loop for Servo operations.
    ///
    /// Creates a background task that continuously processes Servo events.
    /// The loop runs until the adapter is dropped (weak reference becomes invalid).
    ///
    /// # Arguments
    ///
    /// * `state` - The Slint-Servo adapter containing the event channel
    ///
    /// # Panics
    ///
    /// Panics if the async task cannot be spawned
    fn spin_servo_event_loop(state: Rc<SlintServoAdapter>) {
        let state_weak = Rc::downgrade(&state);

        slint::spawn_local({
            async move {
                loop {
                    let state = match state_weak.upgrade() {
                        Some(s) => s,
                        None => break,
                    };

                    let _ = state.waker_reciver().recv().await;
                    state.servo().spin_event_loop();
                }
            }
        })
        .expect("Failed to spawn servo event loop task");
    }
}
