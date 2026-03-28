// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

@preconcurrency import SlintCBridge

/// A Swift wrapper around Slint's `SharedString` type.
///
/// `SlintString` provides efficient, reference-counted string storage that is
/// compatible with Slint's internal string representation. It bridges between
/// Swift `String` and the Rust `SharedString` type through the C FFI.
public final class SlintString: @unchecked Sendable {
    var handle: SlintSharedStringOpaque

    /// Creates a `SlintString` from a Swift `String`.
    public init(_ string: String) {
        handle = SlintSharedStringOpaque(_0: nil)
        string.withCString { ptr in
            slint_shared_string_from_bytes(&handle, ptr, UInt(string.utf8.count))
        }
    }

    /// Creates a `SlintString` by cloning another `SlintString`.
    public init(cloning other: SlintString) {
        handle = SlintSharedStringOpaque(_0: nil)
        slint_shared_string_clone(&handle, &other.handle)
    }

    deinit {
        slint_shared_string_drop(&handle)
    }

    /// The Swift `String` representation of this `SlintString`.
    public var stringValue: String {
        guard let bytes = slint_shared_string_bytes(&handle) else {
            return ""
        }
        return String(cString: bytes)
    }
}

extension SlintString: ExpressibleByStringLiteral {
    public convenience init(stringLiteral value: String) {
        self.init(value)
    }
}

extension SlintString: CustomStringConvertible {
    public var description: String { stringValue }
}

extension SlintString: Equatable {
    public static func == (lhs: SlintString, rhs: SlintString) -> Bool {
        lhs.stringValue == rhs.stringValue
    }
}

extension SlintString: Hashable {
    public func hash(into hasher: inout Hasher) {
        hasher.combine(stringValue)
    }
}
