// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! macOS unified title bar support for the visual editor.

use slint::winit_031::winit;
use winit::platform::macos::WindowAttributesMacOS;
use winit::window::WindowAttributes;

use crate::preview::ui::EditorUi;

/// Window-attributes hook that makes the content view fill the whole window
/// underneath a transparent, title-less title bar.
///
/// `setup` applies the same three settings to the live `NSWindow`, but this
/// hook is still needed: it runs at window-creation time, so the window is
/// already correct on the first frame. Without it, the window briefly appears
/// with a normal opaque title bar before the async `setup` runs — a visible
/// flash on startup.
#[allow(dead_code)]
pub fn apply_unified_titlebar(attributes: WindowAttributes) -> WindowAttributes {
    attributes.with_platform_attributes(Box::new(
        WindowAttributesMacOS::default()
            .with_fullsize_content_view(true)
            .with_titlebar_transparent(true)
            .with_title_hidden(true),
    ))
}

/// Configures the unified title bar once the winit window exists.
pub fn setup(editor: slint::Weak<EditorUi>) {
    use objc2::{MainThreadMarker, MainThreadOnly};
    use objc2_app_kit::{
        NSToolbar, NSView, NSWindowStyleMask, NSWindowTitleVisibility, NSWindowToolbarStyle,
    };
    use objc2_foundation::NSString;
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use slint::ComponentHandle;
    use slint::winit_031::WinitWindowAccessor;

    slint::spawn_local(async move {
        let editor = editor.upgrade()?;
        let Ok(winit_window) = editor.window().winit_window().await else {
            return None;
        };
        let RawWindowHandle::AppKit(handle) = winit_window.window_handle().ok()?.as_raw() else {
            return None;
        };
        // SAFETY: `ns_view` is valid for the lifetime of the winit window, and this
        // is only ever called on the main (UI) thread.
        let ns_view: &NSView = unsafe { handle.ns_view.cast().as_ref() };
        let ns_window = ns_view.window()?;

        let mask =
            NSWindowStyleMask(ns_window.styleMask().0 | NSWindowStyleMask::FullSizeContentView.0);
        ns_window.setStyleMask(mask);
        ns_window.setTitlebarAppearsTransparent(true);
        ns_window.setTitleVisibility(NSWindowTitleVisibility::Hidden);
        ns_window.setMovable(false);

        // Give the window an empty unified toolbar. macOS draws "has toolbar"
        // chrome for such windows, which includes the larger rounded corners.
        let mtm = MainThreadMarker::new()?;
        let identifier = NSString::from_str("SlintEditorToolbar");
        let toolbar = NSToolbar::initWithIdentifier(NSToolbar::alloc(mtm), &identifier);
        ns_window.setToolbarStyle(NSWindowToolbarStyle::Unified);
        ns_window.setToolbar(Some(&toolbar));

        Some(())
    })
    .ok();
}
