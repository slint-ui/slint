// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use std::rc::Rc;

use servo::{WebView, WebViewDelegate};

use crate::{MyApp, adapter::SlintServoAdapter};

pub struct AppDelegate {
    pub state: Rc<SlintServoAdapter>,
    pub app: slint::Weak<MyApp>,
}

impl AppDelegate {
    pub fn new(state: Rc<SlintServoAdapter>, app: slint::Weak<MyApp>) -> Self {
        Self { state, app }
    }
}

impl WebViewDelegate for AppDelegate {
    /// Called by Servo when a new frame is ready to be displayed.
    /// Triggers painting and updates the Slint UI with the new frame.
    fn notify_new_frame_ready(&self, webview: WebView) {
        webview.paint();
        if let Some(app) = self.app.upgrade() {
            self.state.update_web_content_with_latest_frame(&app);
        }
    }
}
