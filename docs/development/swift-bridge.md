# Swift Language Binding

> Note for AI coding assistants (agents):
> **When to load this document:** Working on `api/swift/`, Swift code generation,
> Swift/C bridging, or iOS/macOS Swift integration.

## Motivation

Slint currently supports Rust, C++, Node.js, and Python as programming languages.
iOS is already a supported platform via the Winit backend and Skia renderer, but
the [iOS platform guide](../astro/src/content/docs/guide/platforms/mobile/ios.mdx)
states: "When developing Slint applications for iOS, you can only use Rust as the
programming language."

A Swift binding would:

- Allow native Swift development for iOS and macOS Slint applications
- Enable SwiftUI interop (embedding Slint views in SwiftUI hierarchies and vice versa)
- Give access to the Apple ecosystem's preferred language, tooling (Xcode), and
  distribution pipeline (App Store)
- Follow the pattern already established by the C++, Node.js, and Python bindings

## Architecture Overview

The Swift bridge calls the Rust `extern "C"` FFI functions directly through a C
bridging header, then provides idiomatic Swift wrapper classes on top. This follows
the same pattern as the Python (PyO3) and Node.js (Neon) bindings, which also call
into Rust directly rather than going through the C++ headers.

```
┌─────────────────────────────────────────────────────────────────────┐
│                        Language APIs                                │
├──────────┬──────────┬──────────┬────────────────────────────────────┤
│ C++ API  │ Node.js  │ Python   │ Swift API                          │
│ cbindgen │ Neon     │ PyO3     │ C bridging header + Swift wrappers │
├──────────┴──────────┴──────────┴────────────────────────────────────┤
│                     FFI Layer (extern "C")                          │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐                 │
│  │ properties/  │ │ window.rs    │ │ interpreter/ │                 │
│  │ ffi.rs       │ │ ffi module   │ │ ffi.rs       │                 │
│  └──────────────┘ └──────────────┘ └──────────────┘                 │
├─────────────────────────────────────────────────────────────────────┤
│                     Internal Rust Crates                            │
│  i-slint-core   i-slint-compiler   slint-interpreter                │
└─────────────────────────────────────────────────────────────────────┘
```

The C++ headers in `api/cpp/include/` are themselves just thin wrappers around
these same `extern "C"` functions. By calling the C FFI directly, the Swift
binding avoids a dependency on Swift's C++ interoperability (which is still
maturing) and the C++ compiler toolchain entirely.

## Interop Strategy: Pure C Bridging Header

### Rationale

The Rust crates already export all needed functionality as `extern "C"` functions,
gated behind `#[cfg(feature = "ffi")]`. These are plain C ABI symbols — no C++
required. Swift can call C functions natively through a bridging header without
any special interop mode.

This approach:

- **Requires no C++ compiler** in the build chain — simpler SPM package, faster builds
- **Has no dependency on Swift's C++ interop** — avoids limitations with templates,
  namespaces, and evolving interop semantics
- **Produces a fully idiomatic Swift API** — can use `@MainActor`, property wrappers,
  `Result` types, protocols, and closures naturally
- **Works on every platform Swift supports** — iOS, macOS, Linux, Windows — with no
  platform-specific C++ interop quirks
- **Follows the established pattern** — Python and Node.js bindings also call Rust
  directly, not through the C++ headers

### C Bridging Header

A `SlintCore.h` bridging header declares the `extern "C"` functions for Swift.
The functions already exist in the Rust crates:

```c
// SlintCore.h — pure C declarations of Rust FFI functions

#ifndef SLINT_CORE_H
#define SLINT_CORE_H

#include <stdint.h>
#include <stdbool.h>

// Opaque types (match the #[repr(C)] structs in Rust)
typedef struct { void *_0; void *_1; } SlintWindowAdapterRcOpaque;
typedef struct { uintptr_t _0; } SlintPropertyHandleOpaque;
typedef struct { void *_0; void *_1; } SlintCallbackOpaque;
typedef struct { void *_0; } SlintSharedStringOpaque;

// SharedString
void slint_shared_string_from_bytes(SlintSharedStringOpaque *out,
                                     const char *bytes, uintptr_t len);
void slint_shared_string_clone(SlintSharedStringOpaque *out,
                                const SlintSharedStringOpaque *src);
void slint_shared_string_drop(SlintSharedStringOpaque *handle);
const char *slint_shared_string_bytes(const SlintSharedStringOpaque *handle);

// Properties
void slint_property_init(SlintPropertyHandleOpaque *out);
void slint_property_drop(SlintPropertyHandleOpaque *handle);
void slint_property_update(const SlintPropertyHandleOpaque *handle, void *val);
void slint_property_set_changed(const SlintPropertyHandleOpaque *handle,
                                 const void *value);
void slint_property_set_binding(const SlintPropertyHandleOpaque *handle,
                                 void (*binding)(void *user_data, void *ret),
                                 void *user_data,
                                 void (*drop_user_data)(void *),
                                 bool (*intercept_set)(void *, const void *),
                                 bool (*intercept_set_binding)(void *, void *));

// Callbacks
void slint_callback_init(SlintCallbackOpaque *out);
void slint_callback_drop(SlintCallbackOpaque *handle);
void slint_callback_set_handler(const SlintCallbackOpaque *handle,
                                 void (*handler)(void *user_data,
                                                 const void *arg,
                                                 void *ret),
                                 void *user_data,
                                 void (*drop_user_data)(void *));
void slint_callback_call(const SlintCallbackOpaque *handle,
                          const void *arg, void *ret);

// Window
void slint_windowrc_init(SlintWindowAdapterRcOpaque *out);
void slint_windowrc_clone(const SlintWindowAdapterRcOpaque *src,
                           SlintWindowAdapterRcOpaque *out);
void slint_windowrc_drop(SlintWindowAdapterRcOpaque *handle);
void slint_windowrc_show(const SlintWindowAdapterRcOpaque *handle);
void slint_windowrc_hide(const SlintWindowAdapterRcOpaque *handle);

// Event loop
void slint_run_event_loop(bool quit_on_last_window_closed);
void slint_quit_event_loop(void);
void slint_post_event(void (*callback)(void *user_data),
                       void *user_data,
                       void (*drop_user_data)(void *));

// Interpreter (when interpreter feature is enabled)
// ... slint_interpreter_value_*, slint_interpreter_component_compiler_*, etc.

#endif
```

