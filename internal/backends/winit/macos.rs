// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company , info@kdab.com, author Robin Cramer <robin.cramer@kdab.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! AppKit bits the backend reaches for directly, on top of what winit exposes.

/// Returns the `NSView` backing a winit window.
///
/// `None` when the window isn't AppKit's, or when its handle isn't reachable.
/// The handle is unreachable off the main thread.
pub(crate) fn ns_view(window: &winit::window::Window) -> Option<&objc2_app_kit::NSView> {
    use raw_window_handle::HasWindowHandle;

    let raw_window_handle::RawWindowHandle::AppKit(raw_window_handle::AppKitWindowHandle {
        ns_view,
        ..
    }) = window.window_handle().ok()?.as_raw()
    else {
        return None;
    };
    // SAFETY: winit hands out the content view of the `NSWindow` it owns, so `window` keeps
    // the view alive for the borrow. The main-thread check sits in winit's `window_handle()`,
    // which is what stops a caller from reaching an `NSView` off the main thread.
    Some(unsafe { ns_view.cast().as_ref() })
}
