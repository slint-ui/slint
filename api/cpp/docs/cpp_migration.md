# Migrating from Older Versions

The C++ library is versioned according to the principles of [Semantic Versioning](https://semver.org). We define that the left-most non-zero component of the version is the major version, followed by the minor and optionally patch version. That means releases in the "0.y.z" series treat changes in "y" as a major release, which can contain incompatible API changes, while changes in just "z" are minor. For example the release 0.1.6 is fully backwards compatible to 0.1.5, but it contains new functionality. The release 0.2.0 however is a new major version compared to 0.1.x and may contain API incompatible changes.

This guide lists all API incompatible changes between major versions and describes how you can migrate your application's source code.

## Migrating from Version 0.1.x to 0.2.0

In version 0.2.0 we have increased the minimum version of C++. You need to have a C++ compiler installed that supports C++ 20 or newer.

If you are building Slint from source, you need to make sure that your Rust installation is up-to-date. If you have installed Rust using `rustup`, then you can upgrade to the latest Version of Rust by running `rustup update`.

### CMake interface

-   When using `FetchContent`, the `SOURCE_SUBDIR` has changed from `api/sixtyfps-cpp` to `api/cpp`
-   `find_package(SixtyFPS)` becomes `find_package(Slint)`.
-   The `SixtyFPS::SixtyFPS` CMake target was renamed to `Slint::Slint`.
-   The `sixtyfps_target_60_sources` CMake command was renamed to `slint_target_sources`.

Some CMake options have been renamed:

| Old Option                    | New Option                         | Note                                                                                                                     |
| ----------------------------- | ---------------------------------- | ------------------------------------------------------------------------------------------------------------------------ |
| `SIXTYFPS_FEATURE_BACKEND_GL` | `SLINT_FEATURE_BACKEND_GL_ALL`     | Enable this feature if you want to use the OpenGL ES 2.0 rendering backend with support for all windowing systems.       |
| `SIXTYFPS_FEATURE_X11`        | `SLINT_FEATURE_BACKEND_GL_X11`     | Enable this feature and switch off `SLINT_FEATURE_BACKEND_GL_ALL` if you want a smaller build with just X11 support.     |
| `SIXTYFPS_FEATURE_WAYLAND`    | `SLINT_FEATURE_BACKEND_GL_WAYLAND` | Enable this feature and switch off `SLINT_FEATURE_BACKEND_GL_ALL` if you want a smaller build with just wayland support. |

### Models

`Model::row_data` returns now a `std::optional<ModelData>` and can thus be used with indices that are out of bounds.

This also means that `Model`s must handle invalid indices and may not crash when a invalid index is passed in.

Old code:

```cpp
float value = another_model->row_data(2);
do_something(value)
```

New code:

```cpp
// `another_model` is a model that contains floats.
std::optional<float> value = another_model->row_data(2);
if (value.has_value()) {
    do_something(*value);
} else {
    // row index 2 is out of bounds
}
```

### C++ Interpreter API

#### Callbacks

Callbacks declared in `.slint` markup can be invoked from C++ using {cpp:func}`slint::interpreter::ComponentInstance::invoke_callback()` or {cpp:func}`slint::interpreter::ComponentInstance::invoke_global_callback()`. The arguments to the callback at invocation time used to require the use of `sixtyfps::Slice` type. This was changed to use the C++ 20 [`std::span`](https://en.cppreference.com/w/cpp/container/span) type, for easier passing.

Old code:

```cpp
sixtyfps::Value args[] = { SharedString("Hello"), 42. };
instance->invoke_callback("foo", sixtyfps::Slice{ args, 2 });
```

New code:

```cpp
slint::Value args[] = { SharedString("Hello"), 42. };
instance->invoke_callback("foo", args);
```

#### Models

The `Value::Type::Array` has been replaced by `Value::Type::Model`
