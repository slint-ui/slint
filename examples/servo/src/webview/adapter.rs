// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use std::cell::{Ref, RefCell, RefMut};
use std::rc::{Rc, Weak};

use servo::{Servo, WebView};
use smol::channel::{Receiver, Sender};

use slint::ComponentHandle;

use slint::wgpu_28::wgpu;

use crate::{MyApp, WebviewLogic, webview::rendering_context::ServoRenderingAdapter};

/// Upgrades a weak reference to `SlintServoAdapter` to a strong reference.
///
/// # Arguments
///
/// * `weak_ref` - Weak reference to upgrade
///
/// # Panics
///
/// Panics if the adapter has been dropped (weak reference cannot be upgraded).
pub fn upgrade_adapter(weak_ref: &Weak<SlintServoAdapter>) -> Rc<SlintServoAdapter> {
    weak_ref.upgrade().expect("Failed to upgrade SlintServoAdapter")
}

/// Bridge between Slint UI and Servo browser engine.
///
/// `SlintServoAdapter` manages the lifecycle and communication between the Slint UI
/// framework and the Servo browser engine. It holds references to both systems and
/// facilitates bidirectional data flow.
///
/// # Responsibilities
///
/// - **State Management**: Holds Servo and WebView instances
/// - **Event Communication**: Manages async channels for event loop waking
/// - **Rendering Coordination**: Bridges Servo's framebuffer to Slint's display
/// - **Resource Management**: Manages WGPU device and queue (non-Android)
///
/// # Thread Safety
///
/// This type uses `RefCell` for interior mutability and is designed to be used
/// within a single-threaded context (Slint's main thread). Access is coordinated
/// via `Rc` reference counting.
pub struct SlintServoAdapter {
    /// Channel sender to wake the event loop
    waker_sender: Sender<()>,
    /// Channel receiver for event loop wake signals
    waker_receiver: Receiver<()>,
    inner: RefCell<SlintServoAdapterInner>,
}

/// Internal state for `SlintServoAdapter`.
///
/// Holds the WebView instance, rendering adapter, and platform-specific
/// GPU resources. Wrapped in `RefCell` for interior mutability.
pub struct SlintServoAdapterInner {
    servo: Option<Servo>,
    webview: Option<WebView>,
    rendering_adapter: Option<Rc<Box<dyn ServoRenderingAdapter>>>,
    device: wgpu::Device,
    queue: wgpu::Queue,
}

impl SlintServoAdapter {
    pub fn new(
        waker_sender: Sender<()>,
        waker_receiver: Receiver<()>,
        device: wgpu::Device,
        queue: wgpu::Queue,
    ) -> Self {
        Self {
            waker_sender,
            waker_receiver,
            inner: RefCell::new(SlintServoAdapterInner {
                servo: None,
                webview: None,
                rendering_adapter: None,
                device: device,
                queue: queue,
            }),
        }
    }

    pub fn inner(&self) -> Ref<'_, SlintServoAdapterInner> {
        self.inner.borrow()
    }

    pub fn inner_mut(&self) -> RefMut<'_, SlintServoAdapterInner> {
        self.inner.borrow_mut()
    }

    pub fn waker_sender(&self) -> Sender<()> {
        self.waker_sender.clone()
    }

    pub fn waker_reciver(&self) -> Receiver<()> {
        self.waker_receiver.clone()
    }

    pub fn wgpu_device(&self) -> wgpu::Device {
        self.inner().device.clone()
    }

    pub fn wgpu_queue(&self) -> wgpu::Queue {
        self.inner().queue.clone()
    }

    pub fn servo(&self) -> Ref<'_, Servo> {
        Ref::map(self.inner(), |inner| inner.servo.as_ref().expect("Servo not initialized yet"))
    }

    pub fn webview(&self) -> WebView {
        self.inner().webview.as_ref().expect("Webview not initialized yet").clone()
    }

    pub fn set_inner(
        &self,
        servo: Servo,
        webview: WebView,
        rendering_adapter: Rc<Box<dyn ServoRenderingAdapter>>,
    ) {
        let mut inner = self.inner_mut();
        inner.servo = Some(servo);
        inner.webview = Some(webview);
        inner.rendering_adapter = Some(rendering_adapter);
    }

    /// Captures the current Servo framebuffer and updates the Slint UI with the rendered content.
    /// This bridges the rendering output from Servo to the Slint display surface.
    pub fn update_web_content_with_latest_frame(&self, app: &MyApp) {
        let inner = self.inner();
        let rendering_adapter = inner.rendering_adapter.as_ref().unwrap();

        // Convert framebuffer to Slint image format
        let slint_image = rendering_adapter.current_framebuffer_as_image();

        app.global::<WebviewLogic>().set_web_content(slint_image);
        app.window().request_redraw();
    }
}
