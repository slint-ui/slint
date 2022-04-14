# Migrating from Older Versions

The Rust library is versioned according to the principles of [Semantic Versioning](https://semver.org). We define that the left-most non-zero component of the version is the major version, followed by the minor and optionally patch version. That means releases in the "0.y.z" series treat changes in "y" as a major release, which can contain incompatible API changes, while changes in just "z" are minor. For example the release 0.1.6 is fully backwards compatible to 0.1.5, but it contains new functionality. The release 0.2.0 however is a new major version compared to 0.1.x and may contain API incompatible changes.

This guide lists all API incompatible changes between major versions and describes how you can migrate your application's source code.

## Migrating from Version 0.1.x to 0.2.0

### Models

#### `Model::row_data`

[`Model::row_data`] now returns an `Option<T>` instead of a simple `T`.

[`Model`] implementation must no longer panic when encountering invalid index in [`row_data`](Model::row_data)
and [`set_row_data`](Model::set_row_data), they should return `None` instead.

When calling `row_data` one need to unwrap the value

Old code:

```rust,ignore
let row_five = model.row_data(5);
```

New code:

```rust,ignore
let row_five = model.row_data(5).unwrap_or_default();
```

#### `Model::attach_peer` and `Model::model_tracker`

`attach_peer()` has been removed. Instead you must implement the
[`fn model_tracker(&self) -> &dyn ModelTracker`](Model::model_tracker) function.
If you have a constant model, then you can just return `&()`, otherwise you can return a reference
to the [`ModelNotify`] instance that you previously used in `attach_peer`:

Old code:

```rust,ignore
fn attach_peer(&self, peer: slint::ModelPeer) {
    self.model_notify.attach_peer(peer);
}
```

New code:

```rust,ignore
fn model_tracker(&self) -> &dyn ModelTracker {
    &self.model_notify
}
```

or if your model is constant:

```rust,ignore
fn model_tracker(&self) -> &dyn ModelTracker {
    &()
}
```

#### ModelHandle

`ModelHandle` was renamed [`ModelRc`].

[`ModelRc::new`]  no longer takes a `Rc`, but takes the structure that implements the [`Model`] trait directly.
To construct a `ModelRc` from a Rc for your model, use the `From` trait. [`ModelRc::from`] is doing what
`ModelHandle::new` was doing.

## Crate features

Some crate features have been renamed:

| Old Feature Name                    | New Feature Name                   | Note                                                                          |
| ------------------------------------| ---------------------------------- | ----------------------------------------------------------------------------- |
| `backend-gl` | `backend-gl-all`     | Enable this feature if you want to use the OpenGL ES 2.0 rendering backend with support for all windowing systems. |
| `x11`        | `backend-gl-x11`     | Enable this feature and switch off `backend-gl-all` if you want a smaller build with just X11 support.             |
| `wayland`    | `backend-gl-wayland` | Enable this feature and switch off `backend-gl-all` if you want a smaller build with just wayland support.         |
