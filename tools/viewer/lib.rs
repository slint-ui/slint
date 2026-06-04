// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#[cfg(feature = "remote")]
pub mod remote;

/// Poll a future that is expected to resolve immediately (e.g. the interpreter's
/// `build_from_path` when no async file loader is installed).
pub fn poll_ready<F: std::future::Future>(future: F) -> F::Output {
    let mut future = core::pin::pin!(future);
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    match std::future::Future::poll(future.as_mut(), &mut cx) {
        std::task::Poll::Ready(result) => result,
        std::task::Poll::Pending => unreachable!("Compiler returned Pending"),
    }
}

#[cfg(all(target_os = "android", not(feature = "remote")))]
compile_error!("The `remote` feature is required when building for Android");

#[cfg(all(target_os = "android", feature = "remote"))]
#[unsafe(no_mangle)]
fn android_main(app: i_slint_backend_android_activity::android_activity::AndroidApp) {
    *remote::ANDROID_DEVICE_NAME.lock().unwrap_or_else(|e| e.into_inner()) =
        android_device_name(&app);
    i_slint_core::platform::set_platform(Box::new(
        i_slint_backend_android_activity::AndroidPlatform::new(app),
    ))
    .unwrap();
    remote::run(None, true).unwrap();
}

/// Read the user-set device name from `Settings.Global.DEVICE_NAME` via JNI.
/// Returns `None` when the platform hasn't recorded a value (the setting is optional and
/// guaranteed populated only from Android 7.1 / API 25 onwards) or when the JNI call fails.
#[cfg(all(target_os = "android", feature = "remote"))]
fn android_device_name(
    app: &i_slint_backend_android_activity::android_activity::AndroidApp,
) -> Option<String> {
    use jni::JavaVM;
    use jni::objects::{JObject, JString, JValue};
    use jni::refs::Global;
    use jni::{jni_sig, jni_str};

    // Safety: documented contract of android-activity to obtain the JavaVM. `vm_as_ptr`
    // itself asserts the pointer is non-null, so this never proceeds with a null VM.
    let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr() as *mut _) };
    let result: jni::errors::Result<Option<String>> = vm.attach_current_thread(|env| {
        let activity_ptr = app.activity_as_ptr() as jni::sys::jobject;
        // Safety: `activity_as_ptr` returns an unowned global JNI reference that lives for
        // the duration of `app`. Wrapping it as `Global<JObject>` via `as_cast_raw` is the
        // pattern documented by android-activity and avoids treating a global as a local.
        let activity = unsafe { env.as_cast_raw::<Global<JObject>>(&activity_ptr)? };
        let resolver = env
            .call_method(
                activity.as_ref(),
                jni_str!("getContentResolver"),
                jni_sig!(() -> android.content.ContentResolver),
                &[],
            )?
            .l()?;
        let key = JString::new(env, "device_name")?;
        let value = env
            .call_static_method(
                jni_str!("android/provider/Settings$Global"),
                jni_str!("getString"),
                jni_sig!(
                    (resolver: android.content.ContentResolver, name: java.lang.String)
                        -> java.lang.String
                ),
                &[JValue::Object(&resolver), JValue::Object(&key)],
            )?
            .l()?;
        if value.is_null() {
            return Ok(None);
        }
        let value = JString::cast_local(env, value)?.try_to_string(env)?;
        Ok((!value.is_empty()).then_some(value))
    });
    match result {
        Ok(name) => name,
        Err(err) => {
            tracing::warn!("Failed reading Settings.Global.DEVICE_NAME via JNI: {err}");
            None
        }
    }
}
