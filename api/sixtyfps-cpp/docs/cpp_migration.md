# Migrating from Older Versions

The C++ library is versioned according to the principles of [Semantic Versioning](https://semver.org). We define that the left-most non-zero component of the version is the major version, followed by the minor and optionally patch version. That means releases in the "0.y.z" series treat changes in "y" as a major release, which can contain incompatible API changes, while changes in just "z" are minor. For example the release 0.1.6 is fully backwards compatible to 0.1.5, but it contains new functionality. The release 0.2.0 however is a new major version compared to 0.1.x and may contain API incompatible changes.

This guide lists all API incompatible changes between major versions and describes how you can migrate your application's source code.

## Migrating from Version 0.1.x to 0.2.0

In the 0.2.x series we have increased the minimum version of C++ and Rust that we require. You need to have Rust >= 1.56 installed and a C++ compiler that supports C++ 20 or newer. If you have installed Rust using `rustup`, then you can upgrade to the latest Version of Rust by running `rustup update`.

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
