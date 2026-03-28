// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

@preconcurrency import SlintCBridge

/// A box holding a Swift closure, used to bridge closures across the FFI boundary.
private final class CallbackBox {
    let closure: () -> Void
    init(_ closure: @escaping () -> Void) {
        self.closure = closure
    }
}

/// The C function pointer that invokes the Swift closure stored in user_data.
private let timerCallbackInvoke: @convention(c) (UnsafeMutableRawPointer?) -> Void = { ptr in
    guard let ptr = ptr else { return }
    let box_ = Unmanaged<CallbackBox>.fromOpaque(ptr).takeUnretainedValue()
    box_.closure()
}

/// The C function pointer that releases the Swift closure stored in user_data.
private let timerCallbackDrop: @convention(c) (UnsafeMutableRawPointer?) -> Void = { ptr in
    guard let ptr = ptr else { return }
    Unmanaged<CallbackBox>.fromOpaque(ptr).release()
}

/// A timer that can fire callbacks at specified intervals.
///
/// Timers can be single-shot (fire once) or repeated (fire periodically).
/// The timer is automatically destroyed when the `SlintTimer` instance is deallocated.
public final class SlintTimer {
    private var id: UInt

    /// Creates a new timer.
    ///
    /// - Parameters:
    ///   - mode: Whether the timer fires once (`.singleShot`) or repeatedly (`.repeated`).
    ///   - interval: The interval in milliseconds.
    ///   - callback: The closure to call when the timer fires.
    public init(mode: TimerMode, interval: UInt64, callback: @escaping () -> Void) {
        let box_ = CallbackBox(callback)
        let context = Unmanaged.passRetained(box_).toOpaque()
        let cMode: SlintTimerMode = mode == .singleShot ? SLINT_TIMER_MODE_SINGLE_SHOT : SLINT_TIMER_MODE_REPEATED
        id = slint_timer_start(0, cMode, interval, timerCallbackInvoke, context, timerCallbackDrop)
    }

    deinit {
        if id != 0 {
            slint_timer_destroy(id)
        }
    }

    /// Stops the timer. It can be restarted with `restart()`.
    public func stop() {
        slint_timer_stop(id)
    }

    /// Restarts a stopped timer with the same interval.
    public func restart() {
        slint_timer_restart(id)
    }

    /// Whether the timer is currently running.
    public var isRunning: Bool {
        slint_timer_running(id)
    }

    /// The timer's interval in milliseconds.
    public var interval: UInt64 {
        slint_timer_interval(id)
    }

    /// Fires a single-shot timer after a delay.
    ///
    /// - Parameters:
    ///   - delay: The delay in milliseconds before the callback fires.
    ///   - callback: The closure to call when the timer fires.
    public static func singleShot(delay: UInt64, callback: @escaping () -> Void) {
        let box_ = CallbackBox(callback)
        let context = Unmanaged.passRetained(box_).toOpaque()
        slint_timer_singleshot(delay, timerCallbackInvoke, context, timerCallbackDrop)
    }

    /// Timer mode.
    public enum TimerMode {
        /// The timer fires only once.
        case singleShot
        /// The timer fires repeatedly until stopped or destroyed.
        case repeated
    }
}
