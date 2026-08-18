// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! This is an internal module that contains the [`LiveReloadingComponent`] struct.

use crate::file_watcher::FileWatcher;
use core::cell::RefCell;
use core::task::Waker;
use i_slint_core::api::{ComponentHandle, PlatformError};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

//re-export for the generated code:
pub use i_slint_compiler::rust_interface::{
    RustInterfaceDescriptor, RustInterfaceDiff, RustInterfaceEntry,
};
pub use slint_interpreter::{Compiler, ComponentInstance, DefaultTranslationContext, Value};

/// The outcome of compiling and applying a changed Slint component.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LiveReloadResult {
    /// The candidate component replaced the active instance.
    Applied,
    /// The candidate couldn't be compiled, instantiated, or restored.
    CompileError,
    /// The candidate changes the Rust API and requires a Cargo rebuild.
    RequiresRebuild { diff: RustInterfaceDiff },
}

/// This struct is used to compile and instantiate a component from a .slint file on disk.
/// The file is watched for changes and the component is recompiled and instantiated
pub struct LiveReloadingComponent {
    // because new_cyclic cannot return error, we need to initialize the instance after
    instance: Option<ComponentInstance>,
    // Kept so the FFI can return a stable reference; the window is reused across reloads.
    window_adapter: Option<Rc<dyn i_slint_core::window::WindowAdapter>>,
    watcher: Arc<Mutex<Watcher>>,
    compiler: Compiler,
    file_name: PathBuf,
    component_name: Option<String>,
    compiled_interface: Option<RustInterfaceDescriptor>,
    properties: RefCell<HashMap<String, Value>>,
    callbacks: RefCell<HashMap<String, Rc<dyn Fn(&[Value]) -> Value + 'static>>>,
    post_reload_hook: Option<Box<dyn Fn(&ComponentInstance)>>,
    extra_watch_paths: Vec<PathBuf>,
}

impl LiveReloadingComponent {
    /// Compile and instantiate a component from the specified .slint file and component.
    pub fn new(
        mut compiler: Compiler,
        file_name: PathBuf,
        component_name: Option<String>,
        compiled_interface: Option<RustInterfaceDescriptor>,
    ) -> Result<Rc<RefCell<Self>>, PlatformError> {
        compiler.set_embed_resources(i_slint_compiler::EmbedResourcesKind::ListAllResources);

        let self_rc = Rc::<RefCell<Self>>::new_cyclic(move |self_weak| {
            let watcher = Watcher::new(self_weak.clone());
            RefCell::new(Self {
                instance: None,
                window_adapter: None,
                watcher,
                compiler,
                file_name,
                component_name,
                compiled_interface,
                properties: Default::default(),
                callbacks: Default::default(),
                post_reload_hook: None,
                extra_watch_paths: Vec::new(),
            })
        });

        let mut self_mut = self_rc.borrow_mut();
        let result = self_mut.build();
        result.print_diagnostics();
        if result.has_errors() {
            return Err(format!("Could not compile {}", self_mut.file_name.display()).into());
        }
        let definition = self_mut.find_component(&result).ok_or_else(|| -> PlatformError {
            format!("No component found in {}", self_mut.file_name.display()).into()
        })?;
        let instance = definition.create()?;
        self_mut.window_adapter =
            Some(i_slint_core::window::WindowInner::from_pub(instance.window()).window_adapter());
        self_mut.instance = Some(instance);
        drop(self_mut);
        Ok(self_rc)
    }

    /// Return the Rust interface that was compiled into the host application.
    pub fn compiled_interface(&self) -> Option<&RustInterfaceDescriptor> {
        self.compiled_interface.as_ref()
    }

