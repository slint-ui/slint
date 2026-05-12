// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

@preconcurrency import SlintCBridge

// MARK: - C callback thunks (non-generic, file-scope)

/// Invokes the binding stored in `userData` and writes its result into `retPtr`.
private let propertyBindingInvoke: @convention(c) (
    UnsafeMutableRawPointer?, UnsafeMutableRawPointer?
) -> Void = { userData, retPtr in
    guard let userData, let retPtr else { return }
    Unmanaged<PropertyBindingBox>.fromOpaque(userData)
        .takeUnretainedValue()
        .invoke(retPtr)
}

/// Releases the `PropertyBindingBox` retained when the binding was set.
private let propertyBindingDrop: @convention(c) (UnsafeMutableRawPointer?) -> Void = { userData in
    guard let userData else { return }
    Unmanaged<PropertyBindingBox>.fromOpaque(userData).release()
}

/// Type-erased container for a property binding closure.
private final class PropertyBindingBox {
    /// Calls the binding and writes the result (as raw bytes) into `dest`.
    let invoke: (UnsafeMutableRawPointer) -> Void

    init(_ invoke: @escaping (UnsafeMutableRawPointer) -> Void) {
        self.invoke = invoke
    }
}

// MARK: - SlintProperty<T>

/// A reactive property that integrates with Slint's dependency-tracking system.
///
/// Reading a `SlintProperty<T>` with `get()` registers the current evaluation context as a
/// dependent, so that it is automatically re-evaluated when the property's value changes.
/// Writing with `set(_:)` notifies all dependents. `setBinding(_:)` installs a lazy closure
/// that is re-evaluated whenever the property is read and its upstream dependencies have
/// changed.
///
/// `T` must be a value type whose in-memory representation is compatible with the Rust FFI
/// (e.g. `Int32`, `Float`, `Bool`, `UInt8`).
public final class SlintProperty<T> {
    private var handle: SlintPropertyHandleOpaque
    /// In-memory value storage. The address of this variable is passed to the Rust property
    /// system as the value pointer, so it **must not move**. Because `SlintProperty` is a
    /// `final class`, the stored property lives on the heap and its address is stable for the
    /// object's lifetime.
    private var storage: T

    /// Creates a property with an initial value.
    public init(_ defaultValue: T) {
        storage = defaultValue
        handle = SlintPropertyHandleOpaque(_0: 0)
        slint_property_init(&handle)
    }

    deinit {
        slint_property_drop(&handle)
    }

    /// The current value of the property.
    ///
    /// Getting evaluates any active binding and registers this read as a dependency.
    /// Setting removes any active binding and marks all dependents as dirty.
    public var value: T {
        get {
            withUnsafeMutablePointer(to: &storage) { ptr in
                slint_property_update(&handle, ptr)
            }
            return storage
        }
        set {
            storage = newValue
            withUnsafePointer(to: storage) { ptr in
                slint_property_set_changed(&handle, ptr)
            }
        }
    }

    /// Installs a binding closure. The closure is called lazily each time `get()` is called
    /// and the property's dependencies have changed.
    public func setBinding(_ binding: @escaping () -> T) {
        let box = PropertyBindingBox { retPtr in
            let result = binding()
            retPtr.assumingMemoryBound(to: T.self).pointee = result
        }
        let ptr = Unmanaged.passRetained(box).toOpaque()
        slint_property_set_binding(&handle, propertyBindingInvoke, ptr, propertyBindingDrop, nil, nil)
    }
}