### FFI Surface

The core `extern "C"` functions that need to be declared are well-organized
by subsystem. Approximate counts:

| Subsystem          | Functions | Source                            |
|--------------------|-----------|-----------------------------------|
| SharedString       | ~5        | `internal/core/` + `api/cpp/lib.rs` |
| Property           | ~8        | `internal/core/properties/ffi.rs` |
| Callback           | ~4        | `internal/core/` FFI modules      |
| Window             | ~10       | `internal/core/window.rs` ffi mod |
| Event loop / Timer | ~5        | `api/cpp/lib.rs`                  |
| Interpreter values | ~20       | `internal/interpreter/ffi.rs`     |
| Interpreter compiler | ~10     | `internal/interpreter/ffi.rs`     |

Total: ~60 C function declarations in the bridging header.

## Type Mapping

### Primitive Types

| Slint Type         | C FFI Type   | Swift Type           |
|--------------------|-------------|----------------------|
| `int`              | `int32_t`   | `Int32`              |
| `float`            | `float`     | `Float`              |
| `bool`             | `bool`      | `Bool`               |
| `string`           | opaque      | `String` (bridged)   |
| `color`            | `uint32_t` (ARGB) | `SlintColor`   |
| `brush`            | opaque      | `SlintBrush`         |
| `image`            | opaque      | `SlintImage`         |
| `length`           | `float`     | `Float`              |
| `duration`         | `int64_t` (ms) | `Duration` / `Int64` |
| `physical-length`  | `float`     | `Float`              |

### Core Wrapper Types

#### SlintString

A Swift class wrapping the opaque `SharedString` via C FFI calls:

```swift
public final class SlintString {
    var handle: SlintSharedStringOpaque

    public init(_ string: String) {
        handle = SlintSharedStringOpaque()
        string.withCString { ptr in
            slint_shared_string_from_bytes(&handle, ptr, string.utf8.count)
        }
    }

    public init(cloning other: SlintString) {
        handle = SlintSharedStringOpaque()
        slint_shared_string_clone(&handle, &other.handle)
    }

    deinit {
        slint_shared_string_drop(&handle)
    }

    public var stringValue: String {
        String(cString: slint_shared_string_bytes(&handle))
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
```

#### SlintProperty\<T\>

A generic wrapper around the property FFI. Uses Swift closures for bindings:

```swift
public final class SlintProperty<T> {
    private var handle: SlintPropertyHandleOpaque
    private var value: T

    public init(defaultValue: T) {
        handle = SlintPropertyHandleOpaque()
        value = defaultValue
        slint_property_init(&handle)
    }

    deinit {
        slint_property_drop(&handle)
    }

    public func get() -> T {
        slint_property_update(&handle, &value)
        return value
    }

    public func set(_ newValue: T) {
        value = newValue
        slint_property_set_changed(&handle, &value)
    }

    public func setBinding(_ binding: @escaping () -> T) {
        // Wrap the Swift closure into C function pointer + user_data
        let context = BindingContext(binding: binding)
        let unmanaged = Unmanaged.passRetained(context)

        slint_property_set_binding(
            &handle,
            { userData, retPtr in
                let ctx = Unmanaged<BindingContext<T>>
                    .fromOpaque(userData!).takeUnretainedValue()
                let result = ctx.binding()
                retPtr!.assumingMemoryBound(to: T.self).pointee = result
            },
            unmanaged.toOpaque(),
            { userData in
                Unmanaged<AnyObject>.fromOpaque(userData!).release()
            },
            nil,  // intercept_set
            nil   // intercept_set_binding
        )
    }
}

private class BindingContext<T> {
    let binding: () -> T
    init(binding: @escaping () -> T) { self.binding = binding }
}
```

A property wrapper provides idiomatic access in generated code:

```swift
@propertyWrapper
public struct SlintPropertyWrapper<T> {
    private let property: SlintProperty<T>

    public var wrappedValue: T {
        get { property.get() }
        set { property.set(newValue) }
    }

    public var projectedValue: SlintProperty<T> { property }
}
```

#### SlintCallback

Wraps the callback FFI, bridging to Swift closures:

```swift
public final class SlintCallback<Args, Ret> {
    private var handle: SlintCallbackOpaque

    public init() {
        handle = SlintCallbackOpaque()
        slint_callback_init(&handle)
    }

    deinit {
        slint_callback_drop(&handle)
    }

    public func setHandler(_ handler: @escaping (Args) -> Ret) {
        let context = Unmanaged.passRetained(
            CallbackContext(handler: handler)
        )
        slint_callback_set_handler(
            &handle,
            { userData, argPtr, retPtr in
                let ctx = Unmanaged<CallbackContext<Args, Ret>>
                    .fromOpaque(userData!).takeUnretainedValue()
                let args = argPtr!.assumingMemoryBound(to: Args.self).pointee
                let result = ctx.handler(args)
                retPtr!.assumingMemoryBound(to: Ret.self).pointee = result
            },
            context.toOpaque(),
            { userData in
                Unmanaged<AnyObject>.fromOpaque(userData!).release()
            }
        )
    }

    public func invoke(_ args: Args) -> Ret {
        var args = args
        var result: Ret!
        slint_callback_call(&handle, &args, &result)
        return result
    }
}

private class CallbackContext<Args, Ret> {
    let handler: (Args) -> Ret
    init(handler: @escaping (Args) -> Ret) { self.handler = handler }
}
```

#### SlintModel Protocol

The model interface as a Swift protocol, with a concrete array-backed
implementation:

