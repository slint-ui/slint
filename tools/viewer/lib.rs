// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Embeddable entry points for the Slint Viewer.

mod debug;

#[cfg(not(any(target_os = "android", target_os = "ios")))]
mod simulator;
#[cfg(all(
    not(any(target_os = "android", target_os = "ios")),
    any(
        feature = "backend-default",
        feature = "backend-winit",
        feature = "backend-winit-x11",
        feature = "backend-winit-wayland"
    )
))]
mod simulator_resize;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
mod simulator_settings;

#[cfg(all(target_os = "android", not(feature = "remote")))]
compile_error!("The `remote` feature is required when building for Android");

#[cfg(all(target_os = "android", feature = "remote"))]
mod remote;

#[cfg(not(any(target_os = "android", target_os = "ios")))]
mod local_runner {
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    use i_slint_compiler::ComponentSelection;
    use slint_interpreter::ComponentInstance;

    use super::{debug, simulator};

    /// A structured lifecycle message from an embedded local viewer.
    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct ViewerLogMessage {
        pub level: ViewerLogLevel,
        pub message: String,
    }

    /// The severity of an embedded viewer lifecycle message.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum ViewerLogLevel {
        Error,
        Information,
    }

    /// A callback that receives embedded viewer lifecycle messages.
    pub type ViewerLogSink = Arc<dyn Fn(ViewerLogMessage) + Send + Sync>;

    /// Options for the reusable auto-reload simulator runner.
    #[derive(Clone)]
    pub struct ViewerRunnerOptions {
        pub entry_file: PathBuf,
        pub component: Option<String>,
        pub style: Option<String>,
        pub log_sink: Option<ViewerLogSink>,
    }

    impl ViewerRunnerOptions {
        /// Create runner options for an entry file and component.
        pub fn new(entry_file: PathBuf, component: Option<String>) -> Self {
            Self { entry_file, component, style: None, log_sink: None }
        }
    }

    /// An embedded viewer failure.
    pub type ViewerRunnerError = anyhow::Error;

    pub(super) type Result<T> = std::result::Result<T, ViewerRunnerError>;

    #[derive(Clone)]
    pub(super) struct Cli {
        path: PathBuf,
        pub(super) component: Option<String>,
        style: Option<String>,
        pub(super) on: Vec<String>,
        pub(super) load_data: Option<PathBuf>,
    }

    impl Cli {
        pub(super) fn path(&self) -> &Path {
            &self.path
        }
    }

    static EXIT_CODE: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);

    /// Run the auto-reload simulator on the calling process's main thread.
    pub fn run_auto_reload_simulator(
        options: ViewerRunnerOptions,
    ) -> std::result::Result<i32, ViewerRunnerError> {
        log(
            &options.log_sink,
            ViewerLogLevel::Information,
            format!("Starting local viewer for {}", options.entry_file.display()),
        );
        let args = Cli {
            path: options.entry_file,
            component: options.component,
            style: options.style,
            on: Vec::new(),
            load_data: None,
        };
        let result = run(&args);
        match result {
            Ok(()) => {
                log(&options.log_sink, ViewerLogLevel::Information, "Local viewer exited".into());
                Ok(EXIT_CODE.load(std::sync::atomic::Ordering::Relaxed))
            }
            Err(error) => {
                log(&options.log_sink, ViewerLogLevel::Error, error.to_string());
                Err(error)
            }
        }
    }

    fn run(args: &Cli) -> Result<()> {
        EXIT_CODE.store(0, std::sync::atomic::Ordering::Relaxed);
        slint_interpreter::BackendSelector::new().select()?;
        let _ = i_slint_backend_selector::with_global_context(|context| {
            context.set_log_message_handler(Some(Box::new(move |message| {
                debug::log_message_handler(&message);
            })))
        })?;

        let mut compiler = slint_interpreter::Compiler::new();
        if let Some(style) = &args.style {
            compiler.set_style(style.clone());
        }
        compiler.compiler_configuration(i_slint_core::InternalToken).components_to_generate =
            match &args.component {
                Some(component) => ComponentSelection::Named(component.clone()),
                None => ComponentSelection::LastExported,
            };
        simulator::run(compiler, args)
    }

    pub(super) fn setup_instance(
        instance: &ComponentInstance,
        callbacks: &[String],
        load_data_path: Option<&Path>,
    ) -> Result<()> {
        debug_assert!(callbacks.is_empty());
        debug_assert!(load_data_path.is_none());
        init_dialog(instance);
        Ok(())
    }

    fn init_dialog(instance: &ComponentInstance) {
        for callback in instance.definition().callbacks() {
            let exit_code = match callback.as_str() {
                "ok-clicked" | "yes-clicked" | "close-clicked" => 0,
                "cancel-clicked" | "no-clicked" => 1,
                _ => continue,
            };
            EXIT_CODE.store(1, std::sync::atomic::Ordering::Relaxed);
            instance
                .set_callback(&callback, move |_| {
                    EXIT_CODE.store(exit_code, std::sync::atomic::Ordering::Relaxed);
                    i_slint_core::api::quit_event_loop().unwrap();
                    Default::default()
                })
                .unwrap();
        }
    }

    pub(super) fn watchable_path(path: &Path) -> Option<PathBuf> {
        (path != Path::new("-")).then(|| path.to_path_buf())
    }

    fn log(sink: &Option<ViewerLogSink>, level: ViewerLogLevel, message: String) {
        if let Some(sink) = sink {
            sink(ViewerLogMessage { level, message });
        }
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[doc(hidden)]
pub use local_runner::{
    ViewerLogLevel, ViewerLogMessage, ViewerLogSink, ViewerRunnerError, ViewerRunnerOptions,
    run_auto_reload_simulator,
};

#[cfg(not(any(target_os = "android", target_os = "ios")))]
use local_runner::{Cli, Result, setup_instance, watchable_path};

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
