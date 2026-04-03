// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

@preconcurrency import SlintCBridge

/// Protocol for custom Slint platforms.
///
/// Implement this protocol to provide a custom platform that controls how
/// windows are created and how the event loop runs. Register it with
/// `SlintPlatform.register(_:)` before creating any windows.
///
/// All methods are called on the main thread by the Slint runtime.
/// For most applications using the default backend (winit), you do not need
/// to implement this protocol.
public protocol SlintPlatformProtocol: AnyObject {
    /// Creates a new window adapter. Called by the Slint runtime when a new
    /// window is needed.
    func createWindowAdapter() -> any SlintWindowAdapterProtocol

    /// Runs the platform event loop. This should block until the event loop
    /// is quit.
    func runEventLoop()

    /// Quits the event loop. May be called from any thread.
    func quitEventLoop()

    /// Invokes a task on the event loop thread.
    ///
    /// The task must be run on the main thread by calling `task.run()`.
    /// If you cannot run the task, call `task.drop()` to release it.
    func invokeFromEventLoop(_ task: SlintPlatformTask)
}

/// An opaque task that must be run on the event loop thread.
///
/// Received from the Slint runtime via `SlintPlatformProtocol.invokeFromEventLoop(_:)`.
/// You must call either `run()` or `drop()` exactly once.
public struct SlintPlatformTask: @unchecked Sendable {
    private var opaque: SlintPlatformTaskOpaque
    private var consumed: Bool = false

    init(opaque: SlintPlatformTaskOpaque) {
        self.opaque = opaque
    }

    /// Runs the task. Must be called on the main/event-loop thread.
    public mutating func run() {
        precondition(!consumed, "SlintPlatformTask already consumed")
        consumed = true
        slint_swift_platform_task_run(opaque)
    }

    /// Drops the task without running it.
    public mutating func drop() {
        precondition(!consumed, "SlintPlatformTask already consumed")
        consumed = true
        slint_swift_platform_task_drop(opaque)
    }
}

/// Namespace for platform registration and timer management.
public enum SlintPlatform {
    /// Registers a custom platform with the Slint runtime.
    ///
    /// Must be called before creating any windows or running the event loop.
    /// Can only be called once.
    public static func register(_ platform: some SlintPlatformProtocol) {
        let box_ = PlatformBox(platform)
        let context = Unmanaged.passRetained(box_).toOpaque()

        slint_swift_platform_register(
            context,
            platformDrop,
            platformWindowFactory,
            platformRunEventLoop,
            platformQuitEventLoop,
            platformInvokeFromEventLoop
        )
    }

    /// Updates all timers and animations.
    ///
    /// Call this from your custom event loop to process pending timer callbacks
    /// and advance animations.
    public static func updateTimersAndAnimations() {
        slint_swift_platform_update_timers_and_animations()
    }

    /// Returns the number of milliseconds until the next timer fires,
    /// or `nil` if no timer is pending.
    public static func durationUntilNextTimerUpdate() -> UInt64? {
        let ms = slint_swift_platform_duration_until_next_timer_update()
        return ms == UInt64.max ? nil : ms
    }
}

// MARK: - Platform C callbacks

private final class PlatformBox: @unchecked Sendable {
    let platform: any SlintPlatformProtocol
    init(_ platform: some SlintPlatformProtocol) {
        self.platform = platform
    }
}

nonisolated(unsafe) private let platformDrop: @convention(c) (
    UnsafeMutableRawPointer?
) -> Void = { ptr in
    guard let ptr = ptr else { return }
    Unmanaged<PlatformBox>.fromOpaque(ptr).release()
}

nonisolated(unsafe) private let platformWindowFactory: @convention(c) (
    UnsafeMutableRawPointer?, UnsafeMutablePointer<SlintWindowAdapterRcOpaque>?
) -> Void = { userData, target in
    guard let userData = userData, let target = target else { return }
    let box_ = Unmanaged<PlatformBox>.fromOpaque(userData).takeUnretainedValue()
    let adapter = box_.platform.createWindowAdapter()
    adapter.writeInto(target: target)
}

nonisolated(unsafe) private let platformRunEventLoop: @convention(c) (
    UnsafeMutableRawPointer?
) -> Void = { userData in
    guard let userData = userData else { return }
    let box_ = Unmanaged<PlatformBox>.fromOpaque(userData).takeUnretainedValue()
    box_.platform.runEventLoop()
}

nonisolated(unsafe) private let platformQuitEventLoop: @convention(c) (
    UnsafeMutableRawPointer?
) -> Void = { userData in
    guard let userData = userData else { return }
    let box_ = Unmanaged<PlatformBox>.fromOpaque(userData).takeUnretainedValue()
    box_.platform.quitEventLoop()
}

nonisolated(unsafe) private let platformInvokeFromEventLoop: @convention(c) (
    UnsafeMutableRawPointer?, SlintPlatformTaskOpaque
) -> Void = { userData, taskOpaque in
    guard let userData = userData else {
        slint_swift_platform_task_drop(taskOpaque)
        return
    }
    let box_ = Unmanaged<PlatformBox>.fromOpaque(userData).takeUnretainedValue()
    let task = SlintPlatformTask(opaque: taskOpaque)
    box_.platform.invokeFromEventLoop(task)
}