```swift
public protocol SlintModel: AnyObject {
    associatedtype Element

    var rowCount: Int { get }
    func rowData(at index: Int) -> Element?
    func setRowData(at index: Int, data: Element)

    func notifyRowChanged(_ index: Int)
    func notifyRowAdded(_ index: Int, count: Int)
    func notifyRowRemoved(_ index: Int, count: Int)
    func notifyReset()
}

public final class SlintArrayModel<T>: SlintModel {
    public typealias Element = T
    private var storage: [T]

    public init(_ elements: [T] = []) {
        self.storage = elements
    }

    public var rowCount: Int { storage.count }

    public func rowData(at index: Int) -> T? {
        guard index >= 0, index < storage.count else { return nil }
        return storage[index]
    }

    public func setRowData(at index: Int, data: T) {
        guard index >= 0, index < storage.count else { return }
        storage[index] = data
        notifyRowChanged(index)
    }

    public func append(_ element: T) {
        let index = storage.count
        storage.append(element)
        notifyRowAdded(index, count: 1)
    }

    public func remove(at index: Int) {
        storage.remove(at: index)
        notifyRowRemoved(index, count: 1)
    }

    // Notification methods call into FFI to inform the Slint runtime
    public func notifyRowChanged(_ index: Int) { /* FFI call */ }
    public func notifyRowAdded(_ index: Int, count: Int) { /* FFI call */ }
    public func notifyRowRemoved(_ index: Int, count: Int) { /* FFI call */ }
    public func notifyReset() { /* FFI call */ }
}
```

### Struct Types

Slint structs (declared with `struct` in `.slint` files) are generated as Swift
structs:

```
// .slint
export struct TodoItem {
    title: string,
    completed: bool,
}
```

Generated Swift:

```swift
public struct TodoItem {
    public var title: String
    public var completed: Bool

    public init(title: String = "", completed: Bool = false) {
        self.title = title
        self.completed = completed
    }
}
```

### Enum Types

Slint enums map to Swift enums:

```
// .slint
export enum Priority {
    low,
    medium,
    high,
}
```

Generated Swift:

```swift
public enum Priority: Int32 {
    case low
    case medium
    case high
}
```

## Code Generator Design

### New Output Format

Add `OutputFormat::Swift` to the compiler's `OutputFormat` enum in
`internal/compiler/generator.rs`:

```rust
#[derive(Clone, Debug, PartialEq)]
pub enum OutputFormat {
    #[cfg(feature = "cpp")]
    Cpp(cpp::Config),
    #[cfg(feature = "rust")]
    Rust,
    Interpreter,
    Llr,
    #[cfg(feature = "python")]
    Python,
    #[cfg(feature = "swift")]
    Swift,
}
```

The extension mapping in `guess_from_extension`:

```rust
#[cfg(feature = "swift")]
Some("swift") => Some(Self::Swift),
```

### Generator Module

Create `internal/compiler/generator/swift.rs` following the pattern of `cpp.rs`
(5000+ lines). The generator:

1. Lowers the compiled document to LLR (same as C++ and Python generators)
2. Emits Swift source code for each exported component

**What the generator produces for each component:**

```swift
// Generated from MyApp.slint

import Slint

@MainActor
public final class MyApp {
    private let inner = SlintComponentHandle()

    public init() {
        inner.create(/* item tree VTable */)
    }

    // Properties — direct get/set via FFI
    public var greeting: String {
        get {
            var result = SlintSharedStringOpaque()
            inner.getProperty(offset: /* computed */, value: &result)
            defer { slint_shared_string_drop(&result) }
            return String(cString: slint_shared_string_bytes(&result))
        }
        set {
            newValue.withCString { ptr in
                var str = SlintSharedStringOpaque()
                slint_shared_string_from_bytes(&str, ptr, newValue.utf8.count)
                inner.setProperty(offset: /* computed */, value: &str)
                slint_shared_string_drop(&str)
            }
        }
    }

    // Callbacks — closure-based handler registration
    public func onButtonClicked(_ handler: @escaping () -> Void) {
        inner.setCallbackHandler(offset: /* computed */, handler: handler)
    }

    public func invokeButtonClicked() {
        inner.invokeCallback(offset: /* computed */)
    }

    // Window management
    public func show() throws {
        slint_windowrc_show(&inner.windowHandle)
    }

    public func hide() throws {
        slint_windowrc_hide(&inner.windowHandle)
    }

    public func run() throws {
        try show()
        slint_run_event_loop(true)
        try hide()
    }
}
```

**Global singletons** (declared with `export global` in `.slint`) generate as
nested types:

```swift
extension MyApp {
    @MainActor
    public struct AppLogic {
        fileprivate let component: MyApp

        public var theme: String {
            get { /* FFI property access */ }
            set { /* FFI property set */ }
        }
    }

    public var appLogic: AppLogic { AppLogic(component: self) }
}
```

### Generator Architecture

The Swift generator mirrors the C++ generator's structure:

| C++ Generator Function       | Swift Generator Equivalent     | Purpose                          |
|-------------------------------|-------------------------------|----------------------------------|
| `generate()`                  | `generate()`                  | Entry point, produces Swift file |
| `generate_component()`        | `generate_component()`        | Per-component class              |
| `generate_sub_component()`    | `generate_sub_component()`    | Internal sub-components          |
| `generate_global()`           | `generate_global()`           | Global singletons                |
| `compile_expression()`        | `compile_expression()`        | Expression to Swift code          |
| `property_set_value_code()`   | `property_set_value_code()`   | Property setter logic            |
| `property_set_binding_code()` | `property_set_binding_code()` | Binding setup                    |

### Keyword Handling

Swift reserved words need escaping (like `is_cpp_keyword` in `cpp.rs`):

```rust
fn is_swift_keyword(word: &str) -> bool {
    matches!(word,
        "as" | "break" | "case" | "catch" | "class" | "continue" |
        "default" | "defer" | "do" | "else" | "enum" | "extension" |
        "fallthrough" | "false" | "for" | "func" | "guard" | "if" |
        "import" | "in" | "init" | "inout" | "internal" | "is" |
        "let" | "nil" | "operator" | "private" | "protocol" |
        "public" | "repeat" | "rethrows" | "return" | "self" |
        "Self" | "static" | "struct" | "subscript" | "super" |
        "switch" | "throw" | "throws" | "true" | "try" | "typealias" |
        "var" | "where" | "while"
    )
}

fn ident(name: &str) -> String {
    let name = name.replace('-', "_");
    if is_swift_keyword(&name) {
        format!("`{}`", name)  // Swift uses backticks for keyword escaping
    } else {
        name
    }
}
```

