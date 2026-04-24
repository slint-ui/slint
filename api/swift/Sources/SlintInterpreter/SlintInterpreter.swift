// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

@preconcurrency import SlintCBridge
import Slint

// MARK: - SlintDiagnostic

/// A diagnostic message produced by the compiler.
public struct SlintDiagnostic: Sendable {
    /// The human-readable description of the problem.
    public let message: String
    /// The source file in which the problem was found.
    public let sourceFile: String
    /// 1-based line number.
    public let line: Int
    /// 1-based column number.
    public let column: Int
    /// Severity.
    public let level: Level

    /// Diagnostic severity.
    public enum Level: Sendable {
        case error
        case warning
        case note
    }
}

// MARK: - SlintComponentDefinition

/// The compiled description of a Slint component — its name, public properties, and callbacks.
///
/// Create instances with `SlintCompiler.buildFromSource(_:path:)`.
public final class SlintComponentDefinition: @unchecked Sendable {
    // Opaque pointer to a heap-allocated `ComponentDefinition` (Box<ComponentDefinition> in Rust).
    var ptr: OpaquePointer  // *mut ComponentDefinition

    init(takingOwnership rawPtr: OpaquePointer) {
        ptr = rawPtr
    }

    deinit {
        slint_swift_definition_drop(ptr)
    }

    // MARK: Metadata

    /// The name of the root component as declared in the `.slint` source.
    public var name: String {
        var storage = SlintSharedStringOpaque()
        slint_swift_definition_name(ptr, &storage)
        defer { slint_shared_string_drop(&storage) }
        return String(cString: slint_shared_string_bytes(&storage))
    }

    /// The number of public properties exposed by this component.
    public var propertiesCount: Int {
        Int(slint_swift_definition_properties_count(ptr))
    }

    /// Returns the name and type of the property at `index`, or `nil` if out of range.
    public func property(at index: Int) -> (name: String, type: SlintValueType)? {
        var storage = SlintSharedStringOpaque()
        var rawType = SLINT_VALUE_TYPE_VOID
        guard slint_swift_definition_property_at(
            ptr, UInt(index), &storage, &rawType
        ) else { return nil }
        defer { slint_shared_string_drop(&storage) }
        let name = String(cString: slint_shared_string_bytes(&storage))
        return (name, SlintValueType(raw: rawType))
    }

    /// All public properties of this component as an array of `(name, type)` pairs.
    public var properties: [(name: String, type: SlintValueType)] {
        (0..<propertiesCount).compactMap { property(at: $0) }
    }

    /// The number of public callbacks exposed by this component.
    public var callbacksCount: Int {
        Int(slint_swift_definition_callbacks_count(ptr))
    }

    /// Returns the name of the callback at `index`, or `nil` if out of range.
    public func callbackName(at index: Int) -> String? {
        var storage = SlintSharedStringOpaque()
        guard slint_swift_definition_callback_at(
            ptr, UInt(index), &storage
        ) else { return nil }
        defer { slint_shared_string_drop(&storage) }
        return String(cString: slint_shared_string_bytes(&storage))
    }

    /// All public callback names.
    public var callbackNames: [String] {
        (0..<callbacksCount).compactMap { callbackName(at: $0) }
    }

    // MARK: Instance creation

    /// Creates a new component instance.
    ///
    /// - Returns: A `SlintComponentInstance`, or `nil` if the platform is not yet initialised.
    @MainActor
    public func createInstance() -> SlintComponentInstance? {
        guard let raw = slint_swift_definition_create_instance(ptr) else {
            return nil
        }
        return SlintComponentInstance(takingOwnership: raw)
    }
}

// MARK: - SlintComponentInstance

/// A running instance of a compiled Slint component.
///
/// Properties can be read and written, callbacks can be registered and invoked,
/// and the associated window can be shown and hidden.
@MainActor
public final class SlintComponentInstance {
    // Opaque pointer to a heap-allocated `ComponentInstance` (Box<ComponentInstance> in Rust).
    nonisolated(unsafe) var ptr: OpaquePointer  // *mut ComponentInstance

    @preconcurrency init(takingOwnership rawPtr: OpaquePointer) {
        ptr = rawPtr
    }

    deinit {
        slint_swift_instance_drop(ptr)
    }

    // MARK: Window management

    /// Shows the component's window.
    public func show() {
        slint_swift_instance_show(ptr, true)
    }

    /// Hides the component's window.
    public func hide() {
        slint_swift_instance_show(ptr, false)
    }

    // MARK: Properties

    /// Returns the current value of the named property, or `nil` on failure.
    public func getProperty(_ name: String) -> SlintValue? {
        guard let raw = name.withCString({ namePtr in
            slint_swift_instance_get_property(
                ptr, namePtr, UInt(name.utf8.count)
            )
        }) else { return nil }
        return SlintValue(takingOwnership: raw)
    }

