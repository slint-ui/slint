// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

@preconcurrency import SlintCBridge

/// A Slint window that can display UI content.
///
/// `SlintWindow` wraps the Slint window adapter, which manages the native
/// window lifecycle. All window operations must be performed on the main thread.
@MainActor
public final class SlintWindow {
    var handle: SlintWindowAdapterRcOpaque

    /// Creates a new window.
    ///
    /// This will create a native window adapter using the current backend.
    /// Make sure a backend is initialized (via `SlintEventLoop.ensureBackend()`)
    /// before creating a window.
    public init() {
        handle = SlintWindowAdapterRcOpaque(_0: nil, _1: nil)
        slint_windowrc_init(&handle)
    }

    deinit {
        slint_windowrc_drop(&handle)
    }

    /// Shows the window, making it visible on screen.
    public func show() {
        slint_windowrc_show(&handle)
    }

    /// Hides the window.
    public func hide() {
        slint_windowrc_hide(&handle)
    }

    /// Whether the window is currently visible.
    public var isVisible: Bool {
        slint_windowrc_is_visible(&handle)
    }

    /// The window size in physical pixels.
    public var size: (width: UInt32, height: UInt32) {
        let s = slint_windowrc_size(&handle)
        return (width: s.width, height: s.height)
    }

    /// Sets the window size in physical pixels.
    public func setSize(width: UInt32, height: UInt32) {
        var s = SlintIntSize(width: width, height: height)
        slint_windowrc_set_physical_size(&handle, &s)
    }

    /// Sets the window size in logical pixels.
    public func setLogicalSize(width: Float, height: Float) {
        var s = SlintSizeF32(width: width, height: height)
        slint_windowrc_set_logical_size(&handle, &s)
    }

    /// The window position in physical pixels.
    public var position: (x: Int32, y: Int32) {
        var pos = SlintPoint2DI32(x: 0, y: 0)
        slint_windowrc_position(&handle, &pos)
        return (x: pos.x, y: pos.y)
    }

    /// Sets the window position in physical pixels.
    public func setPosition(x: Int32, y: Int32) {
        var pos = SlintPoint2DI32(x: x, y: y)
        slint_windowrc_set_physical_position(&handle, &pos)
    }

    /// Sets the window position in logical pixels.
    public func setLogicalPosition(x: Float, y: Float) {
        var pos = SlintPoint2DF32(x: x, y: y)
        slint_windowrc_set_logical_position(&handle, &pos)
    }

    /// Whether the window is in fullscreen mode.
    public var isFullscreen: Bool {
        get { slint_windowrc_is_fullscreen(&handle) }
        set { slint_windowrc_set_fullscreen(&handle, newValue) }
    }

    /// Whether the window is minimized.
    public var isMinimized: Bool {
        get { slint_windowrc_is_minimized(&handle) }
        set { slint_windowrc_set_minimized(&handle, newValue) }
    }

    /// Whether the window is maximized.
    public var isMaximized: Bool {
        get { slint_windowrc_is_maximized(&handle) }
        set { slint_windowrc_set_maximized(&handle, newValue) }
    }

    /// The window's scale factor.
    public var scaleFactor: Float {
        slint_windowrc_get_scale_factor(&handle)
    }

    /// Requests that the window be redrawn.
    public func requestRedraw() {
        slint_windowrc_request_redraw(&handle)
    }
}