    /// Compile the component from disk and apply it when its Rust interface still matches.
    pub fn reload(&mut self) -> LiveReloadResult {
        let result = self.build();
        result.print_diagnostics();
        if result.has_errors() {
            return LiveReloadResult::CompileError;
        }

        if let Some(compiled_interface) = &self.compiled_interface {
            let Some(candidate_interface) = result.rust_interface(i_slint_core::InternalToken)
            else {
                eprintln!("The live compiler didn't produce a Rust interface descriptor");
                return LiveReloadResult::CompileError;
            };
            let diff = compiled_interface.diff(candidate_interface);
            if !diff.is_empty() {
                eprintln!("The Slint Rust interface changed and requires a rebuild:\n{diff}");
                return LiveReloadResult::RequiresRebuild { diff };
            }
        }

        let Some(definition) = self.find_component(&result) else {
            eprintln!(
                "Component {} not found",
                self.component_name.as_deref().unwrap_or("<default>")
            );
            return LiveReloadResult::CompileError;
        };
        if let Err(error) = self.validate_restore_targets(&definition) {
            eprintln!("The changed component can't restore the active Rust state: {error}");
            return LiveReloadResult::CompileError;
        }

        let instance = match definition.create_with_existing_window(self.instance().window()) {
            Ok(instance) => instance,
            Err(error) => {
                eprintln!("Error while creating the component: {error}");
                let _ = self.instance().show();
                return LiveReloadResult::CompileError;
            }
        };
        if let Err(error) = self.restore_properties_and_callbacks(&instance) {
            eprintln!("Error while restoring the component state: {error}");
            let _ = self.instance().show();
            return LiveReloadResult::CompileError;
        }
        self.instance = Some(instance);
        eprintln!(
            "Reloaded component {} from {}",
            self.component_name.as_deref().unwrap_or("<default>"),
            self.file_name.display()
        );
        LiveReloadResult::Applied
    }

    fn find_component(
        &self,
        result: &slint_interpreter::CompilationResult,
    ) -> Option<slint_interpreter::ComponentDefinition> {
        match &self.component_name {
            Some(name) => result.component(name),
            None => result.components().next(),
        }
    }

    fn build(&self) -> slint_interpreter::CompilationResult {
        let mut future = core::pin::pin!(self.compiler.build_from_path(&self.file_name));
        let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
        let std::task::Poll::Ready(result) = std::future::Future::poll(future.as_mut(), &mut cx)
        else {
            unreachable!("Compiler returned Pending")
        };
        Watcher::update_watched_paths(
            &self.watcher,
            std::iter::once(self.file_name.clone())
                .chain(result.watch_paths(i_slint_core::InternalToken).iter().cloned())
                .chain(self.extra_watch_paths.iter().cloned()),
        );
        result
    }

    fn validate_restore_targets(
        &self,
        definition: &slint_interpreter::ComponentDefinition,
    ) -> Result<(), String> {
        for (name, value) in self.properties.borrow().iter() {
            if let Some((global, prop)) = name.split_once('.') {
                let Some(mut properties) = definition.global_properties(global) else {
                    return Err(format!("global '{global}' no longer exists"));
                };
                let Some((_, candidate_type)) = properties.find(|(name, _)| name == prop) else {
                    return Err(format!("property '{name}' no longer exists"));
                };
                if candidate_type != value.value_type() {
                    return Err(format!("property '{name}' changed type"));
                }
            } else {
                let Some((_, candidate_type)) =
                    definition.properties().find(|(candidate, _)| candidate == name)
                else {
                    return Err(format!("property '{name}' no longer exists"));
                };
                if candidate_type != value.value_type() {
                    return Err(format!("property '{name}' changed type"));
                }
            }
        }
        for name in self.callbacks.borrow().keys() {
            let exists = if let Some((global, callback)) = name.split_once('.') {
                definition
                    .global_callbacks(global)
                    .is_some_and(|mut callbacks| callbacks.any(|candidate| candidate == callback))
            } else {
                definition.callbacks().any(|candidate| candidate == *name)
            };
            if !exists {
                return Err(format!("callback '{name}' no longer exists"));
            }
        }
        Ok(())
    }

