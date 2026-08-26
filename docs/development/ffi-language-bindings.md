# FFI & Language Bindings

> Note for AI coding assistants (agents):
> **When to load this document:** Working on `api/cpp/`, `api/node/`, `api/python/`,
> language bindings, cbindgen, FFI modules in `internal/`, or adding new cross-language APIs.
> For general build commands and project structure, see `/AGENTS.md`.

## Overview

Slint provides language bindings for C++, Node.js, and Python, all built on top of the Rust core. The FFI layer uses:

- **C++ bindings**: cbindgen-generated headers with manual C++ wrapper classes
- **Node.js bindings**: Neon/NAPI framework for native Node modules
- **Python bindings**: PyO3 with maturin build system
- **Internal FFI**: `#[no_mangle] extern "C"` functions in core crates

## Key Files

| File | Purpose |
|------|---------|
| `api/cpp/lib.rs` | Core C FFI exports (window, event loop, timers) |
| `api/cpp/cbindgen.rs` | C++ header generator (enums, structs, vtables) |
| `api/cpp/platform.rs` | Platform abstraction for C++ |
| `api/cpp/CMakeLists.txt` | CMake integration via Corrosion |
| `api/node/rust/lib.rs` | Neon/NAPI module entry point |
| `api/node/rust/interpreter/` | Interpreter bindings for Node.js |
| `api/python/slint/lib.rs` | PyO3 module initialization |
| `api/python/slint/interpreter.rs` | Interpreter bindings for Python |
| `internal/core/properties/ffi.rs` | Property system FFI |
| `internal/core/window.rs` | Window FFI in `ffi` module |
| `internal/core/item_tree.rs` | ItemTreeVTable definitions |
| `internal/interpreter/ffi.rs` | Interpreter value FFI |
| `internal/backends/testing/ffi.rs` | Testing backend FFI |

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                        Language APIs                                 │
├─────────────────┬─────────────────┬─────────────────────────────────┤
│   C++ (api/cpp) │ Node.js (api/node)│ Python (api/python)            │
│   cbindgen      │ Neon/NAPI        │ PyO3                           │
├─────────────────┴─────────────────┴─────────────────────────────────┤
│                     FFI Layer (extern "C")                           │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐ ┌─────────────┐   │
│  │ properties/ │ │ window.rs   │ │ item_tree.rs│ │ interpreter/│   │
│  │ ffi.rs      │ │ ffi module  │ │ VTables     │ │ ffi.rs      │   │
│  └─────────────┘ └─────────────┘ └─────────────┘ └─────────────┘   │
├─────────────────────────────────────────────────────────────────────┤
│                     Internal Rust Crates                             │
│  i-slint-core   i-slint-compiler   slint-interpreter                │
└─────────────────────────────────────────────────────────────────────┘
```

## C++ Bindings

### Structure

The C++ API consists of:
- **Generated headers**: Created by `cbindgen.rs` from Rust types
- **Hand-written headers**: C++ wrapper classes in `api/cpp/include/`
- **Rust FFI**: `extern "C"` functions in `api/cpp/lib.rs`

### FFI Function Pattern

```rust
// api/cpp/lib.rs
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_windowrc_init(out: *mut WindowAdapterRcOpaque) {
    // Size assertion for ABI safety
    assert_eq!(
        core::mem::size_of::<Rc<dyn WindowAdapter>>(),
        core::mem::size_of::<WindowAdapterRcOpaque>()
    );
    let win = with_platform(|b| b.create_window_adapter()).unwrap();
    unsafe {
        core::ptr::write(out as *mut Rc<dyn WindowAdapter>, win);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn slint_run_event_loop(quit_on_last_window_closed: bool) {
    with_platform(|b| {
        if !quit_on_last_window_closed {
            b.set_event_loop_quit_on_last_window_closed(false);
        }
        b.run_event_loop()
    }).unwrap();
}
```

### Opaque Pointer Types

Hide internal Rust types from C++. Each is a `#[repr(C)]` struct of the same size as the type it
stands for, so C++ can hold it by value without knowing its layout — `PropertyHandleOpaque` wraps
the real type, the other two are two pointer-sized fields with a static size assertion:

| Opaque type | Stands for | Defined in |
|-------------|------------|------------|
| `WindowAdapterRcOpaque` | `Rc<dyn WindowAdapter>` | `internal/core/window.rs` |
| `PropertyHandleOpaque` | `PropertyHandle` | `internal/core/properties/ffi.rs` |
| `CallbackOpaque` | `Callback` | `internal/core/callbacks.rs` |

### cbindgen Code Generation

`api/cpp/cbindgen.rs` generates the C++ headers. Enums and built-in structs are not read from
the Rust source at all: they are printed from the `for_each_enums!` and `for_each_builtin_structs!`
macros in `i-slint-common`, which are the single definition both the Rust types and the C++
headers come from. Everything else goes through cbindgen proper, with a rename table mapping Rust
names to their C++ equivalents (`Coord` to `float`, `StringArg` to `slint::SharedString`, ...) and
an exclude list for the types the hand-written headers declare themselves, `SharedString` among
them.

**Generated headers:**
- `slint_enums.h` / `slint_enums_internal.h` - Public/private enums
- `slint_builtin_structs.h` / `slint_builtin_structs_internal.h` - Structs
- `slint_string_internal.h` - SharedString, StyledText
- `slint_properties_internal.h` - Property system
- `slint_timer_internal.h` - Timer management
- Item VTables for UI elements

### CMake Integration

Uses Corrosion to bridge CMake and Cargo. `api/cpp/CMakeLists.txt` declares one CMake option per
Cargo feature with `define_cargo_feature(<cargo-feature> <description> <default>)`, or
`define_cargo_dependent_feature(...)` with a trailing condition for the ones that only make sense
in some configurations — most are conditioned on `NOT SLINT_FEATURE_FREESTANDING`, since a
bare-metal build has no windowing system.

Each becomes a `SLINT_FEATURE_<FEATURE>` option that maps back to `--features <feature>`, so
`SLINT_FEATURE_BACKEND_WINIT` turns into `--features backend-winit`.

### Building C++ Library

```sh
cargo build --lib -p slint-cpp

# With CMake
mkdir build && cd build
cmake -GNinja ..
cmake --build .
```

## Node.js Bindings

### Structure

Uses Neon/NAPI for Node.js native modules:

```
api/node/
├── rust/
│   ├── lib.rs              # Module entry point
│   ├── types/              # Type wrappers
│   │   ├── brush.rs
│   │   ├── image_data.rs
│   │   └── ...
│   └── interpreter/        # Interpreter bindings
│       ├── component_compiler.rs
│       ├── component_instance.rs
│       └── value.rs
├── Cargo.toml
└── package.json
```

### NAPI Function Pattern

A `#[napi]` attribute on a free function exports it to JavaScript, and on an enum or struct
exports the type:

```rust
#[napi]
pub fn mock_elapsed_time(_ms: f64) {
    #[cfg(feature = "testing")]
    i_slint_backend_testing::mock_elapsed_time(_ms as u64);
}

#[napi]
pub enum ProcessEventsResult {
    Continue,
    Exited,
}
```

Anything fallible returns `napi::Result` so the error surfaces as a JavaScript exception, which
means mapping the Rust error through `napi::Error::from_reason` — `process_events()` in the same
file does both. See `api/node/rust/lib.rs`.

### Type Bindings

Two shapes appear in `api/node/rust/types/`. A `#[napi(object)]` struct such as `RgbaColor` is a
plain JavaScript object, converted field by field. A `#[napi]` struct such as `SlintRgbaColor` or
`SlintBrush` is a JavaScript class wrapping the Rust value, with `#[napi]` methods on its `impl`
and `From` conversions to and from the core type.

### Callback Handling

A JavaScript function that has to outlive the call cannot be held directly: it must become a
reference that keeps it alive, and that reference plus the `Env` must be wrapped in a
`send_wrapper::SendWrapper` to satisfy the `Send` bound of whatever holds it. Calling it back
means borrowing it against the `Env` again. `invoke_from_event_loop()` in `api/node/rust/lib.rs`
is the smallest example of the whole shape.

### Building Node.js Module

```sh
cd api/node
pnpm install
pnpm build
```

## Python Bindings

### Structure

Uses PyO3 with maturin build system:

```
api/python/slint/
├── lib.rs              # Module initialization
├── interpreter.rs      # Compiler, ComponentInstance
├── value.rs            # Value conversions
├── models.rs           # Model wrappers
├── image.rs            # Image type
├── errors.rs           # Error types
└── Cargo.toml
```

### PyO3 Function Pattern

`#[pyfunction]` exports a function, `#[pymodule]` marks the module initializer, and each class
and function is registered there with `add_class::<T>()` and `add_function(wrap_pyfunction!(...))`.
`api/python/slint/lib.rs` has two `#[pymodule]` entry points — one for the released `slint`
extension and one for the dev distribution — both delegating to the same `register_module()`.

A function that blocks must release the GIL around the blocking part with `py.detach()`, or other
Python threads stall for as long as it runs. `run_event_loop()` does that, and stashes any
exception raised from a callback in a thread-local so it can be re-raised once the loop returns.

### Class Bindings

A class is a `#[pyclass]` struct wrapping the Rust type, with its methods in a `#[pymethods]`
impl: `#[new]` for the constructor, `#[getter]` and `#[setter]` pairs for what looks like an
attribute from Python. `unsendable` says the object may only be touched from the thread that
created it, which is what the interpreter types require. `Compiler` in
`api/python/slint/interpreter.rs` is the model to copy.

### Value Conversion

`api/python/slint/value.rs` converts in both directions. Rust to Python goes through an
`IntoPyObject` impl for `SlintToPyValue`, which matches on the interpreter `Value` and produces
the corresponding Python object — a plain `int`, `float`, `str` or `bool` where one fits, and a
dedicated class (`PyImage`, a model wrapper, `LogicalPosition`, ...) otherwise.

`SlintToPyValue` carries the expected `.slint` type alongside the value, because the `Value` alone
does not always determine the Python type: a `Value::Number` becomes a Python `int` when the
property is declared `int` and a `float` otherwise, and a `Value::Model` needs its element type to
convert its rows.

### Building Python Module

```sh
cd api/python
maturin develop  # Development build
maturin build    # Release wheel
```

## Internal FFI Modules

### Property FFI (`internal/core/properties/ffi.rs`)

Everything here works on a `PropertyHandleOpaque`, the opaque `PropertyHandle`. The
`slint_property_*` functions cover the whole property lifecycle: `init` and `drop`, `update`, `set_changed`,
`register_as_dependency`, the binding calls (`set_binding`, `delete_binding`, `evaluate_binding`,
`intercept_set_binding`), and one `set_animated_value_*` / `set_animated_binding_*` pair per
animatable type (int, float, color, brush).

`make_c_function_binding()` is what turns the C side into something the property system can hold:
it takes the binding's function pointer, the user data and its drop function, and the two optional
interception hooks (one for a value being set, one for a binding being set), and returns an
`impl BindingCallable<c_void>`.

### Window FFI (`internal/core/window.rs`, plus `slint_windowrc_init` in `api/cpp/lib.rs`)

The `ffi` module of `internal/core/window.rs` defines `WindowAdapterRcOpaque` and some forty
`slint_windowrc_*` functions — the lifecycle ones (`drop`, `clone`), showing and hiding, the
scale factor, the focus item and text-input-focused flag, setting the component, the popups, the
event dispatch entry points, the fullscreen/maximized/minimized state, the rendering notifier and
close-requested callbacks, `request_redraw`, `take_snapshot`, and the position and size accessors.
A second module, `ffi_window`, follows it with the ones that work on a `Window` rather than the
adapter.

`slint_windowrc_init`, which creates the window adapter through the platform, is the exception:
it lives in `api/cpp/lib.rs`, because that is where the platform is bound.

### Item Tree VTables (`internal/core/item_tree.rs`)

`ItemTreeVTable` is the vtable every component instance provides, on both the Rust and the C++
side. Its entries are all `extern "C" fn` taking a `Pin<VRef<ItemTreeVTable>>` as the receiver;
see [item-tree.md](item-tree.md#itemtreevtable) for what they do.

### Interpreter FFI (`internal/interpreter/ffi.rs`)

`Value` crosses the boundary as a `Box<Value>`, so the `slint_interpreter_value_*` functions come
in sets: a `_new` and a `_new_<kind>` constructor per kind, a `_type` returning the `ValueType`,
and a `_to_<kind>` accessor. Most of those return `Option<&T>` — `None` when the value holds
something else — but `_to_struct` and `_to_model` return a raw pointer and `_to_array` writes
through an out parameter and returns a `bool`.

`ValueType` (`internal/interpreter/api.rs`, not the ffi module) is `Void`, `Number`, `String`,
`Bool`, `Model`, `Struct`, `Brush`, `Image`, plus a hidden `Other = -1` for values whose type is
not part of the public API.

## Core FFI Patterns

### Pattern 1: Opaque Pointer Types

Hide internal types from FFI consumers:

```rust
#[repr(C)]
pub struct OpaqueType(*const c_void, *const c_void);

// Size must match the actual type
assert_eq!(
    core::mem::size_of::<ActualType>(),
    core::mem::size_of::<OpaqueType>()
);
```

### Pattern 2: User Data + Cleanup

For callbacks that need to release resources:

```rust
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_set_callback(
    callback: extern "C" fn(user_data: *mut c_void),
    user_data: *mut c_void,
    drop_user_data: Option<extern "C" fn(*mut c_void)>,
) {
    struct UserData {
        user_data: *mut c_void,
        drop_user_data: Option<extern "C" fn(*mut c_void)>,
    }

    impl Drop for UserData {
        fn drop(&mut self) {
            if let Some(drop_fn) = self.drop_user_data {
                drop_fn(self.user_data)
            }
        }
    }

    let ud = UserData { user_data, drop_user_data };
    // Use ud, it will be cleaned up when dropped
}
```

### Pattern 3: VTable System

For polymorphic behavior across FFI:

```rust
#[repr(C)]
pub struct MyVTable {
    pub method_a: extern "C" fn(VRef<MyVTable>, arg: i32) -> i32,
    pub method_b: extern "C" fn(VRef<MyVTable>) -> bool,
    pub drop: extern "C" fn(VRefMut<MyVTable>),
}

// Use with vtable crate
vtable::VRef<MyVTable>
vtable::VBox<MyVTable>
```

### Pattern 4: Feature-Gated FFI

```rust
#[cfg(feature = "ffi")]
pub mod ffi {
    #[unsafe(no_mangle)]
    pub extern "C" fn slint_feature_specific_function() { ... }
}

#[cfg(all(feature = "ffi", feature = "std"))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_register_font_from_path(...) { ... }
```

### Pattern 5: cbindgen Visibility

```rust
// Make types visible to cbindgen without exporting
#[cfg(cbindgen)]
#[repr(C)]
struct InternalRect {
    x: f32, y: f32, width: f32, height: f32,
}
```

## Adding New FFI Functions

### Step 1: Add to Internal Module

```rust
// internal/core/mymodule.rs
#[cfg(feature = "ffi")]
pub mod ffi {
    use super::*;

    #[unsafe(no_mangle)]
    pub extern "C" fn slint_mymodule_new_function(
        param: i32,
        out: *mut ResultType,
    ) -> bool {
        // Implementation
        let result = internal_function(param);
        unsafe { *out = result };
        true
    }
}
```

### Step 2: Update cbindgen (for C++)

```rust
// api/cpp/cbindgen.rs
config.export.include = [
    // ... existing exports
    "slint_mymodule_new_function",
];
```

### Step 3: Add C++ Wrapper

```cpp
// api/cpp/include/slint_mymodule.h
namespace slint {
    inline ResultType mymodule_new_function(int param) {
        ResultType result;
        slint_mymodule_new_function(param, &result);
        return result;
    }
}
```

### Step 4: Add Python Binding

```rust
// api/python/slint/mymodule.rs
#[pyfunction]
fn new_function(param: i32) -> PyResult<ResultType> {
    Ok(internal_function(param))
}

// In lib.rs
m.add_function(wrap_pyfunction!(mymodule::new_function, m)?)?;
```

### Step 5: Add Node.js Binding

```rust
// api/node/rust/mymodule.rs
#[napi]
pub fn new_function(param: i32) -> napi::Result<ResultType> {
    Ok(internal_function(param))
}
```

## Build System

### Cargo Features

`api/cpp/Cargo.toml` builds `slint-cpp` as `["lib", "cdylib", "staticlib"]` and sets
`links = "slint_cpp"` so the build script's metadata reaches the CMake side.

Two thirds of its features just forward to `i-slint-backend-selector`, one per renderer
(`renderer-femtovg`, `renderer-skia` and its per-API variants, `renderer-software`) and one per
backend (`backend-winit` and its X11/Wayland-only variants, `backend-qt`, `backend-linuxkms`).
The rest are its own: `interpreter` and `live-preview`, `testing`, `gettext`, `system-tray`, and
the two that decide the environment — `std`, which the default set enables, and `freestanding`
for bare metal.

### CMake Feature Mapping

```cmake
# Feature flags: CMake options → Cargo features
define_cargo_feature(backend-winit "Enable winit" ON)
define_cargo_feature(backend-qt "Enable Qt" OFF)
define_cargo_feature(renderer-femtovg "Enable FemtoVG" ON)
define_cargo_feature(interpreter "Enable interpreter" ON)
```

### Header Generation

Headers are generated automatically during the build process by the `slint-cpp` crate's `build.rs`.

```sh
# Headers are placed in the OUT_DIR of the slint-cpp build, for example:
# target/debug/build/slint-cpp-[hash]/out/generated_include/
```

## Testing

### C++ Tests

```sh
# Build with testing backend
cargo build -p slint-cpp --features testing

# Run C++ tests
cd cppbuild
ctest
```

### Node.js Tests

```sh
cd api/node
pnpm test
```

### Python Tests

```sh
cd api/python
pytest
```

### FFI-Specific Tests

```sh
# Test interpreter FFI
cargo test -p slint-interpreter ffi

# Test core FFI
cargo test -p i-slint-core ffi
```

## Debugging Tips

### Common Issues

| Issue | Cause | Solution |
|-------|-------|----------|
| Segfault on init | Size mismatch | Check `assert_eq!` for opaque types |
| Memory leak | Missing drop_user_data | Ensure cleanup function is called |
| Type mismatch | cbindgen out of sync | Rebuild the project to regenerate headers |
| Undefined symbol | FFI function not exported | Add to `config.export.include` |
| Python crash | GIL issues | Use `py.detach()` around blocking calls |
| Node crash | Ref counting | Keep the JS function alive with `create_ref()` |

### Checking ABI Compatibility

```rust
// Add size checks in FFI functions
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_init(out: *mut OpaqueType) {
    const _: () = assert!(
        core::mem::size_of::<ActualType>() == core::mem::size_of::<OpaqueType>()
    );
    // ...
}
```

### Inspecting Generated Headers

```sh
# Find generated C++ headers
find target -name "generated_include" -type d
```

### Tracing FFI Calls

```rust
#[unsafe(no_mangle)]
pub extern "C" fn slint_debug_function(param: i32) -> i32 {
    eprintln!("slint_debug_function called with: {}", param);
    let result = internal_function(param);
    eprintln!("slint_debug_function returning: {}", result);
    result
}
```

## Rust Public API

### Private Unstable API

Generated code uses the helpers in `api/rs/slint/private_unstable_api.rs`. Its `re_exports`
module re-exports everything the generated code names — core types, the native widgets, `vtable`,
`const_field_offset` — so the generated file only ever has to `use` that one module.

Alongside it are the helpers that keep the generated code short, such as
`set_property_binding()`, which installs a binding that holds only a weak reference to the
component and falls back to the default value once it is gone.

### Build Script Support

`api/rs/build/lib.rs` is what a `build.rs` calls. `compile_with_config()` takes the path of the
`.slint` file, relative to the crate manifest, and a `CompilerConfiguration`, and writes the
generated Rust into the build output directory.

`CompilerConfiguration` wraps the compiler's own configuration and is built by chaining consuming
`with_*` methods: `with_include_paths()`, `with_library_paths()`, `with_style()`,
`with_scale_factor()`, `with_bundled_translations()`, `with_default_translation_context()`,
`with_debug_info()`, `with_sdf_fonts()`,
`as_library()`, `rust_module()`, and `embed_resources()`, which takes an `EmbedResourcesKind`
saying whether resources are loaded from an absolute path at run-time, embedded as-is, or
pre-processed into raw pixel data for the software renderer.
