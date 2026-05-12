// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#if canImport(UIKit) && !os(watchOS)
import UIKit
@preconcurrency import SlintCBridge

/// A UIKit `UIView` subclass that hosts Slint UI content.
///
/// `SlintUIKitView` forwards touch and keyboard events to the Slint window
/// adapter. Use it to embed Slint content in a UIKit-based iOS application.
///
/// ```swift
/// let slintView = SlintUIKitView(adapter: myWindowAdapter)
/// view.addSubview(slintView)
/// ```
@MainActor
public class SlintUIKitView: UIView {
    /// The window adapter handle used to dispatch events.
    public let windowHandle: SlintWindowAdapterHandle

    /// Creates a new `SlintUIKitView` with the given adapter handle.
    public init(windowHandle: SlintWindowAdapterHandle, frame: CGRect = .zero) {
        self.windowHandle = windowHandle
        super.init(frame: frame)
        commonInit()
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) is not supported")
    }

    private func commonInit() {
        isMultipleTouchEnabled = true
        isUserInteractionEnabled = true
    }

    // MARK: - Layout

    public override func layoutSubviews() {
        super.layoutSubviews()
        let scale = Float(contentScaleFactor)
        let logicalWidth = Float(bounds.width)
        let logicalHeight = Float(bounds.height)
        windowHandle.dispatchScaleFactorChanged(scale)
        windowHandle.dispatchResized(width: logicalWidth, height: logicalHeight)
    }

    public override func didMoveToWindow() {
        super.didMoveToWindow()
        if let window = self.window {
            let scale = Float(window.screen.scale)
            windowHandle.dispatchScaleFactorChanged(scale)
        }
    }

    // MARK: - Touch events

    public override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
        for touch in touches {
            let pos = logicalPosition(for: touch)
            windowHandle.dispatchPointerPressed(x: pos.x, y: pos.y, button: .left)
        }
    }

    public override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) {
        for touch in touches {
            let pos = logicalPosition(for: touch)
            windowHandle.dispatchPointerMoved(x: pos.x, y: pos.y)
        }
    }

    public override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
        for touch in touches {
            let pos = logicalPosition(for: touch)
            windowHandle.dispatchPointerReleased(x: pos.x, y: pos.y, button: .left)
        }
    }

    public override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) {
        for touch in touches {
            let pos = logicalPosition(for: touch)
            windowHandle.dispatchPointerReleased(x: pos.x, y: pos.y, button: .left)
        }
    }

    // MARK: - First responder (for keyboard)

    public override var canBecomeFirstResponder: Bool { true }

    // MARK: - Keyboard (via UIKeyInput)

    // Note: Full keyboard support requires UIKeyInput conformance.
    // For now, keyboard events should be dispatched externally or
    // via a UITextField overlay.

    // MARK: - Coordinate conversion

    private func logicalPosition(for touch: UITouch) -> (x: Float, y: Float) {
        let point = touch.location(in: self)
        // UIKit has origin at top-left, same as Slint
        return (x: Float(point.x), y: Float(point.y))
    }
}

// MARK: - UIKeyInput for keyboard event forwarding

extension SlintUIKitView: UIKeyInput {
    public var hasText: Bool { true }

    public func insertText(_ text: String) {
        windowHandle.dispatchKeyPressed(text: text)
        windowHandle.dispatchKeyReleased(text: text)
    }

    public func deleteBackward() {
        let backspace = "\u{08}" // Slint Key::Backspace
        windowHandle.dispatchKeyPressed(text: backspace)
        windowHandle.dispatchKeyReleased(text: backspace)
    }
}
#endif