    fn restore_properties_and_callbacks(&self, instance: &ComponentInstance) -> Result<(), String> {
        for (name, value) in self.properties.borrow().iter() {
            if let Some((global, prop)) = name.split_once('.') {
                instance
                    .set_global_property(global, prop, value.clone())
                    .map_err(|error| format!("cannot set property '{name}': {error}"))?;
            } else {
                instance
                    .set_property(name, value.clone())
                    .map_err(|error| format!("cannot set property '{name}': {error}"))?;
            }
        }
        for (name, callback) in self.callbacks.borrow().iter() {
            let callback = callback.clone();
            if let Some((global, prop)) = name.split_once('.') {
                instance
                    .set_global_callback(global, prop, move |args| callback(args))
                    .map_err(|error| format!("cannot set callback '{name}': {error}"))?;
            } else {
                instance
                    .set_callback(name, move |args| callback(args))
                    .map_err(|error| format!("cannot set callback '{name}': {error}"))?;
            }
        }
        Ok(())
    }

    /// Set a hook that runs after each successful reload, receiving the new instance.
    pub fn set_post_reload_hook(&mut self, hook: impl Fn(&ComponentInstance) + 'static) {
        self.post_reload_hook = Some(Box::new(hook));
    }

    /// Set extra file paths to watch in addition to the .slint file and its imports.
    pub fn set_extra_watch_paths(&mut self, paths: Vec<PathBuf>) {
        self.extra_watch_paths = paths;
    }

    /// Return the instance
    pub fn instance(&self) -> &ComponentInstance {
        self.instance.as_ref().expect("always set after Self is created from Rc::new_cyclic")
    }

    /// Set a property and remember its value for when the component is reloaded
    pub fn set_property(&self, name: &str, value: Value) {
        self.properties.borrow_mut().insert(name.into(), value.clone());
        self.instance()
            .set_property(name, value)
            .unwrap_or_else(|e| panic!("Cannot set property {name}: {e}"))
    }

    /// Forward to get_property
    pub fn get_property(&self, name: &str) -> Value {
        self.instance()
            .get_property(name)
            .unwrap_or_else(|e| panic!("Cannot get property {name}: {e}"))
    }

    /// Forward to invoke
    pub fn invoke(&self, name: &str, args: &[Value]) -> Value {
        self.instance()
            .invoke(name, args)
            .unwrap_or_else(|e| panic!("Cannot invoke callback {name}: {e}"))
    }

    /// Forward to set_callback
    pub fn set_callback(&self, name: &str, callback: Rc<dyn Fn(&[Value]) -> Value + 'static>) {
        self.callbacks.borrow_mut().insert(name.into(), callback.clone());
        self.instance()
            .set_callback(name, move |args| callback(args))
            .unwrap_or_else(|e| panic!("Cannot set callback {name}: {e}"));
    }

    /// forward to set_global_property
    pub fn set_global_property(&self, global_name: &str, name: &str, value: Value) {
        self.properties.borrow_mut().insert(format!("{global_name}.{name}"), value.clone());
        self.instance()
            .set_global_property(global_name, name, value)
            .unwrap_or_else(|e| panic!("Cannot set property {global_name}::{name}: {e}"))
    }

    /// forward to get_global_property
    pub fn get_global_property(&self, global_name: &str, name: &str) -> Value {
        self.instance()
            .get_global_property(global_name, name)
            .unwrap_or_else(|e| panic!("Cannot get property {global_name}::{name}: {e}"))
    }

    /// Forward to invoke_global
    pub fn invoke_global(&self, global_name: &str, name: &str, args: &[Value]) -> Value {
        self.instance()
            .invoke_global(global_name, name, args)
            .unwrap_or_else(|e| panic!("Cannot invoke callback {global_name}::{name}: {e}"))
    }

    /// Forward to set_global_callback
    pub fn set_global_callback(
        &self,
        global_name: &str,
        name: &str,
        callback: Rc<dyn Fn(&[Value]) -> Value + 'static>,
    ) {
        self.callbacks.borrow_mut().insert(format!("{global_name}.{name}"), callback.clone());
        self.instance()
            .set_global_callback(global_name, name, move |args| callback(args))
            .unwrap_or_else(|e| panic!("Cannot set callback {global_name}::{name}: {e}"));
    }
}

enum WatcherState {
    Starting,
    /// The file system watcher notified the main thread of a change
    Changed,
    /// The main thread is waiting for the next event
    Waiting(Waker),
}

