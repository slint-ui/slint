use std::cell::{Ref, RefCell, RefMut};
use std::rc::{Rc, Weak};

use servo::{Servo, WebView};
use slint::ComponentHandle;
use smol::channel::{Receiver, Sender};

#[cfg(not(target_os = "android"))]
use slint::wgpu_27::wgpu;

use crate::{MyApp, WebviewLogic, rendering_context::ServoRenderingAdapter};

pub fn upgrade_adapter(weak_ref: &Weak<SlintServoAdapter>) -> Rc<SlintServoAdapter> {
    weak_ref
        .upgrade()
        .expect("Failed to upgrade SlintServoAdapter")
}

pub struct SlintServoAdapter {
    app: slint::Weak<MyApp>,
    waker_sender: Sender<()>,
    waker_receiver: Receiver<()>,
    pub servo: RefCell<Option<Servo>>,
    inner: RefCell<SlintServoAdapterInner>,
}

pub struct SlintServoAdapterInner {
    scale_factor: f32,
    webview: Option<WebView>,
    rendering_adapter: Option<Box<dyn ServoRenderingAdapter>>,
    #[cfg(not(target_os = "android"))]
    device: Option<wgpu::Device>,
    #[cfg(not(target_os = "android"))]
    queue: Option<wgpu::Queue>,
}

impl SlintServoAdapter {
    pub fn new(
        app: slint::Weak<MyApp>,
        waker_sender: Sender<()>,
        waker_receiver: Receiver<()>,
    ) -> Self {
        Self {
            app,
            waker_sender,
            waker_receiver,
            servo: RefCell::new(None),
            inner: RefCell::new(SlintServoAdapterInner {
                webview: None,
                scale_factor: 1.0,
                rendering_adapter: None,
                #[cfg(not(target_os = "android"))]
                device: None,
                #[cfg(not(target_os = "android"))]
                queue: None,
            }),
        }
    }

    pub fn inner(&self) -> Ref<'_, SlintServoAdapterInner> {
        self.inner.borrow()
    }

    pub fn inner_mut(&self) -> RefMut<'_, SlintServoAdapterInner> {
        self.inner.borrow_mut()
    }

    pub fn app(&self) -> MyApp {
        self.app.upgrade().expect("Failed to upgrade MyApp")
    }

    pub fn waker_sender(&self) -> Sender<()> {
        self.waker_sender.clone()
    }

    pub fn waker_reciver(&self) -> Receiver<()> {
        self.waker_receiver.clone()
    }

    pub fn scale_factor(&self) -> f32 {
        self.inner().scale_factor
    }

    #[cfg(not(target_os = "android"))]
    pub fn wgpu_device(&self) -> wgpu::Device {
        self.inner()
            .device
            .as_ref()
            .expect("Device not initialized yet")
            .clone()
    }

    #[cfg(not(target_os = "android"))]
    pub fn wgpu_queue(&self) -> wgpu::Queue {
        self.inner()
            .queue
            .as_ref()
            .expect("Queue not initialized yet")
            .clone()
    }

    pub fn webview(&self) -> WebView {
        self.inner()
            .webview
            .as_ref()
            .expect("Webview not initialized yet")
            .clone()
    }

    pub fn set_inner(
        &self,
        servo: Servo,
        webview: WebView,
        scale_factor: f32,
        rendering_adapter: Box<dyn ServoRenderingAdapter>,
    ) {
        *self.servo.borrow_mut() = Some(servo);
        let mut inner = self.inner_mut();
        inner.webview = Some(webview);
        inner.scale_factor = scale_factor;
        inner.rendering_adapter = Some(rendering_adapter);
    }

    #[cfg(not(target_os = "android"))]
    pub fn set_wgpu_device_queue(&self, device: &wgpu::Device, queue: &wgpu::Queue) {
        let mut inner = self.inner_mut();
        inner.device = Some(device.clone());
        inner.queue = Some(queue.clone());
    }

    pub fn update_web_content_with_latest_frame(&self) {
        let inner = self.inner();
        let rendering_adapter = inner.rendering_adapter.as_ref().unwrap();

        let slint_image = rendering_adapter.current_framebuffer_as_image();

        let app = self
            .app
            .upgrade()
            .expect("Application reference is no longer valid - UI may have been destroyed");

        app.global::<WebviewLogic>().set_web_content(slint_image);
        app.window().request_redraw();
    }
}