### Naming Conventions

The generator converts Slint's kebab-case identifiers to Swift's camelCase:

| Slint Identifier        | Swift Identifier       |
|-------------------------|------------------------|
| `my-property`           | `myProperty`           |
| `button-clicked`        | `buttonClicked`        |
| `MyComponent`           | `MyComponent` (unchanged) |
| `background-color`      | `backgroundColor`      |

The existing `to_pascal_case` utility in `generator.rs` can be extended with a
`to_camel_case` function.

## Build Integration

### Swift Package Manager

The primary build system integration is via Swift Package Manager (SPM). The
package structure:

```
api/swift/
├── Package.swift
├── Sources/
│   ├── Slint/
│   │   ├── Slint.swift              # Core Swift API
│   │   ├── SlintString.swift        # SharedString wrapper
│   │   ├── SlintProperty.swift      # Property wrapper
│   │   ├── SlintCallback.swift      # Callback wrapper
│   │   ├── SlintModel.swift         # Model protocol + ArrayModel
│   │   ├── SlintWindow.swift        # Window management
│   │   ├── SlintTimer.swift         # Timer management
│   │   └── SlintEventLoop.swift     # Event loop
│   ├── SlintInterpreter/
│   │   ├── SlintCompiler.swift      # ComponentCompiler wrapper
│   │   ├── SlintComponentDefinition.swift
│   │   ├── SlintComponentInstance.swift
│   │   └── SlintValue.swift         # Dynamic Value type
│   └── SlintCBridge/
│       ├── include/
│       │   ├── module.modulemap
│       │   └── SlintCore.h          # C declarations of Rust FFI functions
│       └── shim.c                   # Empty file required by SPM for C target
├── Tests/
│   └── SlintTests/
│       └── BasicTests.swift
└── Cargo.toml                       # Rust crate producing the static library
```

**Package.swift:**

```swift
// swift-tools-version: 6.2
import PackageDescription

let package = Package(
    name: "Slint",
    platforms: [.macOS(.v13), .iOS(.v16)],
    products: [
        .library(name: "Slint", targets: ["Slint"]),
        .library(name: "SlintInterpreter", targets: ["SlintInterpreter"]),
    ],
    targets: [
        // Binary target: pre-built Rust static library
        .binaryTarget(
            name: "SlintRustLib",
            path: "SlintRustLib.xcframework"
        ),

        // C bridging header (declares Rust extern "C" symbols)
        .target(
            name: "SlintCBridge",
            dependencies: ["SlintRustLib"],
            path: "Sources/SlintCBridge",
            publicHeadersPath: "include"
        ),

        // Core Swift API
        .target(
            name: "Slint",
            dependencies: ["SlintCBridge"],
            path: "Sources/Slint"
        ),

        // Interpreter API
        .target(
            name: "SlintInterpreter",
            dependencies: ["Slint"],
            path: "Sources/SlintInterpreter"
        ),

        .testTarget(
            name: "SlintTests",
            dependencies: ["Slint", "SlintInterpreter"]
        ),
    ]
)
```

Note: no `.interoperabilityMode(.Cxx)` setting is needed. The `Slint` target
depends only on `SlintCBridge` (a pure C target), and Swift imports C headers
natively.

### Rust Crate

The `api/swift/Cargo.toml` mirrors `api/cpp/Cargo.toml`:

```toml
[package]
name = "slint-swift"
version.workspace = true

[lib]
crate-type = ["staticlib"]

[features]
renderer-femtovg = ["i-slint-backend-selector/renderer-femtovg"]
renderer-skia = ["i-slint-backend-selector/renderer-skia"]
renderer-software = ["i-slint-backend-selector/renderer-software"]
backend-winit = ["i-slint-backend-selector/backend-winit"]
interpreter = ["slint-interpreter"]

default = ["std", "backend-winit", "renderer-skia"]

std = [
    "i-slint-core/default",
    "i-slint-backend-selector",
]

[dependencies]
i-slint-core = { workspace = true, features = ["ffi"] }
i-slint-backend-selector = { workspace = true, optional = true }
slint-interpreter = { workspace = true, features = ["ffi"], optional = true }
```

The library produces a static library (`libslint_swift.a`) that gets packaged
into an XCFramework for distribution:

```sh
# Build for iOS device and simulator
cargo build --target aarch64-apple-ios --release -p slint-swift
cargo build --target aarch64-apple-ios-sim --release -p slint-swift
cargo build --target x86_64-apple-ios --release -p slint-swift

# Build for macOS
cargo build --target aarch64-apple-darwin --release -p slint-swift
cargo build --target x86_64-apple-darwin --release -p slint-swift

# Build for Linux (no XCFramework, link directly)
cargo build --release -p slint-swift

# Create XCFramework (Apple platforms only)
xcodebuild -create-xcframework \
    -library target/aarch64-apple-ios/release/libslint_swift.a \
    -library target/aarch64-apple-ios-sim/release/libslint_swift.a \
    -library target/aarch64-apple-darwin/release/libslint_swift.a \
    -output SlintRustLib.xcframework
```

### Cross-Platform Support

All platforms are first-class targets. The pure C bridging approach works on
every platform Swift supports:

| Platform    | Swift             | Slint Backend          | Build System   |
|-------------|-------------------|------------------------|----------------|
| **macOS**   | Native            | Winit + Skia/FemtoVG   | SPM / Xcode   |
| **iOS**     | Native            | Winit + Skia           | SPM / Xcode   |
| **Linux**   | swift.org builds  | Winit + Skia/FemtoVG   | SPM            |
| **Windows** | swift.org builds  | Winit + Skia/FemtoVG   | SPM            |
| **Android** | Community builds  | Winit + Skia           | SPM + Gradle   |

On Apple platforms, the Rust static library is distributed as an XCFramework.
On Linux, Windows, and Android, the static library is linked directly via SPM's
`linkerSettings`.

