// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#if canImport(AppKit) && !targetEnvironment(macCatalyst)
import AppKit
@preconcurrency import SlintCBridge

/// An AppKit `NSView` subclass that hosts Slint UI content.
///
/// `SlintNSView` forwards mouse, keyboard, and window lifecycle events
/// to the Slint window adapter. Use it to embed Slint content in an
/// AppKit-based macOS application.
///
/// ```swift
/// let slintView = SlintNSView(adapter: myWindowAdapter)
/// window.contentView = slintView
/// ```
@MainActor
public class SlintNSView: NSView {
    /// The window adapter handle used to dispatch events.
    public let windowHandle: SlintWindowAdapterHandle

    /// Creates a new `SlintNSView` with the given adapter handle.
    public init(windowHandle: SlintWindowAdapterHandle, frame: NSRect = .zero) {
        self.windowHandle = windowHandle
        super.init(frame: frame)
        commonInit()
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) is not supported")
    }

    private func commonInit() {
        wantsLayer = true
        // Accept first responder for keyboard events
    }

    // MARK: - First responder

    public override var acceptsFirstResponder: Bool { true }

    public override func becomeFirstResponder() -> Bool {
        windowHandle.dispatchWindowActiveChanged(true)
        return super.becomeFirstResponder()
    }

    public override func resignFirstResponder() -> Bool {
        windowHandle.dispatchWindowActiveChanged(false)
        return super.resignFirstResponder()
    }

    // MARK: - Layout

    public override func layout() {
        super.layout()
        let scaleFactor = Float(window?.backingScaleFactor ?? 1.0)
        let logicalWidth = Float(bounds.width)
        let logicalHeight = Float(bounds.height)
        windowHandle.dispatchScaleFactorChanged(scaleFactor)
        windowHandle.dispatchResized(width: logicalWidth, height: logicalHeight)
    }

    public override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        if let window = self.window {
            let scaleFactor = Float(window.backingScaleFactor)
            windowHandle.dispatchScaleFactorChanged(scaleFactor)
        }
    }

    // MARK: - Mouse events

    public override func mouseDown(with event: NSEvent) {
        let pos = logicalPosition(for: event)
        windowHandle.dispatchPointerPressed(x: pos.x, y: pos.y, button: .left)
    }

    public override func mouseUp(with event: NSEvent) {
        let pos = logicalPosition(for: event)
        windowHandle.dispatchPointerReleased(x: pos.x, y: pos.y, button: .left)
    }

    public override func mouseMoved(with event: NSEvent) {
        let pos = logicalPosition(for: event)
        windowHandle.dispatchPointerMoved(x: pos.x, y: pos.y)
    }

    public override func mouseDragged(with event: NSEvent) {
        let pos = logicalPosition(for: event)
        windowHandle.dispatchPointerMoved(x: pos.x, y: pos.y)
    }

    public override func rightMouseDown(with event: NSEvent) {
        let pos = logicalPosition(for: event)
        windowHandle.dispatchPointerPressed(x: pos.x, y: pos.y, button: .right)
    }

    public override func rightMouseUp(with event: NSEvent) {
        let pos = logicalPosition(for: event)
        windowHandle.dispatchPointerReleased(x: pos.x, y: pos.y, button: .right)
    }

    public override func rightMouseDragged(with event: NSEvent) {
        let pos = logicalPosition(for: event)
        windowHandle.dispatchPointerMoved(x: pos.x, y: pos.y)
    }

    public override func otherMouseDown(with event: NSEvent) {
        let pos = logicalPosition(for: event)
        windowHandle.dispatchPointerPressed(x: pos.x, y: pos.y, button: .middle)
    }

    public override func otherMouseUp(with event: NSEvent) {
        let pos = logicalPosition(for: event)
        windowHandle.dispatchPointerReleased(x: pos.x, y: pos.y, button: .middle)
    }

    public override func otherMouseDragged(with event: NSEvent) {
        let pos = logicalPosition(for: event)
        windowHandle.dispatchPointerMoved(x: pos.x, y: pos.y)
    }

    public override func scrollWheel(with event: NSEvent) {
        let pos = logicalPosition(for: event)
        let deltaX = Float(event.scrollingDeltaX)
        let deltaY = Float(event.scrollingDeltaY)
        windowHandle.dispatchPointerScrolled(x: pos.x, y: pos.y, deltaX: deltaX, deltaY: deltaY)
    }

    public override func mouseExited(with event: NSEvent) {
        windowHandle.dispatchPointerExited()
    }

    // Enable mouse tracking for move and exit events
    public override func updateTrackingAreas() {
        super.updateTrackingAreas()
        for area in trackingAreas {
            removeTrackingArea(area)
        }
        let area = NSTrackingArea(
            rect: bounds,
            options: [.activeInKeyWindow, .mouseMoved, .mouseEnteredAndExited, .inVisibleRect],
            owner: self,
            userInfo: nil
        )
        addTrackingArea(area)
    }

    // MARK: - Keyboard events

    public override func keyDown(with event: NSEvent) {
        guard let chars = event.characters, !chars.isEmpty else { return }
        if event.isARepeat {
            windowHandle.dispatchKeyPressRepeated(text: chars)
        } else {
            windowHandle.dispatchKeyPressed(text: chars)
        }
    }

    public override func keyUp(with event: NSEvent) {
        guard let chars = event.characters, !chars.isEmpty else { return }
        windowHandle.dispatchKeyReleased(text: chars)
    }

    public override func flagsChanged(with event: NSEvent) {
        // Handle modifier key changes (Shift, Control, etc.)
        // This is called when modifier keys are pressed/released without
        // generating a character. We forward it as key events using
        // special key codes that Slint recognizes.
        let modifiers = event.modifierFlags
        dispatchModifierEvent(flag: .shift, modifiers: modifiers, text: "\u{10}") // Key::Shift
        dispatchModifierEvent(flag: .control, modifiers: modifiers, text: "\u{11}") // Key::Control
        dispatchModifierEvent(flag: .option, modifiers: modifiers, text: "\u{12}") // Key::Alt
        dispatchModifierEvent(flag: .command, modifiers: modifiers, text: "\u{13}") // Key::Meta
    }

    // Track which modifier keys are currently pressed
    private var activeModifiers: NSEvent.ModifierFlags = []

    private func dispatchModifierEvent(flag: NSEvent.ModifierFlags, modifiers: NSEvent.ModifierFlags, text: String) {
        let wasActive = activeModifiers.contains(flag)
        let isActive = modifiers.contains(flag)
        if isActive && !wasActive {
            activeModifiers.insert(flag)
            windowHandle.dispatchKeyPressed(text: text)
        } else if !isActive && wasActive {
            activeModifiers.remove(flag)
            windowHandle.dispatchKeyReleased(text: text)
        }
    }

    // MARK: - Coordinate conversion

    private func logicalPosition(for event: NSEvent) -> (x: Float, y: Float) {
        let localPoint = convert(event.locationInWindow, from: nil)
        // AppKit has origin at bottom-left; Slint uses top-left
        return (x: Float(localPoint.x), y: Float(bounds.height - localPoint.y))
    }
}
#endif
