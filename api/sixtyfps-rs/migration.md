# Migrating from Older Versions

The Rust library is versioned according to the principles of [Semantic Versioning](https://semver.org). We define that the left-most non-zero component of the version is the major version, followed by the minor and optionally patch version. That means releases in the "0.y.z" series treat changes in "y" as a major release, which can contain incompatible API changes, while changes in just "z" are minor. For example the release 0.1.6 is fully backwards compatible to 0.1.5, but it contains new functionality. The release 0.2.0 however is a new major version compared to 0.1.x and may contain API incompatible changes.

This guide lists all API incompatible changes between major versions and describes how you can migrate your application's source code.

## Migrating from Version 0.1.x to 0.2.0

In 0.2.0 we have increased the minimum version of rust. You need to have rust compiler version >= 1.56 installed.

### Rust API

#### Models

`Model::row_data` now returns an `Option<T>` instead of a simple `T`.

This implies that `Model`s must handle invalid indices and may not panic when they encounter one.

Old code:

```rust,ignore
let row_five = model.row_data(5);
```

New code:

```rust,ignore
let row_five = model.row_data(5).unwrap_or_default();
```

`Model::model_tracker` has no default implementation anymore. This has no effect for custom dynamic models, as
those have overridden the default implementation in any case. You will need to add this code into the implementation of the `Model` trait of your custom model:

```rust,ignore
fn model_tracker(&self) -> &dyn ModelTracker {
    &()
}
```
