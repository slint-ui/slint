
# Slint Android Activity Backend

This crate runs Slint apps on Android phones, tablets, and Android-based devices,
with the same `.slint` language and Slint API as on desktop, iOS, and embedded.
It's built on the [android-activity](https://github.com/rust-mobile/android-activity) crate.
To use it, enable the `backend-android-activity-06` feature of the `slint` crate
and call the functions of the [`slint::android`](https://slint.dev/docs/rust/slint/android/) module.

It also makes text input feel native.
A small Java helper, compiled into the app, connects Slint's text fields to Android's input method framework.
The helper provides:

- predictive text and autocorrect
- IME composition for languages such as Chinese and Japanese
- native selection handles, with a magnifier on Android 10 and later when using NativeActivity
- the cut, copy, and paste menu

**NOTE**: This library is an **internal** crate of the [Slint project](https://slint.dev).

**WARNING**: This crate does not follow the semver convention for versioning and can
only be used with `version = "=x.y.z"` in Cargo.toml.