Android support requires the [Swift Android SDK](https://github.com/AntranCodes/Swift-Android-SDK)
or similar community toolchain. The existing Slint Android backend
(`internal/backends/android-activity/`) provides the Winit + Skia integration
that the Swift binding calls into via the same C FFI.

### Xcode Integration

For Xcode projects (non-SPM), the integration follows the existing iOS pattern
from `scripts/build_for_ios_with_cargo.bash` but adds a code generation step:

1. **Build script phase** — Compiles the Rust static library via Cargo
2. **Code generation phase** — Runs `slint-compiler --format swift` to generate
   Swift source files from `.slint` markup
3. **Compile phase** — Xcode compiles the generated Swift files alongside user code

### Build Script (slint-build for Swift)

Similar to `api/rs/build/lib.rs` for Rust, a build tool generates Swift code
at build time:

```sh
# CLI usage
slint-compiler --format swift -o Generated/ path/to/app.slint

# This generates:
# Generated/App.swift     — component classes
# Generated/AppTypes.swift — struct and enum types
```

## Interpreter Support

The interpreter API allows loading `.slint` files at runtime without ahead-of-time
compilation. This wraps the `extern "C"` functions from `internal/interpreter/ffi.rs`.

### Swift Interpreter API

```swift
import SlintInterpreter

// Compile from source
let compiler = SlintCompiler()
compiler.includePaths = ["/path/to/includes"]
compiler.style = "fluent"

let definition = try compiler.buildFromSource("""
    export component MyApp inherits Window {
        in property <string> greeting: "Hello";
        Text { text: greeting; }
    }
""")

// Create instance
let instance = definition.create()!
instance.setProperty("greeting", .string("Hello from Swift!"))
instance.show()
SlintEventLoop.run()
```

### Value Type

The interpreter's `Value` type maps to a Swift enum (mirroring the
`ValueType` enum from `internal/interpreter/ffi.rs`):

```swift
public enum SlintValue {
    case void
    case number(Double)
    case string(String)
    case bool(Bool)
    case image(SlintImage)
    case model(any SlintModel)
    case brush(SlintBrush)
    case `struct`(SlintStruct)

    // Convert to/from the FFI Box<Value> via
    // slint_interpreter_value_new_*, slint_interpreter_value_to_*, etc.
}
```

### Interpreter Wrapper Classes

```swift
@MainActor
public final class SlintCompiler {
    private var handle: SlintComponentCompilerOpaque

    public init() { /* slint_interpreter_component_compiler_new */ }
    deinit { /* slint_interpreter_component_compiler_drop */ }

    public var includePaths: [String] { get { ... } set { ... } }
    public var style: String? { get { ... } set { ... } }

    public func buildFromSource(_ source: String) throws -> SlintComponentDefinition {
        // slint_interpreter_component_compiler_build_from_source
    }

    public func buildFromPath(_ path: String) throws -> SlintComponentDefinition {
        // slint_interpreter_component_compiler_build_from_path
    }
}

@MainActor
public final class SlintComponentDefinition { ... }

@MainActor
public final class SlintComponentInstance {
    public func setProperty(_ name: String, _ value: SlintValue) { ... }
    public func getProperty(_ name: String) -> SlintValue { ... }
    public func setCallback(_ name: String, _ handler: @escaping ([SlintValue]) -> SlintValue) { ... }
    public func invokeCallback(_ name: String, _ args: [SlintValue]) -> SlintValue { ... }
    public func show() { ... }
    public func hide() { ... }
}
```

## Platform Adapter

Swift apps can implement custom platform adapters for cases where the built-in
Winit backend is not suitable (e.g., embedding Slint in an existing UIKit or
AppKit view hierarchy).

### WindowAdapter Protocol

```swift
@MainActor
public protocol SlintWindowAdapter: AnyObject {
    /// Return the renderer (SoftwareRenderer or SkiaRenderer)
    var renderer: SlintRenderer { get }

    /// Return the current window size in physical pixels
    var size: SlintPhysicalSize { get }

    /// Called when Slint needs the window to be repainted
    func requestRedraw()

    /// Called when Slint wants to change the window visibility
    func setVisible(_ visible: Bool)

    /// Called when Slint wants to change the window size
    func setSize(_ size: SlintLogicalSize)

    /// Called when window properties (title, etc.) change
    func updateWindowProperties(_ properties: SlintWindowProperties)
}
```

### Platform Protocol

```swift
@MainActor
public protocol SlintPlatform {
    func createWindowAdapter() throws -> SlintWindowAdapter
    func runEventLoop() throws
    func newEventLoopProxy() -> SlintEventLoopProxy?
}

// Set the platform before creating any windows
public func slintSetPlatform(_ platform: SlintPlatform) throws
```

### UIKit Integration Example

```swift
@MainActor
class SlintUIKitView: UIView, SlintWindowAdapter {
    private let skiaRenderer: SlintSkiaRenderer
    private var metalLayer: CAMetalLayer!

    init(frame: CGRect) {
        self.skiaRenderer = SlintSkiaRenderer()
        super.init(frame: frame)
        setupMetalLayer()
    }

    var renderer: SlintRenderer { skiaRenderer }

    var size: SlintPhysicalSize {
        let scale = UIScreen.main.scale
        return SlintPhysicalSize(
            width: UInt32(bounds.width * scale),
            height: UInt32(bounds.height * scale)
        )
    }

    func requestRedraw() {
        setNeedsDisplay()
    }

    override func draw(_ rect: CGRect) {
        skiaRenderer.render(/* ... */)
    }

    // Forward touch events to Slint
    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
        for touch in touches {
            let point = touch.location(in: self)
            window?.dispatchPointerPressEvent(
                position: SlintLogicalPosition(x: Float(point.x), y: Float(point.y)),
                button: .left
            )
        }
    }
}
```

### SwiftUI Integration

A `SlintView` SwiftUI wrapper enables embedding Slint components in SwiftUI:

