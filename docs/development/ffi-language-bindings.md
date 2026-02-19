# FFI & Language Bindings

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

Hide internal Rust types from C++:

```rust
/// Opaque type for Rc<dyn WindowAdapter>
#[repr(C)]
pub struct WindowAdapterRcOpaque(*const c_void, *const c_void);

/// Opaque type for PropertyHandle
#[repr(C)]
pub struct PropertyHandleOpaque(PropertyHandle);

/// Opaque type for callbacks
#[repr(C)]
pub struct CallbackOpaque(*const c_void, *const c_void);
```

### cbindgen Code Generation

The `cbindgen.rs` file (900+ lines) generates C++ headers:

```rust
// api/cpp/cbindgen.rs
fn gen_enums(include_dir: &Path) {
    // Generates slint_enums.h and slint_enums_internal.h
    i_slint_common::for_each_enums!(gen_enum_descriptors);
}

fn gen_structs(include_dir: &Path) {
    // Generates slint_builtin_structs.h
    i_slint_common::for_each_builtin_structs!(gen_struct_descriptors);
}

// Type renaming for C++
config.export.rename = [
    ("Callback".into(), "private_api::CallbackHelper".into()),
    ("Coord".into(), "float".into()),
    ("SharedString".into(), "slint::SharedString".into()),
    // ... more mappings
];
```

**Generated headers:**
- `slint_enums.h` / `slint_enums_internal.h` - Public/private enums
- `slint_builtin_structs.h` / `slint_builtin_structs_internal.h` - Structs
- `slint_string_internal.h` - SharedString, StyledText
- `slint_properties_internal.h` - Property system
- `slint_timer_internal.h` - Timer management
- Item VTables for UI elements

### CMake Integration

Uses Corrosion to bridge CMake and Cargo:

```cmake
# api/cpp/CMakeLists.txt
define_cargo_feature(freestanding "Enable freestanding environment" OFF)
define_cargo_dependent_feature(interpreter "Enable .slint loading" ON)
define_cargo_feature(backend-winit "Enable winit windowing" ON)

# Feature flags map: CMake options → Cargo features
# SLINT_FEATURE_BACKEND_WINIT → --features backend-winit
```

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
│   │   ├── image.rs
│   │   └── ...
│   └── interpreter/        # Interpreter bindings
│       ├── component_compiler.rs
│       ├── component_instance.rs
│       └── value.rs
├── Cargo.toml
└── package.json
```

### NAPI Function Pattern

```rust
// api/node/rust/lib.rs
use napi::{Env, JsFunction};
extern crate napi_derive;

#[napi]
pub fn mock_elapsed_time(ms: f64) {
    i_slint_core::tests::slint_mock_elapsed_time(ms as _);
}

#[napi]
pub enum ProcessEventsResult {
    Continue,
    Exited,
}

#[napi]
pub fn process_events() -> napi::Result<ProcessEventsResult> {
    i_slint_backend_selector::with_platform(|b| {
        b.process_events(std::time::Duration::ZERO, i_slint_core::InternalToken)
    })
    .map_err(|e| napi::Error::from_reason(e.to_string()))
    .map(|result| match result {
        core::ops::ControlFlow::Continue(()) => ProcessEventsResult::Continue,
        core::ops::ControlFlow::Break(()) => ProcessEventsResult::Exited,
    })
}
```

### Type Bindings

```rust
// api/node/rust/types/brush.rs
#[napi(object)]
pub struct RgbaColor {
    pub red: f64,
    pub green: f64,
    pub blue: f64,
    pub alpha: Option<f64>,
}

#[napi]
pub struct SlintRgbaColor {
    inner: Color,
}

#[napi]
impl SlintRgbaColor {
    #[napi(constructor)]
    pub fn new() -> Self { ... }

    #[napi]
    pub fn red(&self) -> f64 { self.inner.red() as f64 }
}
```

### Callback Handling

```rust
#[napi]
pub fn invoke_from_event_loop(env: Env, callback: JsFunction) -> napi::Result<napi::JsUndefined> {
    let function_ref = RefCountedReference::new(&env, callback)?;
    let function_ref = send_wrapper::SendWrapper::new(function_ref);

    i_slint_core::api::invoke_from_event_loop(move || {
        let guard = function_ref.get();
        if let Err(e) = guard.call::<JsUnknown>(None, &[]) {
            eprintln!("Callback error: {:?}", e);
        }
    })
    .map_err(|e| napi::Error::from_reason(e.to_string()))?;

    env.get_undefined()
}
```

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

```rust
// api/python/slint/lib.rs
use pyo3::prelude::*;

