// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use std::rc::Rc;

use super::adapter::SlintServoAdapter;
use crate::MyApp;
use servo::{WebView, WebViewDelegate};

pub struct AppDelegate {
    pub app: slint::Weak<MyApp>,
    pub adapter: Rc<SlintServoAdapter>,
}

impl AppDelegate {
    pub fn new(app: slint::Weak<MyApp>, adapter: Rc<SlintServoAdapter>) -> Self {
        Self { app, adapter }
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
}
