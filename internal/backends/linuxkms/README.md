
# Slint Linux KMS Backend

This crate runs Slint apps full-screen on embedded Linux, without a windowing system such as X11 or Wayland.
It's made for kiosks, industrial HMIs, and other dedicated devices.
It renders through KMS/DRM with FemtoVG, Skia, or the software renderer,
and reads mouse, touch, and keyboard input through libinput.
Apps get it through the `backend-linuxkms` feature of the `slint` crate.

**NOTE**: This library is an **internal** crate of the [Slint project](https://slint.dev).
This crate should **not be used directly** by applications using Slint.
You should use the `slint` crate instead.

**WARNING**: This crate does not follow the semver convention for versioning and can
only be used with `version = "=x.y.z"` in Cargo.toml.
