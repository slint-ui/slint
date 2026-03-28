// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

@preconcurrency import SlintCBridge

/// A box holding a Swift closure for event loop posting.
/// Marked as @unchecked Sendable because the closure is only ever invoked
/// on the main thread by the Slint event loop.
private final class EventCallbackBox: @unchecked Sendable {
    let closure: () -> Void
    init(_ closure: @escaping () -> Void) {
        self.closure = closure
    }
}

/// The C function pointer that invokes the Swift closure stored in user_data.
private let eventCallbackInvoke: @convention(c) (UnsafeMutableRawPointer?) -> Void = { ptr in
    guard let ptr = ptr else { return }
    let box_ = Unmanaged<EventCallbackBox>.fromOpaque(ptr).takeUnretainedValue()
    box_.closure()
}

/// The C function pointer that releases the Swift closure stored in user_data.
private let eventCallbackDrop: @convention(c) (UnsafeMutableRawPointer?) -> Void = { ptr in
    guard let ptr = ptr else { return }
    Unmanaged<EventCallbackBox>.fromOpaque(ptr).release()
}

/// The Slint event loop.
///
/// `SlintEventLoop` provides static methods to control the Slint event loop.
/// The event loop processes UI events, timer callbacks, and redraws.
///
/// Use `SlintEventLoop.run()` to start the event loop (blocks the current thread)
/// and `SlintEventLoop.quit()` to stop it.
public enum SlintEventLoop {
    /// Ensures a backend is initialized.
    ///
    /// Call this before creating any windows if you need to ensure the backend
    /// is set up. It is automatically called by `run()` and window creation.
    public static func ensureBackend() {
        slint_ensure_backend()
    }

    /// Runs the event loop synchronously.
    ///
    /// This blocks the calling thread until the event loop is quit (either by
    /// calling `quit()` or when the last window is closed, if
    /// `quitOnLastWindowClosed` is `true`).
    ///
    /// - Parameter quitOnLastWindowClosed: If `true` (the default), the event
    ///   loop will quit automatically when the last window is closed.
    public static func run(quitOnLastWindowClosed: Bool = true) {
        slint_run_event_loop(quitOnLastWindowClosed)
    }

    /// Quits the event loop.
    ///
    /// This causes `run()` to return. Can be called from any thread.
    public static func quit() {
        slint_quit_event_loop()
    }

    /// Posts a callback to be executed on the main event loop thread.
    ///
    /// This is thread-safe and can be called from any thread. The callback
    /// will be executed during the next event loop iteration on the main thread.
    ///
    /// - Parameter callback: The closure to execute on the main thread.
    public static func postEvent(_ callback: @escaping () -> Void) {
        let box_ = EventCallbackBox(callback)
        let context = Unmanaged.passRetained(box_).toOpaque()
        slint_post_event(eventCallbackInvoke, context, eventCallbackDrop)
    }

    /// Runs the event loop asynchronously using Swift concurrency.
    ///
    /// This starts the event loop on a detached task and suspends the current
    /// async context until the event loop finishes.
    @MainActor
    public static func runAsync(quitOnLastWindowClosed: Bool = true) async {
        await withCheckedContinuation { continuation in
            Task.detached {
                await MainActor.run {
                    slint_run_event_loop(quitOnLastWindowClosed)
                }
                continuation.resume()
            }
        }
    }
}
