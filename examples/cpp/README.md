# C++ Platform Integration Examples

These examples demonstrate different ways to integrate Slint into C++ applications using the platform API.

## Examples

### platform_native

Shows how to use the Slint C++ platform API to integrate into a native Windows application using the WIN32 API directly.

**Use case:** Embedding Slint in existing native Windows applications without Qt or other frameworks.

**Key files:**
- `main.cpp` - Native WIN32 application shell
- `windowadapter_win.h` - Slint platform implementation using WIN32 API
- `appview.h/cpp` - Interface between the application and Slint UI

### platform_qt

Shows how to use the Slint platform API to render a Slint scene inside a Qt window.

**Use case:** Using Slint for specific UI components within a larger Qt application, with full control over the rendering integration.

### qt_viewer

Demonstrates embedding a dynamically loaded `.slint` file into a Qt (QWidgets) application using `slint::interpreter::ComponentInstance::qwidget()`.

**Use case:** Loading and displaying Slint UI files at runtime within a Qt application, useful for plugin systems or dynamic UI loading.

## Comparison

| Example | Qt Required | Dynamic Loading | Platform |
|---------|-------------|-----------------|----------|
| platform_native | No | No | Windows only |
| platform_qt | Yes | No | Cross-platform |
| qt_viewer | Yes | Yes | Cross-platform |

## Building

Each example has its own CMakeLists.txt. Build from the example directory:

```sh
mkdir build && cd build
cmake -GNinja ..
cmake --build .
```

For Qt examples, ensure Qt is installed and `qmake` is in your PATH.
