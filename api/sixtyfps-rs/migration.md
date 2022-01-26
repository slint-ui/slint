# Migrating from Older Versions

The Rust library is versioned according to the principles of [Semantic Versioning](https://semver.org). We define that the left-most non-zero component of the version is the major version, followed by the minor and optionally patch version. That means releases in the "0.y.z" series treat changes in "y" as a major release, which can contain incompatible API changes, while changes in just "z" are minor. For example the release 0.1.6 is fully backwards compatible to 0.1.5, but it contains new functionality. The release 0.2.0 however is a new major version compared to 0.1.x and may contain API incompatible changes.

This guide lists all API incompatible changes between major versions and describes how you can migrate your application's source code.

## Migrating from Version 0.1.x to 0.2.0

In 0.2.0 we have increased the minimum version of rust. You need to have rust compiler version >= 1.56 installed.

### Rust API

#### Models

##### `Model::row_data`

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

##### `Model::attach_peer` and `Model::model_tracker`

`attach_peer()` has been removed. Instead you must implement the `fn model_tracker(&self) -> &dyn ModelTracker` function. If you have a constant model, then you can just return `&()`, otherwise you can return a reference to the `ModelNotify` instance that you previously used in `attach_peer`:

Old code:

```rust
fn attach_peer(&self, peer: ...) {
    self.model_notify.attach_peer(peer);
}
```

New code:

```rust
fn model_tracker(&self) -> &dyn ModelTracker {
    &self.model_notify
}
```

or if your model is constant:

```rust
fn model_tracker(&self) -> &dyn ModelTracker {
    &()
}
```
