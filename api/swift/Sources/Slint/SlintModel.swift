// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// MARK: - SlintModel protocol

/// A data model that provides rows of elements to a Slint UI.
///
/// Implement this protocol to provide custom data to Slint's repeater or list-view
/// components. The Slint runtime observes changes through the notification methods
/// (`notifyRowChanged`, `notifyRowAdded`, `notifyRowRemoved`, `notifyReset`).
///
/// Phase 2 provides a pure-Swift implementation. The FFI notifications that inform
/// the Slint runtime of changes are wired up in Phase 3 when interpreter support is
/// added.
public protocol SlintModel<Element>: AnyObject {
    /// The type of element stored in the model.
    associatedtype Element

    /// The number of rows in the model.
    var rowCount: Int { get }

    /// Accesses the element at `index`. Returns `nil` for out-of-bounds reads;
    /// out-of-bounds writes are silently ignored.
    subscript(index: Int) -> Element? { get set }

    /// Notifies that the element at `index` has changed.
    func notifyRowChanged(_ index: Int)

    /// Notifies that `count` elements were inserted starting at `index`.
    func notifyRowAdded(_ index: Int, count: Int)

    /// Notifies that `count` elements were removed starting at `index`.
    func notifyRowRemoved(_ index: Int, count: Int)

    /// Notifies that the entire model has been replaced.
    func notifyReset()
}

// MARK: - Default notification stubs

extension SlintModel {
    public func notifyRowChanged(_ index: Int) {}
    public func notifyRowAdded(_ index: Int, count: Int) {}
    public func notifyRowRemoved(_ index: Int, count: Int) {}
    public func notifyReset() {}
}

// MARK: - SlintArrayModel<T>

/// A concrete, array-backed implementation of `SlintModel`.
///
/// `SlintArrayModel` stores its elements in a Swift `Array` and fires the appropriate
/// change notifications on every mutation. This is the standard model for most use
/// cases where data is managed on the Swift side.
///
/// Example:
/// ```swift
/// let model = SlintArrayModel(["Alice", "Bob", "Carol"])
/// model.append("Dave")         // notifyRowAdded(3, count: 1)
/// model.remove(at: 0)          // notifyRowRemoved(0, count: 1)
/// model.setRowData(at: 0, data: "Robert")  // notifyRowChanged(0)
/// ```
public final class SlintArrayModel<T>: SlintModel {
    public typealias Element = T

    private var storage: [T]

    /// Creates a model with the given initial elements.
    public init(_ elements: [T] = []) {
        storage = elements
    }

    // MARK: SlintModel conformance

    public var rowCount: Int { storage.count }

    public subscript(index: Int) -> T? {
        get {
            guard index >= 0, index < storage.count else { return nil }
            return storage[index]
        }
        set {
            guard let newValue, index >= 0, index < storage.count else { return }
            storage[index] = newValue
            notifyRowChanged(index)
        }
    }

    // MARK: Mutations

    /// Appends an element to the end of the model.
    public func append(_ element: T) {
        let index = storage.count
        storage.append(element)
        notifyRowAdded(index, count: 1)
    }

    /// Inserts an element at the given index.
    public func insert(_ element: T, at index: Int) {
        guard index >= 0, index <= storage.count else { return }
        storage.insert(element, at: index)
        notifyRowAdded(index, count: 1)
    }

    /// Removes the element at the given index.
    public func remove(at index: Int) {
        guard index >= 0, index < storage.count else { return }
        storage.remove(at: index)
        notifyRowRemoved(index, count: 1)
    }

    /// Replaces all elements with a new array and fires a reset notification.
    public func reset(with elements: [T]) {
        storage = elements
        notifyReset()
    }

    // MARK: Convenience

    /// The number of elements (same as `rowCount`).
    public var count: Int { storage.count }

    /// Returns `true` if the model contains no elements.
    public var isEmpty: Bool { storage.isEmpty }
}
