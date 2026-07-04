// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/// Minimal clipboard abstraction implemented by the platform backends below.
pub trait ClipboardProvider {
    fn get_contents(
        &mut self,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync + 'static>>;
    fn set_contents(
        &mut self,
        contents: String,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>>;
}

/// The Default, and the selection clipboard
pub type ClipboardPair = (Box<dyn ClipboardProvider>, Box<dyn ClipboardProvider>);

pub fn select_clipboard(
    pair: &mut ClipboardPair,
    clipboard: i_slint_core::platform::Clipboard,
) -> Option<&mut dyn ClipboardProvider> {
    match clipboard {
        i_slint_core::platform::Clipboard::DefaultClipboard => Some(pair.0.as_mut()),
        i_slint_core::platform::Clipboard::SelectionClipboard => Some(pair.1.as_mut()),
        _ => None,
    }
}

/// Silent no-op fallback for platforms or configurations without clipboard support.
struct SilentClipboardContext;
impl ClipboardProvider for SilentClipboardContext {
    fn get_contents(
        &mut self,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync + 'static>> {
        Ok(Default::default())
    }

    fn set_contents(
        &mut self,
        _: String,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        Ok(())
    }
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
struct SystemClipboard(arboard::Clipboard);
#[cfg(any(target_os = "windows", target_os = "macos"))]
impl ClipboardProvider for SystemClipboard {
    fn get_contents(
        &mut self,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync + 'static>> {
        self.0.get_text().map_err(Into::into)
    }

    fn set_contents(
        &mut self,
        contents: String,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        self.0.set_text(contents).map_err(Into::into)
    }
}

#[cfg(all(
    unix,
    not(any(target_vendor = "apple", target_os = "android", target_os = "emscripten")),
    any(feature = "x11", feature = "wayland")
))]
struct SelectableClipboard {
    clipboard: arboard::Clipboard,
    kind: arboard::LinuxClipboardKind,
}
#[cfg(all(
    unix,
    not(any(target_vendor = "apple", target_os = "android", target_os = "emscripten")),
    any(feature = "x11", feature = "wayland")
))]
impl ClipboardProvider for SelectableClipboard {
    fn get_contents(
        &mut self,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync + 'static>> {
        use arboard::GetExtLinux;
        self.clipboard.get().clipboard(self.kind).text().map_err(Into::into)
    }

    fn set_contents(
        &mut self,
        contents: String,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        use arboard::SetExtLinux;
        self.clipboard.set().clipboard(self.kind).text(contents).map_err(Into::into)
    }
}

pub fn create_clipboard(
    _display_handle: &winit::raw_window_handle::DisplayHandle<'_>,
) -> ClipboardPair {
    cfg_if::cfg_if! {
        if #[cfg(all(
            unix,
            not(any(target_vendor = "apple", target_os = "android", target_os = "emscripten")),
            any(feature = "x11", feature = "wayland")
        ))] {
            // arboard selects Wayland (data-control protocol, with the "wayland" feature)
            // or X11 at runtime; on compositors without wlr-data-control it falls back to
            // the X11/XWayland clipboard, which such compositors keep in sync.
            let make = |kind| -> Box<dyn ClipboardProvider> {
                arboard::Clipboard::new().map_or(
                    Box::new(SilentClipboardContext) as Box<dyn ClipboardProvider>,
                    |clipboard| Box::new(SelectableClipboard { clipboard, kind }),
                )
            };
            (
                make(arboard::LinuxClipboardKind::Clipboard),
                make(arboard::LinuxClipboardKind::Primary),
            )
        } else if #[cfg(target_os = "ios")] {
            // iOS exposes a single general pasteboard; the selection clipboard is a no-op.
            (Box::new(crate::ios::UiPasteboardClipboard), Box::new(SilentClipboardContext))
        } else if #[cfg(any(target_os = "windows", target_os = "macos"))] {
            (
                arboard::Clipboard::new().map_or(
                    Box::new(SilentClipboardContext) as Box<dyn ClipboardProvider>,
                    |clipboard| Box::new(SystemClipboard(clipboard)) as Box<dyn ClipboardProvider>,
                ),
                Box::new(SilentClipboardContext),
            )
        } else {
            (Box::new(SilentClipboardContext), Box::new(SilentClipboardContext))
        }
    }
}
