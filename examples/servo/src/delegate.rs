// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use std::rc::Rc;

use servo::{WebView, WebViewDelegate};

use crate::adapter::SlintServoAdapter;

pub struct AppDelegate {
    pub state: Rc<SlintServoAdapter>,
}

impl AppDelegate {
    pub fn new(state: Rc<SlintServoAdapter>) -> Self {
        Self { state }
    }
}

impl WebViewDelegate for AppDelegate {
    /// Called by Servo when a new frame is ready to be displayed.
    /// Triggers painting and updates the Slint UI with the new frame.
    fn notify_new_frame_ready(&self, webview: WebView) {
        webview.paint();
        self.state.update_web_content_with_latest_frame();
    }
}
