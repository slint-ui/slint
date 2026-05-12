// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

@preconcurrency import SlintCBridge

/// The pointer button type for pointer events.
public enum SlintPointerButton: UInt32, Sendable {
    case other = 0
    case left = 1
    case right = 2
    case middle = 3
    case back = 4
    case forward = 5
}

/// Protocol for custom Slint window adapters.
///
/// Implement this protocol to control how Slint renders into your own view.
/// The renderer handle must be provided at creation time (e.g. from a
/// `SlintSoftwareRenderer`).
///
/// All methods are called on the main thread by the Slint runtime.
public protocol SlintWindowAdapterProtocol: AnyObject {
    /// The renderer reference handle. Must outlive this adapter.
    var rendererHandle: SlintRendererRefOpaque { get }

    /// Called when the window should be shown or hidden.
    func setVisible(_ visible: Bool)

    /// Called when Slint needs the window to be redrawn.
    func requestRedraw()

    /// Returns the current window size in physical pixels.
    func size() -> (width: UInt32, height: UInt32)

    /// Called when Slint wants to resize the window (physical pixels).
    func setSize(width: UInt32, height: UInt32)

    /// Returns the current window position in physical pixels, or nil if unknown.
    func position() -> (x: Int32, y: Int32)?

    /// Called when Slint wants to move the window (physical pixels).
    func setPosition(x: Int32, y: Int32)

    /// Called when window properties (title, fullscreen, etc.) change.
    /// The default implementation does nothing.
    func updateWindowProperties(title: String, isFullscreen: Bool, isMinimized: Bool, isMaximized: Bool)
}

// Default implementation for optional methods
public extension SlintWindowAdapterProtocol {
    func position() -> (x: Int32, y: Int32)? { return nil }
    func setPosition(x: Int32, y: Int32) {}
    func updateWindowProperties(title: String, isFullscreen: Bool, isMinimized: Bool, isMaximized: Bool) {}
}

// MARK: - Internal: Write the adapter into an opaque slot

extension SlintWindowAdapterProtocol {
    /// Writes this adapter as a `Rc<dyn WindowAdapter>` into the target slot.
    func writeInto(target: UnsafeMutablePointer<SlintWindowAdapterRcOpaque>) {
        let box_ = WindowAdapterBox(self)
        let context = Unmanaged.passRetained(box_).toOpaque()

        slint_swift_window_adapter_new(
            context,
            windowAdapterDrop,
            windowAdapterSetVisible,
            windowAdapterRequestRedraw,
            windowAdapterSize,
            windowAdapterSetSize,
            windowAdapterPosition,
            windowAdapterSetPosition,
            windowAdapterUpdateWindowProperties,
            self.rendererHandle,
            target
        )
    }
}

// MARK: - Window adapter handle for dispatching events

/// A handle to the underlying Slint window, usable for dispatching events.
///
/// Create by passing a `SlintWindowAdapterProtocol` implementation. The handle
/// holds the underlying `Rc<dyn WindowAdapter>` alive. Use this to dispatch
/// pointer, keyboard, and window lifecycle events.
@MainActor
public final class SlintWindowAdapterHandle {
    var handle: SlintWindowAdapterRcOpaque

    /// Creates a handle by creating a window adapter from a protocol implementation.
    public init(adapter: some SlintWindowAdapterProtocol) {
        handle = SlintWindowAdapterRcOpaque(_0: nil, _1: nil)
        adapter.writeInto(target: &handle)
    }

    deinit {
        slint_windowrc_drop(&handle)
    }

    /// Whether the window has currently running animations.
    public var hasActiveAnimations: Bool {
        slint_swift_window_has_active_animations(&handle)
    }

    // MARK: - Event dispatch

    /// Dispatches a pointer-pressed event.
    public func dispatchPointerPressed(x: Float, y: Float, button: SlintPointerButton = .left) {
        slint_swift_dispatch_pointer_pressed(&handle, x, y, button.rawValue)
    }

    /// Dispatches a pointer-released event.
    public func dispatchPointerReleased(x: Float, y: Float, button: SlintPointerButton = .left) {
        slint_swift_dispatch_pointer_released(&handle, x, y, button.rawValue)
    }

    /// Dispatches a pointer-moved event.
    public func dispatchPointerMoved(x: Float, y: Float) {
        slint_swift_dispatch_pointer_moved(&handle, x, y)
    }

    /// Dispatches a pointer-scrolled event.
    public func dispatchPointerScrolled(x: Float, y: Float, deltaX: Float, deltaY: Float) {
        slint_swift_dispatch_pointer_scrolled(&handle, x, y, deltaX, deltaY)
    }

    /// Dispatches a pointer-exited event.
    public func dispatchPointerExited() {
        slint_swift_dispatch_pointer_exited(&handle)
    }

    /// Dispatches a key-pressed event.
    public func dispatchKeyPressed(text: String) {
        withSharedString(text) { str in
            slint_swift_dispatch_key_pressed(&handle, &str)
        }
    }

    /// Dispatches a key-press-repeated event.
    public func dispatchKeyPressRepeated(text: String) {
        withSharedString(text) { str in
            slint_swift_dispatch_key_press_repeated(&handle, &str)
        }
    }

    /// Dispatches a key-released event.
    public func dispatchKeyReleased(text: String) {
        withSharedString(text) { str in
            slint_swift_dispatch_key_released(&handle, &str)
        }
    }

    /// Dispatches a scale-factor-changed event.
    public func dispatchScaleFactorChanged(_ scaleFactor: Float) {
        slint_swift_dispatch_scale_factor_changed(&handle, scaleFactor)
    }