```swift
public struct SlintView<Component>: UIViewRepresentable {
    let componentFactory: () -> Component

    public func makeUIView(context: Context) -> SlintUIKitView {
        let view = SlintUIKitView(frame: .zero)
        // Initialize and attach Slint component
        return view
    }

    public func updateUIView(_ uiView: SlintUIKitView, context: Context) {
        // Handle SwiftUI state changes
    }
}

// Usage in SwiftUI
struct ContentView: View {
    var body: some View {
        SlintView { MySlintApp() }
            .frame(width: 300, height: 400)
    }
}
```

## Memory Management

The C FFI uses manual memory management (init/drop pairs). The Swift wrappers
use `deinit` to ensure cleanup:

```swift
// Every opaque handle follows this pattern:
public final class SlintResource {
    private var handle: OpaqueHandle

    init() {
        handle = OpaqueHandle()
        slint_resource_init(&handle)      // Rust allocates
    }

    deinit {
        slint_resource_drop(&handle)      // Rust deallocates
    }
}
```

For callbacks that cross the FFI boundary, `Unmanaged<T>` is used to convert
Swift objects to/from `void *user_data` pointers, with explicit retain/release:

- `Unmanaged.passRetained(obj).toOpaque()` — retain + convert to `void *`
- `Unmanaged<T>.fromOpaque(ptr).takeUnretainedValue()` — use without consuming
- The `drop_user_data` C function pointer calls `Unmanaged.fromOpaque(ptr).release()`

## Concurrency

Slint is single-threaded (main thread only). The Swift API enforces this with
`@MainActor` and provides both synchronous and asynchronous interfaces.

### @MainActor Enforcement

All Slint types are annotated `@MainActor` to prevent accidental cross-thread
access at compile time:

```swift
@MainActor
public final class MyComponent { ... }

@MainActor
public final class SlintCompiler { ... }
```

### Synchronous API

The blocking event loop for simple apps:

```swift
@MainActor
public func slintRunEventLoop() { ... }

// Cross-thread communication
public func slintInvokeFromEventLoop(_ callback: @escaping @MainActor () -> Void) {
    // Wraps slint_post_event to dispatch onto the main thread
}
```

### Asynchronous API

An `async` variant integrates with Swift's structured concurrency. This is the
recommended API for modern Swift applications:

```swift
@MainActor
public enum SlintEventLoop {
    /// Run the event loop synchronously (blocks until quit)
    public static func run() throws {
        slint_run_event_loop(true)
    }

    /// Run the event loop asynchronously
    public static func run() async throws {
        try await withCheckedThrowingContinuation { continuation in
            // Run the blocking event loop on a background thread,
            // using slint_post_event to marshal results back
            Task.detached {
                do {
                    await MainActor.run {
                        slint_run_event_loop(true)
                    }
                    continuation.resume()
                } catch {
                    continuation.resume(throwing: error)
                }
            }
        }
    }

    /// Quit the event loop
    public static func quit() {
        slint_quit_event_loop()
    }
}
```

Component lifecycle with async:

```swift
@main
struct MyApp {
    static func main() async throws {
        let app = await MySlintComponent()
        app.greeting = "Hello from Swift!"
        try await app.show()
        try await SlintEventLoop.run()
    }
}
```

Async callback handlers:

```swift
// Callbacks can bridge to async contexts
app.onDataRequested { query in
    Task { @MainActor in
        let result = await fetchData(query)
        app.displayData = result
    }
}
```

Async property observation (built on top of property bindings):

```swift
// Observe property changes as an AsyncSequence
for await newValue in app.$counter.values {
    print("Counter changed to: \(newValue)")
}
```

## Testing

### Test Driver

Add `test-driver-swift` following the pattern of existing test drivers. The test
`.slint` files already embed language-specific test code in comments. Add a new
marker for Swift:

````
// .slint test file
export component TestCase {
    in-out property <bool> test: true;
    // ...
}

/*
```swift
let instance = TestCase()
assert(instance.test == true)
instance.test = false
assert(instance.test == false)
```
*/
````

### Test Execution

```sh
# Run Swift tests
cargo test -p test-driver-swift

# Filtered
SLINT_TEST_FILTER=property cargo test -p test-driver-swift
```

The test driver:

1. Scans `.slint` test files for ` ```swift ` code blocks
2. Generates a Swift test file that imports the generated component
3. Compiles and runs via `swift test` or `xcrun swiftc`

## Directory Structure

```
api/swift/
├── Cargo.toml                  # Rust crate producing staticlib
├── lib.rs                      # Re-exports FFI functions (like api/cpp/lib.rs)
├── Package.swift               # Swift Package Manager manifest
├── Sources/
│   ├── Slint/                  # Core Swift API wrappers
│   ├── SlintInterpreter/       # Interpreter wrappers
│   └── SlintCBridge/           # C bridging header + modulemap
│       └── include/
│           └── SlintCore.h     # Declares Rust extern "C" symbols
├── Tests/
│   └── SlintTests/
└── scripts/
    └── build-xcframework.sh    # Builds XCFramework for distribution

internal/compiler/generator/
├── cpp.rs                      # Existing C++ generator
├── rust.rs                     # Existing Rust generator
├── python.rs                   # Existing Python generator
└── swift.rs                    # New Swift generator

tests/
└── driver/
    └── swift/                  # Swift test driver
