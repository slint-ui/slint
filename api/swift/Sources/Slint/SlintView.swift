// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#if canImport(SwiftUI)
import SwiftUI
import Slint
@preconcurrency import SlintCBridge

/// A SwiftUI view that hosts Slint UI content.
///
/// `SlintView` wraps the platform-specific view (`SlintNSView` on macOS,
/// `SlintUIKitView` on iOS) and integrates it into a SwiftUI view hierarchy.
///
/// ```swift
/// struct ContentView: View {
///     let windowHandle: SlintWindowAdapterHandle
///
///     var body: some View {
///         SlintView(windowHandle: windowHandle)
///     }
/// }
/// ```
@MainActor
public struct SlintView {
    let windowHandle: SlintWindowAdapterHandle

    /// Creates a new `SlintView` with the given window adapter handle.
    public init(windowHandle: SlintWindowAdapterHandle) {
        self.windowHandle = windowHandle
    }
}

#if os(macOS) && !targetEnvironment(macCatalyst)

extension SlintView: NSViewRepresentable {
    public func makeNSView(context: Context) -> SlintNSView {
        let view = SlintNSView(windowHandle: windowHandle)
        return view
    }

    public func updateNSView(_ nsView: SlintNSView, context: Context) {
        // The view updates itself via layout callbacks
    }
}

#elseif canImport(UIKit) && !os(watchOS)

extension SlintView: UIViewRepresentable {
    public func makeUIView(context: Context) -> SlintUIKitView {
        let view = SlintUIKitView(windowHandle: windowHandle)
        return view
    }

    public func updateUIView(_ uiView: SlintUIKitView, context: Context) {
        // The view updates itself via layout callbacks
    }
}

#endif

#endif