    /// Dispatches a resized event with logical size.
    public func dispatchResized(width: Float, height: Float) {
        slint_swift_dispatch_resized(&handle, width, height)
    }

    /// Dispatches a close-requested event.
    public func dispatchCloseRequested() {
        slint_swift_dispatch_close_requested(&handle)
    }

    /// Dispatches a window-active-changed event.
    public func dispatchWindowActiveChanged(_ active: Bool) {
        slint_swift_dispatch_window_active_changed(&handle, active)
    }

    // MARK: - Window operations (delegated to underlying handle)

    /// Shows the window.
    public func show() {
        slint_windowrc_show(&handle)
    }

    /// Hides the window.
    public func hide() {
        slint_windowrc_hide(&handle)
    }

    /// Whether the window is visible.
    public var isVisible: Bool {
        slint_windowrc_is_visible(&handle)
    }

    /// Requests a redraw of the window.
    public func requestRedraw() {
        slint_windowrc_request_redraw(&handle)
    }

    /// The scale factor.
    public var scaleFactor: Float {
        slint_windowrc_get_scale_factor(&handle)
    }
}

// MARK: - Shared string helper

private func withSharedString<R>(_ string: String, _ body: (inout SlintSharedStringOpaque) -> R) -> R {
    var str = SlintSharedStringOpaque(_0: nil)
    string.withCString { ptr in
        slint_shared_string_from_bytes(&str, ptr, UInt(string.utf8.count))
    }
    defer { slint_shared_string_drop(&str) }
    return body(&str)
}

// MARK: - C callback functions for WindowAdapter

private final class WindowAdapterBox: @unchecked Sendable {
    let adapter: any SlintWindowAdapterProtocol
    init(_ adapter: some SlintWindowAdapterProtocol) {
        self.adapter = adapter
    }
}

nonisolated(unsafe) private let windowAdapterDrop: @convention(c) (
    UnsafeMutableRawPointer?
) -> Void = { ptr in
    guard let ptr = ptr else { return }
    Unmanaged<WindowAdapterBox>.fromOpaque(ptr).release()
}

nonisolated(unsafe) private let windowAdapterSetVisible: @convention(c) (
    UnsafeMutableRawPointer?, Bool
) -> Void = { ptr, visible in
    guard let ptr = ptr else { return }
    let box_ = Unmanaged<WindowAdapterBox>.fromOpaque(ptr).takeUnretainedValue()
    box_.adapter.setVisible(visible)
}

nonisolated(unsafe) private let windowAdapterRequestRedraw: @convention(c) (
    UnsafeMutableRawPointer?
) -> Void = { ptr in
    guard let ptr = ptr else { return }
    let box_ = Unmanaged<WindowAdapterBox>.fromOpaque(ptr).takeUnretainedValue()
    box_.adapter.requestRedraw()
}

nonisolated(unsafe) private let windowAdapterSize: @convention(c) (
    UnsafeMutableRawPointer?, UnsafeMutablePointer<UInt32>?, UnsafeMutablePointer<UInt32>?
) -> Void = { ptr, wOut, hOut in
    guard let ptr = ptr, let wOut = wOut, let hOut = hOut else { return }
    let box_ = Unmanaged<WindowAdapterBox>.fromOpaque(ptr).takeUnretainedValue()
    let (w, h) = box_.adapter.size()
    wOut.pointee = w
    hOut.pointee = h
}

nonisolated(unsafe) private let windowAdapterSetSize: @convention(c) (
    UnsafeMutableRawPointer?, UInt32, UInt32
) -> Void = { ptr, w, h in
    guard let ptr = ptr else { return }
    let box_ = Unmanaged<WindowAdapterBox>.fromOpaque(ptr).takeUnretainedValue()
    box_.adapter.setSize(width: w, height: h)
}

nonisolated(unsafe) private let windowAdapterPosition: @convention(c) (
    UnsafeMutableRawPointer?, UnsafeMutablePointer<Int32>?, UnsafeMutablePointer<Int32>?
) -> Bool = { ptr, xOut, yOut in
    guard let ptr = ptr, let xOut = xOut, let yOut = yOut else { return false }
    let box_ = Unmanaged<WindowAdapterBox>.fromOpaque(ptr).takeUnretainedValue()
    if let (x, y) = box_.adapter.position() {
        xOut.pointee = x
        yOut.pointee = y
        return true
    }
    return false
}

nonisolated(unsafe) private let windowAdapterSetPosition: @convention(c) (
    UnsafeMutableRawPointer?, Int32, Int32
) -> Void = { ptr, x, y in
    guard let ptr = ptr else { return }
    let box_ = Unmanaged<WindowAdapterBox>.fromOpaque(ptr).takeUnretainedValue()
    box_.adapter.setPosition(x: x, y: y)
}

nonisolated(unsafe) private let windowAdapterUpdateWindowProperties: @convention(c) (
    UnsafeMutableRawPointer?,
    UnsafePointer<SlintSharedStringOpaque>?,
    Bool, Bool, Bool
) -> Void = { ptr, titlePtr, fullscreen, minimized, maximized in
    guard let ptr = ptr else { return }
    let box_ = Unmanaged<WindowAdapterBox>.fromOpaque(ptr).takeUnretainedValue()
    let title: String
    if let titlePtr = titlePtr {
        title = String(cString: slint_shared_string_bytes(titlePtr))
    } else {
        title = ""
    }
    box_.adapter.updateWindowProperties(
        title: title, isFullscreen: fullscreen,
        isMinimized: minimized, isMaximized: maximized
    )
}
