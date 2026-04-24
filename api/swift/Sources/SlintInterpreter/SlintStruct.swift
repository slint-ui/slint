// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

@preconcurrency import SlintCBridge

// MARK: - SlintStruct

/// A key-value mapping of named fields, mirroring a Slint `struct` type.
///
/// Fields can be read and written by name. The underlying storage is a heap-allocated
/// Rust `Struct`.
public final class SlintStruct: @unchecked Sendable {
    // Opaque pointer to a heap-allocated `Struct` (Box<Struct> in Rust).
    var ptr: OpaquePointer  // *mut Struct

    /// Creates an empty struct.
    public init() {
        ptr = slint_swift_struct_new()!
    }

    /// Wraps an existing raw pointer (takes ownership).
    init(takingOwnership rawPtr: OpaquePointer) {
        ptr = rawPtr
    }

    deinit {
        slint_swift_struct_drop(ptr)
    }

    // MARK: Field access

    /// Returns the value of the field named `name`, or `nil` if absent.
    public func getField(_ name: String) -> SlintValue? {
        guard let raw = name.withCString({ namePtr in
            slint_swift_struct_get_field(ptr, namePtr, UInt(name.utf8.count))
        }) else { return nil }
        return SlintValue(takingOwnership: raw)
    }

    /// Sets the field named `name` to `value`.
    public func setField(_ name: String, value: SlintValue) {
        name.withCString { namePtr in
            slint_swift_struct_set_field(
                ptr,
                namePtr,
                UInt(name.utf8.count),
                value.ptr
            )
        }
    }

    /// The number of fields in the struct.
    public var fieldCount: Int {
        Int(slint_swift_struct_field_count(ptr))
    }

    /// Returns the name of the field at `index`, or `nil` if out of bounds.
    public func fieldName(at index: Int) -> String? {
        var ptr_: UnsafePointer<CChar>? = nil
        var len: UInt = 0
        guard slint_swift_struct_field_name_at(
            ptr, UInt(index), &ptr_, &len
        ), let cPtr = ptr_ else { return nil }
        let uint8Ptr = UnsafeRawPointer(cPtr).assumingMemoryBound(to: UInt8.self)
        return String(decoding: UnsafeBufferPointer(start: uint8Ptr, count: Int(len)), as: UTF8.self)
    }

    /// Returns all field names in an unspecified order.
    public var fieldNames: [String] {
        (0..<fieldCount).compactMap { fieldName(at: $0) }
    }

    /// Returns a deep copy of this struct.
    public func clone() -> SlintStruct {
        SlintStruct(takingOwnership: slint_swift_struct_clone(ptr)!)
    }

    // MARK: Internal helpers

    /// Calls `body` with the raw `OpaquePointer` to this struct.
    func withOpaquePointer<R>(_ body: (OpaquePointer) -> R) -> R {
        body(ptr)
    }
}

// MARK: - Subscript

extension SlintStruct {
    /// Accesses a field by name.
    public subscript(field: String) -> SlintValue? {
        get { getField(field) }
        set {
            if let value = newValue {
                setField(field, value: value)
            }
        }
    }
}