```

## Phased Implementation Plan

### Phase 1: Foundation

- Create `api/swift/` directory with Cargo.toml producing a static library
- Write `SlintCore.h` bridging header declaring the core `extern "C"` functions
- Implement Swift wrappers: `SlintString`, `SlintColor`, `SlintBrush`, `SlintImage`
- Implement `SlintWindow`, `SlintTimer`, and `SlintEventLoop` (sync + async)
- Build and verify on macOS, iOS, and Linux with a manually written Swift example
- Set up `Package.swift` and XCFramework build script (Apple platforms)
- Set up direct static library linking for Linux, Windows, Android

### Phase 2: Code Generator

- Add `OutputFormat::Swift` to the compiler
- Implement `internal/compiler/generator/swift.rs`
  - Component class generation (properties, callbacks, window management)
  - Global singleton generation
  - Struct and enum type generation
  - Expression compilation (arithmetic, conditionals, string interpolation)
- Add `--format swift` support to the `slint-compiler` CLI tool
- Implement `SlintProperty<T>` and `SlintCallback` wrappers
- Implement `SlintModel` protocol and `SlintArrayModel`

### Phase 3: Interpreter

- Wrap the interpreter FFI (`SlintCompiler`, `SlintComponentDefinition`,
  `SlintComponentInstance`)
- Implement `SlintValue` type conversions between Swift and Slint
- Implement dynamic property access and callback invocation

### Phase 4: Platform Integration

- `SlintWindowAdapter` and `SlintPlatform` protocols
- UIKit view integration (`SlintUIKitView`)
- AppKit view integration (`SlintNSView`)
- SwiftUI wrapper (`SlintView`)
- Touch/mouse/keyboard event forwarding

### Phase 5: Testing and CI

- Add `test-driver-swift` for running `.slint` test files with Swift code blocks
- Add CI jobs for macOS, iOS, Linux, Windows, and Android builds
- Add Swift-specific examples to the `examples/` directory
- Documentation for the Swift API on the Slint website

### Phase 6: Polish and Distribution

- Publish the Swift package to a Swift package registry
- Provide pre-built XCFrameworks for tagged releases
- CocoaPods and Carthage support (if demand exists)
- Xcode project templates and Xcode plugin for `.slint` syntax highlighting

## Design Decisions

1. **No visionOS** — visionOS is not targeted in the initial implementation.

2. **Minimum Swift 6.2** — Requires Swift 6.2, which provides full strict
   concurrency checking, improved `@MainActor` inference, and mature
   async/await support.

3. **Async API provided** — Both synchronous and asynchronous APIs are offered.
   The async API (`await SlintEventLoop.run()`, async property observation) is
   the recommended path for modern Swift apps.

4. **All platforms are first-class** — macOS, iOS, Linux, Windows, and Android
   are all first-class targets from Phase 1. The pure C bridging approach
   ensures this works uniformly across platforms.

## Implementation Notes

### Phase 1: Foundation

Phase 1 establishes the Rust static library, C bridging header, SPM package
structure, and core Swift wrapper types. Here are the key implementation details.

#### File Structure

```
api/swift/
├── Cargo.toml                                 # Rust crate (staticlib)
├── lib.rs                                     # FFI re-exports + Swift-specific helpers
├── Package.swift                              # SPM manifest
├── Sources/
│   ├── Slint/
│   │   ├── SlintString.swift                  # SharedString wrapper
│   │   ├── SlintColor.swift                   # Color wrapper with HSVA/OKLCh
│   │   ├── SlintBrush.swift                   # Brush enum (solid color only)
│   │   ├── SlintImage.swift                   # Image wrapper (heap-allocated)
│   │   ├── SlintTimer.swift                   # Timer with closure bridging
│   │   ├── SlintWindow.swift                  # Window management (@MainActor)
│   │   └── SlintEventLoop.swift               # Event loop (sync + async)
│   └── SlintCBridge/
│       ├── include/
│       │   ├── SlintCore.h                    # C declarations of FFI functions
│       │   └── module.modulemap               # Clang module map
│       └── shim.c                             # Empty file required by SPM
```

#### Rust Crate (`slint-swift`)

The Rust crate at `api/swift/` follows the pattern of `api/cpp/`:

- `crate-type = ["staticlib"]` produces `libslint_swift.a`
- Depends on `i-slint-core` with `features = ["ffi"]` and `i-slint-backend-selector`
- Re-exports `slint_windowrc_init`, `slint_ensure_backend`, `slint_run_event_loop`,
  `slint_quit_event_loop`, `slint_post_event` (same as `api/cpp/lib.rs`)
- Adds Swift-specific FFI functions: `slint_swift_image_new`, `slint_swift_image_drop`,
  `slint_swift_image_clone`, `slint_swift_image_load_from_path`
- Feature flags mirror the C++ crate for renderer/backend selection

Build: `cargo build --lib -p slint-swift`

#### Image Heap Allocation Strategy

The Rust `Image` type wraps `ImageInner`, a `#[repr(u8)]` enum whose size varies
by compile-time features (SVG support, WGPU textures, etc.). Rather than
embedding an opaque byte buffer of a guessed size in the C header (fragile and
platform-dependent), Phase 1 uses a **heap-allocated box** approach:

```c
// C header declares Image as an opaque forward-declared struct
typedef struct SlintImageOpaque SlintImageOpaque;

// Rust allocates/frees via Box<Image>
SlintImageOpaque *slint_swift_image_new(void);
void slint_swift_image_drop(SlintImageOpaque *image);
SlintImageOpaque *slint_swift_image_clone(const SlintImageOpaque *image);
SlintImageOpaque *slint_swift_image_load_from_path(const SlintSharedStringOpaque *path);
```

Swift holds an `OpaquePointer` to the Rust-managed heap allocation. This
approach is size-agnostic and works across all platforms without compile-time
size knowledge.

The existing `slint_image_size()`, `slint_image_path()`,
`slint_image_compare_equal()`, and `slint_image_set_nine_slice_edges()` functions
from `internal/core/graphics/image.rs` accept `*const Image` / `*mut Image`
directly, so the heap pointer is passed through.

#### Swift 6.2 Concurrency Compliance

Swift 6.2's strict concurrency checking required several patterns:

1. **`@preconcurrency import SlintCBridge`** — C structs imported from the bridging
   header are not `Sendable` by default. The `@preconcurrency` annotation treats
   Sendable-related errors from the C module as warnings, allowing `deinit` to
   access C-typed fields.

2. **`@MainActor` on `SlintWindow`** — Window operations must happen on the main
   thread. The `@MainActor` annotation enforces this at compile time.

3. **`@unchecked Sendable` on callback boxes** — Closure boxes used for FFI
   callback bridging (`CallbackBox`, `EventCallbackBox`) are marked
   `@unchecked Sendable` because they are only ever invoked on the main thread
   by the Slint event loop, but the Swift compiler cannot verify this statically.

#### Closure Bridging Pattern

Timer callbacks and event loop posting use the same pattern to convert Swift
closures into C function pointers:

