# Migrating from Older Versions

The rust library is versioned according to the principles of [Semantic Versioning](https://semver.org). We define that the left-most non-zero component of the version is the major version, followed by the minor and optionally patch version. That means releases in the "0.y.z" series treat changes in "y" as a major release, which can contain incompatible API changes, while changes in just "z" are minor. For example the release 0.1.6 is fully backwards compatible to 0.1.5, but it contains new functionality. The release 0.2.0 however is a new major version compared to 0.1.x and may contain API incompatible changes.

This guide lists all API incompatible changes between major versions and describes how you can migrate your application's source code.

## Migrating from Version 0.1.x to 0.2.0

In 0.2.0 we have increased the minimum version of rust. You need to have rust compiler version >= 1.56 installed.

### Rust API

#### Models

`Model::row_data` now returns an `Option<T>` instead of a simple `T`.

Old code:

```rust
let row_five = model.row_data(5);
```

New code:

```rust
let row_five = model.row_data(5).unwrap_or_default();
```
