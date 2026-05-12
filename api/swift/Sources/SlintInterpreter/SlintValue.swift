// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

@preconcurrency import SlintCBridge

// MARK: - SlintValueType

/// The type tag of a `SlintValue`.
public enum SlintValueType: Int8 {
    case void = 0
    case number = 1
    case string = 2
    case bool = 3
    case model = 4
    case `struct` = 5
    case brush = 6
    case image = 7
    case other = -1

    // Use the module-qualified name to avoid shadowing this Swift enum with the C typedef.
    init(raw: SlintCBridge.SlintValueType) {
        // C enums may be imported as Int32 or UInt32 depending on platform.
        // The Rust side uses i8 repr, so values like -1 (OTHER) may appear
        // as large unsigned values. Truncate to Int8 to recover the original
        // signed value safely.
        self = SlintValueType(rawValue: Int8(truncatingIfNeeded: raw.rawValue)) ?? .other
    }
}

// MARK: - SlintValue

/// A dynamically-typed value produced or consumed by the Slint interpreter.
///
/// Values are heap-allocated on the Rust side and managed by this Swift class.
public final class SlintValue: @unchecked Sendable {
    var ptr: OpaquePointer  // *mut Value

    /// Creates a void (default) value.
    public init() {
        ptr = slint_swift_value_new_void()!
    }

    /// Creates a number value.
    public init(_ value: Double) {
        ptr = slint_swift_value_new_double(value)!
    }

    /// Creates a boolean value.
    public init(_ value: Bool) {
        ptr = slint_swift_value_new_bool(value)!
    }

    /// Creates a string value.
    public init(_ value: String) {
        ptr = value.withCString { bytes in
            slint_swift_value_new_string(bytes, UInt(value.utf8.count))!
        }
    }

    /// Creates a struct value.
    public init(_ value: SlintStruct) {
        ptr = value.withOpaquePointer { stru in
            slint_swift_value_new_struct(stru)!
        }
    }

    /// Wraps an existing raw pointer (takes ownership).
    init(takingOwnership rawPtr: OpaquePointer) {
        ptr = rawPtr
    }

    deinit {
        slint_swift_value_drop(ptr)
    }

    // MARK: Type

    /// The type discriminant of this value.
    public var valueType: SlintValueType {
        SlintValueType(raw: slint_swift_value_type(ptr))
    }

    // MARK: Extractors

    /// The numeric value, or `nil` if this is not a number.
    public var asDouble: Double? {
        var out: Double = 0
        guard slint_swift_value_to_double(ptr, &out) else { return nil }
        return out
    }

    /// The boolean value, or `nil` if this is not a bool.
    public var asBool: Bool? {
        var out: Bool = false
        guard slint_swift_value_to_bool(ptr, &out) else { return nil }
        return out
    }

    /// The string value, or `nil` if this is not a string.
    public var asString: String? {
        var ptr_: UnsafePointer<CChar>? = nil
        var len: UInt = 0
        guard slint_swift_value_to_string(ptr, &ptr_, &len), let cPtr = ptr_ else {
            return nil
        }
        let uint8Ptr = UnsafeRawPointer(cPtr).assumingMemoryBound(to: UInt8.self)
        return String(decoding: UnsafeBufferPointer(start: uint8Ptr, count: Int(len)), as: UTF8.self)
    }

    /// The struct value, or `nil` if this is not a struct.
    public var asStruct: SlintStruct? {
        guard let raw = slint_swift_value_to_struct(ptr) else {
            return nil
        }
        return SlintStruct(takingOwnership: raw)
    }

    // MARK: Cloning

    /// Returns a deep copy of this value.
    public func clone() -> SlintValue {
        SlintValue(takingOwnership: slint_swift_value_clone(ptr)!)
    }
}

// MARK: - Convenience initialisers via ExpressibleBy protocols

extension SlintValue: ExpressibleByFloatLiteral {
    public convenience init(floatLiteral value: Double) { self.init(value) }
}

extension SlintValue: ExpressibleByIntegerLiteral {
    public convenience init(integerLiteral value: Int) { self.init(Double(value)) }
}

extension SlintValue: ExpressibleByBooleanLiteral {
    public convenience init(booleanLiteral value: Bool) { self.init(value) }
}

extension SlintValue: ExpressibleByStringLiteral {
    public convenience init(stringLiteral value: String) { self.init(value) }
}