```swift
// 1. Box the closure in a reference type
let box_ = CallbackBox(closure)

// 2. Retain and convert to raw pointer
let context = Unmanaged.passRetained(box_).toOpaque()

// 3. Pass to FFI with C-compatible function pointers
slint_timer_start(0, mode, interval,
    { ptr in Unmanaged<CallbackBox>.fromOpaque(ptr!).takeUnretainedValue().closure() },
    context,
    { ptr in Unmanaged<CallbackBox>.fromOpaque(ptr!).release() })
```

The `drop_user_data` function pointer releases the Swift reference when Rust
no longer needs the callback.

#### Opaque Type Sizes

| C Type                       | Rust Type                    | Size (64-bit) |
|------------------------------|------------------------------|---------------|
| `SlintSharedStringOpaque`    | `SharedString` (= `SharedVector<u8>`) | 1 pointer |
| `SlintWindowAdapterRcOpaque` | `Rc<dyn WindowAdapter>`      | 2 pointers    |
| `SlintColor`                 | `Color` (`#[repr(C)]`)       | 4 bytes       |
| `SlintImageOpaque`           | `Box<Image>` (heap pointer)  | 1 pointer     |
| `SlintIntSize`               | `euclid::Size2D<u32>`        | 8 bytes       |

#### Build Verification

```sh
# Build Rust static library
cargo build --lib -p slint-swift

# Verify FFI symbols are exported
nm target/debug/libslint_swift.a | grep "T _slint_shared_string"

# Build Swift package (compiles Swift wrappers against C header)
cd api/swift && swift build
```

Note: `swift build` compiles the Swift wrapper types against the C bridging
header but does not link against the Rust static library. Full end-to-end
linking requires either an XCFramework (Apple platforms) or direct linker flags
pointing to `libslint_swift.a` plus its transitive dependencies.

### Phase 2: Properties, Callbacks, and Models

Phase 2 adds reactive properties, callbacks, and the model protocol — the three
primitives needed to connect Swift data to a Slint UI.

#### File Structure (additions to Phase 1)

```
api/swift/Sources/Slint/
├── SlintProperty.swift      # SlintProperty<T> — reactive property with binding support
├── SlintCallback.swift      # SlintCallback — parameter-less callback
└── SlintModel.swift         # SlintModel protocol + SlintArrayModel<T>
```

#### SlintProperty\<T\> — Non-generic FFI thunks

`SlintProperty<T>` is generic over the value type `T`, but the Rust FFI uses raw
`*mut c_void` for values. The challenge is that `@convention(c)` function pointers
cannot capture generic type parameters, so the binding trampoline cannot be a
generic function.

The solution is a **type-erased box** at file scope:

```swift
// File-scope @convention(c) trampoline — not generic, so it compiles to a
// single C function pointer shared by all SlintProperty<T> instances.
private let propertyBindingInvoke: @convention(c) (
    UnsafeMutableRawPointer?, UnsafeMutableRawPointer?
) -> Void = { userData, retPtr in
    Unmanaged<PropertyBindingBox>.fromOpaque(userData!)
        .takeUnretainedValue()
        .invoke(retPtr!)   // PropertyBindingBox.invoke captures T at construction time
}

private final class PropertyBindingBox {
    let invoke: (UnsafeMutableRawPointer) -> Void
    init(_ invoke: @escaping (UnsafeMutableRawPointer) -> Void) {
        self.invoke = invoke
    }
}
```

Inside `SlintProperty<T>.setBinding`, the generic type is captured in the closure
passed to `PropertyBindingBox.init`:

```swift
let box = PropertyBindingBox { retPtr in
    let result: T = binding()                         // T is captured here
    retPtr.assumingMemoryBound(to: T.self).pointee = result
}
```

This pattern avoids generic `@convention(c)` closures entirely while keeping full
type safety inside Swift.

#### Value Storage Stability

The property FFI requires a stable memory address for the value buffer — Rust
stores a pointer to the value and dereferences it on each `slint_property_update`
call. `SlintProperty<T>` is a `final class`, so its stored properties live on the
heap. The backing store is named `storage` (to avoid collision with the public
`value` computed property) and its address remains constant for the object's
lifetime.

#### SlintCallback — Void Args and Return

The Rust callback FFI takes `arg: *const c_void` and `ret: *mut c_void`.
For a parameter-less, void-returning callback the caller must still pass valid
(non-null) pointers because the generated trampoline dereferences them. Two
separate single-byte dummy variables are used:

```swift
var argDummy: UInt8 = 0
var retDummy: UInt8 = 0
slint_callback_call(&handle, &argDummy, &retDummy)
```

Using the same variable for both `arg` and `ret` causes a Swift exclusivity
violation (`&dummy` passed to two inout parameters simultaneously), so separate
variables are required.

#### SlintModel Protocol — Subscript API

`SlintModel` exposes element access via a single subscript rather than separate
`rowData(at:)` / `setRowData(at:data:)` methods, which is idiomatic Swift:

```swift
public protocol SlintModel<Element>: AnyObject {
    subscript(index: Int) -> Element? { get set }
    // ...
}
```

Out-of-bounds reads return `nil`; out-of-bounds writes and `nil` assignments are
silently ignored. This matches Swift's convention for optional-returning
subscripts on containers.

The notification methods (`notifyRowChanged`, `notifyRowAdded`,
`notifyRowRemoved`, `notifyReset`) have default no-op implementations in a
protocol extension so that custom `SlintModel` implementations only need to
override them when they are wired to the Slint runtime in Phase 3.

#### Opaque Type Sizes (Phase 2 additions)

| C Type                        | Rust Type              | Size (64-bit) |
|-------------------------------|------------------------|---------------|
| `SlintPropertyHandleOpaque`   | `PropertyHandle` (= `Cell<usize>`) | 1 pointer |
| `SlintCallbackOpaque`         | `Callback<()>`         | 2 pointers    |

Both types are behind `#[cfg(feature = "ffi")]` in `i-slint-core`, which is
already enabled by the `slint-swift` crate's `i-slint-core = { features = ["ffi"] }`
dependency.

#### Build Verification

```sh
# Build Rust static library (required before swift test)
cargo build --lib -p slint-swift

# Run all Swift tests (83 tests across 7 suites)
cd api/swift && swift test
```
