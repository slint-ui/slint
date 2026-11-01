
# Slint Android Activity Backend

This crate runs Slint apps on Android phones, tablets, and Android-based devices,
with the same `.slint` language and Slint API as on desktop, iOS, and embedded.
It's built on the [android-activity](https://github.com/rust-mobile/android-activity) crate.
Apps use it through the [slint::android](https://slint.dev/docs/rust/slint/android/) module
and the `backend-android-activity-06` feature of the `slint` crate.

It also makes text input feel native.
A small Java helper, compiled into the app, connects Slint's text fields to Android's input method framework.
This gives predictive text and autocorrect, IME composition for languages such as Chinese and Japanese,
native selection handles, and the cut, copy, and paste menu.

**NOTE**: This library is an **internal** crate of the [Slint project](https://slint.dev).

**WARNING**: This crate does not follow the semver convention for versioning and can
only be used with `version = "=x.y.z"` in Cargo.toml.