    /// Sets the named property to `value`. Returns `true` on success.
    @discardableResult
    public func setProperty(_ name: String, value: SlintValue) -> Bool {
        name.withCString { namePtr in
            slint_swift_instance_set_property(
                ptr,
                namePtr,
                UInt(name.utf8.count),
                value.ptr
            )
        }
    }

    // MARK: Callbacks / functions

    /// Invokes the named callback or function with `args`.
    ///
    /// - Returns: The return value, or `nil` on failure.
    public func invoke(_ name: String, args: [SlintValue] = []) -> SlintValue? {
        let rawArgs: [OpaquePointer?] = args.map { Optional($0.ptr) }
        let result: OpaquePointer? = name.withCString { namePtr in
            rawArgs.withUnsafeBufferPointer { argsBuf in
                slint_swift_instance_invoke(
                    ptr,
                    namePtr,
                    UInt(name.utf8.count),
                    argsBuf.baseAddress,
                    UInt(args.count)
                )
            }
        }
        guard let raw = result else { return nil }
        return SlintValue(takingOwnership: raw)
    }
}

// MARK: - SlintCompiler

/// Compiles `.slint` source code into `SlintComponentDefinition` values.
///
/// ```swift
/// let compiler = SlintCompiler()
/// guard let def = compiler.buildFromSource("export component Foo inherits Window {}",
///                                          path: "foo.slint") else {
///     print(compiler.diagnostics.map(\.message))
///     return
/// }
/// let instance = def.createInstance()!
/// ```
public final class SlintCompiler: @unchecked Sendable {
    // Opaque pointer to a heap-allocated `SwiftCompiler` in Rust.
    private var ptr: OpaquePointer  // *mut SwiftCompiler

    public init() {
        ptr = slint_swift_compiler_new()!
    }

    deinit {
        slint_swift_compiler_drop(ptr)
    }

    // MARK: Configuration

    /// Sets the widget style (e.g. `"fluent"`, `"material"`, `"native"`).
    public func setStyle(_ style: String) {
        style.withCString { ptr_ in
            slint_swift_compiler_set_style(ptr, ptr_, UInt(style.utf8.count))
        }
    }

    // MARK: Compilation

    /// Compiles `.slint` source code.
    ///
    /// - Parameters:
    ///   - source: The `.slint` source text.
    ///   - path: A virtual file path used in diagnostic messages (may be empty).
    /// - Returns: The first exported `SlintComponentDefinition`, or `nil` on failure.
    public func buildFromSource(_ source: String, path: String = "") -> SlintComponentDefinition? {
        let raw: OpaquePointer? = source.withCString { srcPtr in
            path.withCString { pathPtr in
                slint_swift_compiler_build_from_source(
                    ptr,
                    srcPtr, UInt(source.utf8.count),
                    pathPtr, UInt(path.utf8.count)
                )
            }
        }
        guard let raw else { return nil }
        return SlintComponentDefinition(takingOwnership: raw)
    }

    // MARK: Diagnostics

    /// The diagnostics produced by the most recent compilation.
    public var diagnostics: [SlintDiagnostic] {
        let count = Int(slint_swift_compiler_diagnostics_count(ptr))
        return (0..<count).compactMap { index -> SlintDiagnostic? in
            var msgPtr: UnsafePointer<CChar>? = nil
            var msgLen: UInt = 0
            var filePtr: UnsafePointer<CChar>? = nil
            var fileLen: UInt = 0
            var line: UInt = 0
            var column: UInt = 0
            var level: SlintDiagnosticLevel = SLINT_DIAGNOSTIC_LEVEL_ERROR
            guard slint_swift_compiler_get_diagnostic(
                ptr, UInt(index),
                &msgPtr, &msgLen,
                &filePtr, &fileLen,
                &line, &column, &level
            ) else { return nil }
            let msg = msgPtr.map { cPtr -> String in
                let uint8Ptr = UnsafeRawPointer(cPtr).assumingMemoryBound(to: UInt8.self)
                return String(decoding: UnsafeBufferPointer(start: uint8Ptr, count: Int(msgLen)), as: UTF8.self)
            } ?? ""
            let file = filePtr.map { cPtr -> String in
                let uint8Ptr = UnsafeRawPointer(cPtr).assumingMemoryBound(to: UInt8.self)
                return String(decoding: UnsafeBufferPointer(start: uint8Ptr, count: Int(fileLen)), as: UTF8.self)
            } ?? ""
            let lvl: SlintDiagnostic.Level
            switch level {
            case SLINT_DIAGNOSTIC_LEVEL_ERROR: lvl = .error
            case SLINT_DIAGNOSTIC_LEVEL_WARNING: lvl = .warning
            default: lvl = .note
            }
            return SlintDiagnostic(
                message: msg, sourceFile: file,
                line: Int(line), column: Int(column), level: lvl
            )
        }
    }

    /// Returns `true` if the last compilation produced any error-level diagnostics.
    public var hasErrors: Bool {
        slint_swift_compiler_has_errors(ptr)
    }
}