#[gen_stub_pyfunction]
#[pyfunction]
fn run_event_loop(py: Python<'_>) -> Result<(), PyErr> {
    EVENT_LOOP_EXCEPTION.replace(None);
    EVENT_LOOP_RUNNING.set(true);

    let result = py.allow_threads(|| slint_interpreter::run_event_loop());

    EVENT_LOOP_RUNNING.set(false);
    result.map_err(|e| errors::PyPlatformError::from(e))?;
    EVENT_LOOP_EXCEPTION.take().map_or(Ok(()), |err| Err(err))
}

#[pymodule]
fn slint(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Compiler>()?;
    m.add_class::<CompilationResult>()?;
    m.add_class::<ComponentInstance>()?;
    m.add_function(wrap_pyfunction!(run_event_loop, m)?)?;
    Ok(())
}
```

### Class Bindings

```rust
// api/python/slint/interpreter.rs
#[gen_stub_pyclass]
#[pyclass(unsendable)]
pub struct Compiler {
    compiler: slint_interpreter::Compiler,
}

#[gen_stub_pymethods]
#[pymethods]
impl Compiler {
    #[new]
    fn py_new() -> PyResult<Self> {
        Ok(Self { compiler: slint_interpreter::Compiler::new() })
    }

    #[getter]
    fn get_include_paths(&self) -> PyResult<Vec<PathBuf>> {
        Ok(self.compiler.include_paths().map(|p| p.to_owned()).collect())
    }

    #[setter]
    fn set_include_paths(&mut self, paths: Vec<PathBuf>) {
        self.compiler.set_include_paths(paths);
    }

