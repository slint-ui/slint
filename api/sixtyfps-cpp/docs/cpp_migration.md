# Migrating from Older Versions

The C++ library is versioned according to the principles of [Semantic Versioning](https://semver.org). We define that the left-most non-zero component of the version is the major version, followed by the minor and optionally patch version. That means releases in the "0.y.z" series treat changes in "y" as a major release, which can contain incompatible API changes, while changes in just "z" are minor. For example the release 0.1.6 is fully backwards compatible to 0.1.5, but it contains new functionality. The release 0.2.0 however is a new major version compared to 0.1.x and may contain API incompatible changes.

This guide lists all API incompatible changes between major versions and describes how you can migrate your application's source code.

## Migrating from Version 0.1.x to 0.2.0

In version 0.2.0 we have increased the minimum version of C++. You need to have a C++ compiler installed that supports C++ 20 or newer.

If you are building SixtyFPS from source, you need to make sure that your Rust installation is up-to-date. If you have installed Rust using `rustup`, then you can upgrade to the latest Version of Rust by running `rustup update`.

### C++ Interpreter API

#### Callbacks

Callbacks declared in `.60` markup can be invoked from C++ using  {cpp:func}`sixtyfps::interpreter::ComponentInstance::invoke_callback()` or {cpp:func}`sixtyfps::interpreter::ComponentInstance::invoke_global_callback()`. The arguments to the callback at invocation time used to require the use of `sixtyfps::Slice` type. This was changed to use the C++ 20 [`std::span`](https://en.cppreference.com/w/cpp/container/span) type, for easier passing.

Old code:

```cpp
sixtyfps::Value args[] = { SharedString("Hello"), 42. };
instance->invoke_callback("foo", sixtyfps::Slice{ args, 2 });
```

New code:

```cpp
sixtyfps::Value args[] = { SharedString("Hello"), 42. };
instance->invoke_callback("foo", args);
```

#### Models

`Model::row_data` returns now a `std::optional<ModelData>` and can thus be used with indices that are out of bounds.

This also means that `Model`s must handle invalid indices and may not crash when a invalid index is passed in.

Old code:

```cpp
another_model->row_data(2);
```

New code:

```cpp
// `another_model` is a model that contains floats.
std::optional<float> entry = another_model->row_data(2);
if (entry.has_value()) {
    do_something(*entry);
} else {
    // row index 2 is out of bounds
}
```
