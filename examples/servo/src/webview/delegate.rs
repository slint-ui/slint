// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use slint::ComponentHandle;
use std::rc::Rc;

use crate::{MyApp, WebviewLogic, webview::SlintServoAdapter};
use servo::{EmbedderControl, EmbedderControlId, WebView, WebViewDelegate};

/// Servo delegate for handling browser engine callbacks.
///
/// `AppDelegate` implements Servo's `WebViewDelegate` trait to receive notifications
/// about rendering events. It acts as a bridge, forwarding Servo's frame updates to
/// the Slint UI for display.
///
/// # Responsibilities
///
/// - Receives frame-ready notifications from Servo
/// - Triggers frame painting in Servo
/// - Updates the Slint UI with the latest rendered content
///
/// # Lifecycle
///
/// The delegate holds a weak reference to the Slint app to avoid circular references.
/// If the app is dropped, frame updates are silently ignored.
pub struct AppDelegate {
    /// Weak reference to the Slint application
    app: slint::Weak<MyApp>,
    /// Reference to the Slint-Servo adapter for state access
    adapter: Rc<SlintServoAdapter>,
}

impl AppDelegate {
    /// Creates a new delegate instance.
    ///
    /// # Arguments
    ///
    /// * `app` - Weak reference to the Slint application
    /// * `adapter` - Reference to the Slint-Servo adapter
    pub fn new(app: &MyApp, adapter: Rc<SlintServoAdapter>) -> Self {
        Self { app: app.as_weak(), adapter }
    }
}

impl WebViewDelegate for AppDelegate {
    /// Called by Servo when a new frame is ready to be displayed.
    /// Triggers painting and updates the Slint UI with the new frame.
    fn notify_new_frame_ready(&self, webview: WebView) {
        webview.paint();
        if let Some(app) = self.app.upgrade() {
            self.adapter.update_web_content_with_latest_frame(&app);
        }
    }

    fn notify_url_changed(&self, _webview: WebView, url: url::Url) {
        if let Some(app) = self.app.upgrade() {
            app.global::<WebviewLogic>().set_current_url(url.to_string().into());
        }
    }

    fn show_embedder_control(&self, _webview: WebView, embedder_control: EmbedderControl) {
        match embedder_control {
            EmbedderControl::InputMethod(input_method_control) => {
                if input_method_control.allow_virtual_keyboard() {
                    if let Some(app) = self.app.upgrade() {
                        app.window().show_ime();
                    }
                }
            }
            _ => {}
        }
    }

    fn hide_embedder_control(&self, _webview: WebView, _control_id: EmbedderControlId) {
        if let Some(app) = self.app.upgrade() {
            app.window().hide_ime();
        }
    }
}