    fn build_from_path(&mut self, py: Python<'_>, path: PathBuf) -> CompilationResult {
        py.allow_threads(|| {
            self.compiler.build_from_path(&path).into()
        })
    }
}
```

### Value Conversion

```rust
// api/python/slint/value.rs
impl<'py> IntoPyObject<'py> for SlintToPyValue {
    fn into_pyobject(self, py: Python<'py>) -> Result<Self::Output, Self::Error> {
        match self.slint_value {
            slint_interpreter::Value::Void => ().into_bound_py_any(py),
            slint_interpreter::Value::Number(num) => num.into_bound_py_any(py),
            slint_interpreter::Value::String(str) => str.into_bound_py_any(py),
            slint_interpreter::Value::Bool(b) => b.into_bound_py_any(py),
            slint_interpreter::Value::Image(image) => {
                crate::image::PyImage::from(image).into_bound_py_any(py)
            }
            slint_interpreter::Value::Model(model) => {
                crate::models::PyModelShared::rust_into_py_model(&model, py)
                    .map_or_else(
                        || type_collection.model_to_py(&model).into_bound_py_any(py),
                        |m| Ok(m),
                    )
            }
            // ... more conversions
        }
    }
}
```

### Building Python Module

```sh
cd api/python
maturin develop  # Development build
maturin build    # Release wheel
```

## Internal FFI Modules

### Property FFI (`internal/core/properties/ffi.rs`)

```rust
#[repr(C)]
pub struct PropertyHandleOpaque(PropertyHandle);

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_property_init(out: *mut PropertyHandleOpaque) {
    // Initialize property handle
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_property_update(
    handle: &PropertyHandleOpaque,
    val: *mut c_void,
) {
    // Update property value
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_property_set_changed(
    handle: &PropertyHandleOpaque,
    value: *const c_void,
) {
    // Mark property as changed
}

// C function binding support
fn make_c_function_binding(
    binding: extern "C" fn(*mut c_void, *mut c_void),
    user_data: *mut c_void,
    drop_user_data: Option<extern "C" fn(*mut c_void)>,
    intercept_set: Option<extern "C" fn(*mut c_void, ...) -> bool>,
) -> impl Fn() -> T {
    // Creates Rust closure from C function pointers
}
```

### Window FFI (`internal/core/window.rs`)

```rust
pub mod ffi {
    #[repr(C)]
    pub struct WindowAdapterRcOpaque(*const c_void, *const c_void);

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_init(out: *mut WindowAdapterRcOpaque) { ... }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_drop(handle: *mut WindowAdapterRcOpaque) { ... }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_clone(
        source: &WindowAdapterRcOpaque,
        target: *mut WindowAdapterRcOpaque,
    ) { ... }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_show(handle: &WindowAdapterRcOpaque) { ... }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_windowrc_hide(handle: &WindowAdapterRcOpaque) { ... }
}
```

### Item Tree VTables (`internal/core/item_tree.rs`)

```rust
/// VTable for component instances
pub struct ItemTreeVTable {
    /// Visit children in traversal order
    pub visit_children_item: extern "C" fn(
        Pin<VRef<ItemTreeVTable>>,
        index: isize,
        order: TraversalOrder,
        visitor: VRefMut<ItemVisitorVTable>,
    ) -> VisitChildrenResult,

    /// Get item reference by index
    pub get_item_ref: extern "C" fn(
        Pin<VRef<ItemTreeVTable>>,
        index: u32,
    ) -> Pin<VRef<ItemVTable>>,

    /// Get subtree range for repeaters
    pub get_subtree_range: extern "C" fn(
        Pin<VRef<ItemTreeVTable>>,
        index: u32,
    ) -> IndexRange,

    // ... more vtable entries
}
```

### Interpreter FFI (`internal/interpreter/ffi.rs`)

```rust
/// Value type enum for FFI
#[repr(C)]
pub enum ValueType {
    Void, Number, String, Bool, Model, Struct, Brush, Image,
}

#[unsafe(no_mangle)]
pub extern "C" fn slint_interpreter_value_new() -> Box<Value> {
    Box::new(Value::Void)
}

#[unsafe(no_mangle)]
pub extern "C" fn slint_interpreter_value_new_string(str: &SharedString) -> Box<Value> {
    Box::new(Value::String(str.clone()))
}

#[unsafe(no_mangle)]
pub extern "C" fn slint_interpreter_value_type(val: &Value) -> ValueType {
    match val {
        Value::Void => ValueType::Void,
        Value::Number(_) => ValueType::Number,
        Value::String(_) => ValueType::String,
        // ...
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn slint_interpreter_value_to_string(val: &Value) -> Option<&SharedString> {
    match val {
        Value::String(s) => Some(s),
        _ => None,
    }
}
```

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
#[gen_stub_pyfunction]
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

```toml
# api/cpp/Cargo.toml
[lib]
crate-type = ["lib", "cdylib", "staticlib"]
links = "slint_cpp"

[features]
# Renderers
renderer-femtovg = ["i-slint-backend-selector/renderer-femtovg"]
renderer-skia = ["i-slint-backend-selector/renderer-skia"]
renderer-software = ["i-slint-backend-selector/renderer-software"]

# Backends
backend-winit = ["i-slint-backend-selector/backend-winit"]
backend-qt = ["i-slint-backend-selector/backend-qt"]
backend-linuxkms = ["i-slint-backend-selector/backend-linuxkms"]

# Other
freestanding = ["i-slint-core/freestanding"]
interpreter = ["slint-interpreter"]
testing = ["i-slint-backend-testing"]
```

### CMake Feature Mapping

```cmake
# Feature flags: CMake options → Cargo features
define_cargo_feature(backend-winit "Enable winit" ON)
define_cargo_feature(backend-qt "Enable Qt" OFF)
define_cargo_feature(renderer-femtovg "Enable FemtoVG" ON)
define_cargo_feature(interpreter "Enable interpreter" ON)
```

### Header Generation

```sh
# Generate headers via xtask
cargo xtask cbindgen

# Headers placed in:
# - target/slint-cpp-generated/include/
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
| Type mismatch | cbindgen out of sync | Regenerate headers with `cargo xtask cbindgen` |
| Undefined symbol | FFI function not exported | Add to `config.export.include` |
| Python crash | GIL issues | Use `py.allow_threads()` for blocking calls |
| Node crash | Ref counting | Use `RefCountedReference` for callbacks |

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
# View generated C++ headers
ls target/slint-cpp-generated/include/

# Check specific header
cat target/slint-cpp-generated/include/slint_properties_internal.h
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

Generated code uses internal helpers:

```rust
// api/rs/slint/private_unstable_api.rs
pub mod re_exports {
    pub use i_slint_core::{*, properties::*, item_tree::*};
    pub use vtable::*;
    pub use pin_weak::rc::PinWeak;
}

pub fn set_property_binding<T, StrongRef>(
    property: Pin<&Property<T>>,
    component_strong: &StrongRef,
    binding: fn(StrongRef) -> T,
) {
    let weak = component_strong.to_weak();
    property.set_binding(move || {
        StrongRef::from_weak(&weak).map(binding).unwrap_or_default()
    })
}
```

### Build Script Support

```rust
// api/rs/build/lib.rs
pub struct CompilerConfiguration {
    pub include_paths: Vec<PathBuf>,
    pub library_paths: HashMap<String, PathBuf>,
    pub style: Option<String>,
}

pub fn compile_with_config(
    path: impl AsRef<Path>,
    config: CompilerConfiguration,
) -> Result<(), CompileError> {
    // Compile .slint file and generate Rust code
}
```
