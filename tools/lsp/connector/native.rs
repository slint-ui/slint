// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::editor_preview;

use i_slint_live_preview::protocol::{LspToPreviewMessage, PreviewTarget};

pub struct EmbeddedLspToPreview {
    server_notifier: crate::ServerNotifier,
}

impl EmbeddedLspToPreview {
    pub fn new(server_notifier: crate::ServerNotifier) -> Self {
        Self { server_notifier }
    }
}

impl editor_preview::LspToPreview for EmbeddedLspToPreview {
    fn send(&self, message: &LspToPreviewMessage) {
        let _ = self.server_notifier.send_notification::<LspToPreviewMessage>(message.clone());
    }

    fn preview_target(&self) -> PreviewTarget {
        PreviewTarget::EmbeddedWasm
    }
}
