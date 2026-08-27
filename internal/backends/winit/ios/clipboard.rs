// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::clipboard::ClipboardProvider;
use objc2_foundation::NSString;
use objc2_ui_kit::UIPasteboard;

pub(crate) struct UiPasteboardClipboard;

impl ClipboardProvider for UiPasteboardClipboard {
    fn get_contents(
        &mut self,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync + 'static>> {
        let pasteboard = UIPasteboard::generalPasteboard();
        Ok(unsafe { pasteboard.string() }.map(|s| s.to_string()).unwrap_or_default())
    }

    fn set_contents(
        &mut self,
        data: String,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        let pasteboard = UIPasteboard::generalPasteboard();
        unsafe { pasteboard.setString(Some(&NSString::from_str(&data))) };
        Ok(())
    }
}