struct Watcher {
    // (wouldn't need to be an option if new_cyclic() could return errors)
    watcher: Option<FileWatcher>,
    state: WatcherState,
}

impl Watcher {
    fn new(component_weak: std::rc::Weak<RefCell<LiveReloadingComponent>>) -> Arc<Mutex<Self>> {
        let arc = Arc::new(Mutex::new(Self { state: WatcherState::Starting, watcher: None }));

        let watcher_weak = Arc::downgrade(&arc);
        let result = slint_interpreter::spawn_local(std::future::poll_fn(move |cx| {
            let (Some(instance), Some(watcher)) =
                (component_weak.upgrade(), watcher_weak.upgrade())
            else {
                // When the instance is dropped, we can stop this future
                return std::task::Poll::Ready(());
            };
            let state = std::mem::replace(
                &mut watcher.lock().unwrap().state,
                WatcherState::Waiting(cx.waker().clone()),
            );
            if matches!(state, WatcherState::Changed) {
                if instance.borrow_mut().reload() == LiveReloadResult::Applied {
                    let borrowed = instance.borrow();
                    if let Some(hook) = &borrowed.post_reload_hook {
                        hook(borrowed.instance());
                    }
                }
            };
            std::task::Poll::Pending
        }));

        // no event loop, no need to start a watcher
        if result.is_err() {
            return arc;
        }

        let watcher_weak = Arc::downgrade(&arc);
        arc.lock().unwrap().watcher = FileWatcher::start(
            move |_event| {
                let Some(watcher) = watcher_weak.upgrade() else { return };
                if let WatcherState::Waiting(waker) =
                    std::mem::replace(&mut watcher.lock().unwrap().state, WatcherState::Changed)
                {
                    // Wait a bit to let the time to write multiple files
                    std::thread::sleep(crate::REBUILD_DEBOUNCE);
                    waker.wake();
                }
            },
            move |err| eprintln!("Warning: file watcher error: {err}"),
        )
        .ok();
        arc
    }

    fn update_watched_paths<I>(self_: &Mutex<Self>, paths: I)
    where
        I: IntoIterator<Item = PathBuf>,
    {
        let mut locked = self_.lock().unwrap();
        let Some(mut watcher) = locked.watcher.take() else { return };
        drop(locked);
        if let Err(err) = watcher.update_watched_paths(paths) {
            eprintln!("Warning: error while updating file watcher paths: {err:?}");
        }
        self_.lock().unwrap().watcher = Some(watcher);
    }
}

#[cfg(feature = "ffi")]
mod ffi {
    use super::*;
    use core::ffi::c_void;
    use i_slint_core::window::WindowAdapter;
    use i_slint_core::{SharedString, SharedVector, slice::Slice};
    type LiveReloadingComponentInner = RefCell<LiveReloadingComponent>;

