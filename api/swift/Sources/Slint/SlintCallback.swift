// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

@preconcurrency import SlintCBridge

// MARK: - C callback thunks (non-generic, file-scope)

/// Dispatches a callback invocation to the Swift handler stored in `userData`.
private let callbackHandlerInvoke: @convention(c) (
    UnsafeMutableRawPointer?, UnsafeRawPointer?, UnsafeMutableRawPointer?
) -> Void = { userData, _, _ in
    guard let userData else { return }
    Unmanaged<CallbackHandlerBox>.fromOpaque(userData)
        .takeUnretainedValue()
        .invoke()
}

/// Releases the `CallbackHandlerBox` retained when the handler was set.
private let callbackHandlerDrop: @convention(c) (UnsafeMutableRawPointer?) -> Void = { userData in
    guard let userData else { return }
    Unmanaged<CallbackHandlerBox>.fromOpaque(userData).release()
}

/// Type-erased container for a callback handler closure.
private final class CallbackHandlerBox {
    let invoke: () -> Void
    init(_ invoke: @escaping () -> Void) { self.invoke = invoke }
}

// MARK: - SlintCallback

/// A Slint callback with no arguments and no return value.
///
/// `SlintCallback` wraps Slint's callback FFI, bridging to Swift closures. In a Slint UI,
/// callbacks are used to signal events such as button clicks. Phase 2 supports
/// parameter-less callbacks; typed arguments are introduced with the code generator
/// in Phase 3.
///
/// Example usage:
/// ```swift
/// let clicked = SlintCallback()
/// clicked.setHandler { print("button clicked") }
/// clicked.invoke()   // prints "button clicked"
/// ```
public final class SlintCallback {
    private var handle: SlintCallbackOpaque

    /// Creates a new, unhandled callback.
    public init() {
        handle = SlintCallbackOpaque(_0: nil, _1: nil)
        slint_callback_init(&handle)
    }

    deinit {
        slint_callback_drop(&handle)
    }

    /// Registers a Swift closure as the callback handler. Replaces any previously set handler.
    public func setHandler(_ handler: @escaping () -> Void) {
        let box = CallbackHandlerBox(handler)
        let ptr = Unmanaged.passRetained(box).toOpaque()
        slint_callback_set_handler(&handle, callbackHandlerInvoke, ptr, callbackHandlerDrop)
    }

    /// Invokes the callback, calling the registered handler (if any).
    public func invoke() {
        // The Rust FFI dereferences arg and ret; we pass stable dummy bytes so the pointers are
        // non-null. The callback ignores the values (Void args/return).
        var argDummy: UInt8 = 0
        var retDummy: UInt8 = 0
        slint_callback_call(&handle, &argDummy, &retDummy)
    }
}