    #[unsafe(no_mangle)]
    /// LibraryPath is an array of string that have in the form `lib=...`
    pub extern "C" fn slint_live_preview_new(
        file_name: Slice<u8>,
        component_name: Slice<u8>,
        include_paths: &SharedVector<SharedString>,
        library_paths: &SharedVector<SharedString>,
        style: Slice<u8>,
        translation_domain: Slice<u8>,
        no_default_translation_context: bool,
    ) -> *const LiveReloadingComponentInner {
        let mut compiler = Compiler::default();
        compiler.set_include_paths(
            include_paths.iter().map(|path| PathBuf::from(path.as_str())).collect(),
        );
        compiler.set_library_paths(
            library_paths
                .iter()
                .map(|path| path.as_str().split_once('=').expect("library path must have an '='"))
                .map(|(lib, path)| (lib.into(), PathBuf::from(path)))
                .collect(),
        );
        if !style.is_empty() {
            compiler.set_style(std::str::from_utf8(&style).unwrap().into());
        }
        if !translation_domain.is_empty() {
            compiler
                .set_translation_domain(std::str::from_utf8(&translation_domain).unwrap().into());
        }
        if no_default_translation_context {
            compiler.set_default_translation_context(DefaultTranslationContext::None);
        }
        Rc::into_raw(
            LiveReloadingComponent::new(
                compiler,
                std::path::PathBuf::from(std::str::from_utf8(&file_name).unwrap()),
                Some(std::str::from_utf8(&component_name).unwrap().into()),
                None,
            )
            .expect("Creating the component failed"),
        )
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_live_preview_clone(
        component: *const LiveReloadingComponentInner,
    ) {
        unsafe { Rc::increment_strong_count(component) };
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_live_preview_drop(
        component: *const LiveReloadingComponentInner,
    ) {
        unsafe { Rc::decrement_strong_count(component) };
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn slint_live_preview_set_property(
        component: &LiveReloadingComponentInner,
        property: Slice<u8>,
        value: &Value,
    ) {
        let property = std::str::from_utf8(&property).unwrap();
        if let Some((global, prop)) = property.split_once('.') {
            component.borrow_mut().set_global_property(global, prop, value.clone());
        } else {
            component.borrow_mut().set_property(property, value.clone());
        }
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn slint_live_preview_get_property(
        component: &LiveReloadingComponentInner,
        property: Slice<u8>,
    ) -> *mut Value {
        let property = std::str::from_utf8(&property).unwrap();
        let val = if let Some((global, prop)) = property.split_once('.') {
            component.borrow().get_global_property(global, prop)
        } else {
            component.borrow().get_property(property)
        };
        Box::into_raw(Box::new(val))
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn slint_live_preview_invoke(
        component: &LiveReloadingComponentInner,
        callback: Slice<u8>,
        args: Slice<Box<Value>>,
    ) -> *mut Value {
        let callback = std::str::from_utf8(&callback).unwrap();
        let args = args.iter().map(|vb| vb.as_ref().clone()).collect::<Vec<_>>();
        let val = if let Some((global, prop)) = callback.split_once('.') {
            component.borrow().invoke_global(global, prop, &args)
        } else {
            component.borrow().invoke(callback, &args)
        };
        Box::into_raw(Box::new(val))
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_live_preview_set_callback(
        component: &LiveReloadingComponentInner,
        callback: Slice<u8>,
        callback_handler: extern "C" fn(
            user_data: *mut c_void,
            arg: Slice<Box<Value>>,
        ) -> Box<Value>,
        user_data: *mut c_void,
        drop_user_data: Option<extern "C" fn(*mut c_void)>,
    ) {
        let ud = unsafe {
            slint_interpreter::ffi::CallbackUserData::new(
                user_data,
                drop_user_data,
                callback_handler,
            )
        };
        let handler = Rc::new(move |args: &[Value]| ud.call(args));
        let callback = std::str::from_utf8(&callback).unwrap();
        if let Some((global, prop)) = callback.split_once('.') {
            component.borrow_mut().set_global_callback(global, prop, handler);
        } else {
            component.borrow_mut().set_callback(callback, handler);
        }
    }

    /// Return a borrowed pointer to the component's window adapter (valid while the component lives).
    #[unsafe(no_mangle)]
    pub extern "C" fn slint_live_preview_window(
        component: &LiveReloadingComponentInner,
    ) -> *const i_slint_core::window::ffi::WindowAdapterRcOpaque {
        assert_eq!(
            core::mem::size_of::<Rc<dyn WindowAdapter>>(),
            core::mem::size_of::<i_slint_core::window::ffi::WindowAdapterRcOpaque>()
        );
        let borrow = component.borrow();
        let adapter = borrow.window_adapter.as_ref().unwrap();
        (adapter as *const Rc<dyn WindowAdapter>).cast()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::future::Future;
    use std::path::Path;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    struct TestProject {
        directory: PathBuf,
        entry: PathBuf,
    }

    impl TestProject {
        fn new(source: &str) -> Self {
            let directory = std::env::temp_dir().join(format!(
                "slint-live-reload-{}-{}",
                std::process::id(),
                NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&directory).unwrap();
            let entry = directory.join("app.slint");
            std::fs::write(&entry, source).unwrap();
            Self { directory, entry }
        }

        fn write(&self, source: &str) {
            std::fs::write(&self.entry, source).unwrap();
        }
    }

    impl Drop for TestProject {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.directory);
        }
    }

    fn poll_ready<T>(future: impl Future<Output = T>) -> T {
        let mut future = core::pin::pin!(future);
        let mut context = std::task::Context::from_waker(std::task::Waker::noop());
        let std::task::Poll::Ready(result) = Future::poll(future.as_mut(), &mut context) else {
            panic!("compiler returned Pending")
        };
        result
    }

    fn compiled_interface(path: &Path) -> RustInterfaceDescriptor {
        let compiler = Compiler::default();
        let result = poll_ready(compiler.build_from_path(path));
        assert!(!result.has_errors());
        result.rust_interface(i_slint_core::InternalToken).unwrap().clone()
    }

    fn source(label: &str, property: &str) -> String {
        format!(
            r#"
                export global Settings {{
                    in-out property <int> threshold: 2;
                    callback adjust(int) -> int;
                }}
                export component App inherits Window {{
                    in-out property <int> {property}: 1;
                    callback ping(int) -> int;
                    Text {{ text: "{label}"; }}
                }}
            "#
        )
    }

    fn number(value: Value) -> f64 {
        let Value::Number(value) = value else { panic!("expected a number") };
        value
    }

    #[test]
    fn reload_outcomes_preserve_the_last_successful_instance() {
        i_slint_backend_testing::init_no_event_loop();
        let project = TestProject::new(&source("before", "count"));
        let interface = compiled_interface(&project.entry);
        let live = LiveReloadingComponent::new(
            Compiler::default(),
            project.entry.clone(),
            Some("App".into()),
            Some(interface),
        )
        .unwrap();
        let window = live.borrow().instance().window() as *const _;

        live.borrow().set_property("count", Value::Number(42.));
        live.borrow().set_global_property("Settings", "threshold", Value::Number(7.));
        live.borrow().set_callback(
            "ping",
            Rc::new(|arguments| {
                let [Value::Number(value)] = arguments else { panic!("invalid arguments") };
                Value::Number(value + 1.)
            }),
        );
        live.borrow().set_global_callback(
            "Settings",
            "adjust",
            Rc::new(|arguments| {
                let [Value::Number(value)] = arguments else { panic!("invalid arguments") };
                Value::Number(value * 2.)
            }),
        );

        project.write(&source("after", "count"));
        assert_eq!(live.borrow_mut().reload(), LiveReloadResult::Applied);
        assert_eq!(number(live.borrow().get_property("count")), 42.);
        assert_eq!(number(live.borrow().get_global_property("Settings", "threshold")), 7.);
        assert_eq!(number(live.borrow().invoke("ping", &[Value::Number(5.)])), 6.);
        assert_eq!(
            number(live.borrow().invoke_global("Settings", "adjust", &[Value::Number(5.)])),
            10.
        );

        project.write("export component App inherits Window { @ }");
        assert_eq!(live.borrow_mut().reload(), LiveReloadResult::CompileError);
        assert_eq!(number(live.borrow().get_property("count")), 42.);
        assert_eq!(number(live.borrow().invoke("ping", &[Value::Number(5.)])), 6.);

        project.write(&source("rebuild", "total"));
        let LiveReloadResult::RequiresRebuild { diff } = live.borrow_mut().reload() else {
            panic!("the interface change should require a rebuild")
        };
        let diff = diff.to_string();
        assert!(diff.contains("- component App/property count: in-out int"));
        assert!(diff.contains("+ component App/property total: in-out int"));
        assert_eq!(number(live.borrow().get_property("count")), 42.);
        assert_eq!(number(live.borrow().invoke("ping", &[Value::Number(5.)])), 6.);
        assert_eq!(live.borrow().instance().window() as *const _, window);

        project.write(&source("viewer", "count"));
        let viewer = LiveReloadingComponent::new(
            Compiler::default(),
            project.entry.clone(),
            Some("App".into()),
            None,
        )
        .unwrap();
        viewer.borrow().set_property("count", Value::Number(9.));
        project.write(&source("viewer changed", "total"));
        assert_eq!(viewer.borrow_mut().reload(), LiveReloadResult::CompileError);
        assert_eq!(number(viewer.borrow().get_property("count")), 9.);
    }
}
