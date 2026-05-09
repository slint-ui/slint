// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! This is an internal module that contains the [`LiveReloadingComponent`] struct.

use crate::dynamic_item_tree::WindowOptions;
use crate::file_watcher::FileWatcher;
use core::cell::RefCell;
use core::task::Waker;
use i_slint_core::api::{ComponentHandle, PlatformError};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

//re-export for the generated code:
pub use crate::{Compiler, ComponentInstance, DefaultTranslationContext, Value};

/// This struct is used to compile and instantiate a component from a .slint file on disk.
/// The file is watched for changes and the component is recompiled and instantiated
pub struct LiveReloadingComponent {
    // because new_cyclic cannot return error, we need to initialize the instance after
    instance: Option<ComponentInstance>,
    watcher: Arc<Mutex<Watcher>>,
    compiler: Compiler,
    file_name: PathBuf,
    component_name: String,
    properties: RefCell<HashMap<String, Value>>,
    callbacks: RefCell<HashMap<String, Rc<dyn Fn(&[Value]) -> Value + 'static>>>,
}

impl LiveReloadingComponent {
    /// Compile and instantiate a component from the specified .slint file and component.
    pub fn new(
        mut compiler: Compiler,
        file_name: PathBuf,
        component_name: String,
    ) -> Result<Rc<RefCell<Self>>, PlatformError> {
        compiler.set_embed_resources(i_slint_compiler::EmbedResourcesKind::ListAllResources);

        let self_rc = Rc::<RefCell<Self>>::new_cyclic(move |self_weak| {
            let watcher = Watcher::new(self_weak.clone());
            RefCell::new(Self {
                instance: None,
                watcher,
                compiler,
                file_name,
                component_name,
                properties: Default::default(),
                callbacks: Default::default(),
            })
        });

        let mut self_mut = self_rc.borrow_mut();
        let result = self_mut.build();
        #[cfg(feature = "display-diagnostics")]
        result.print_diagnostics();
        assert!(
            !result.has_errors(),
            "Was not able to compile the file {}. \n{:?}",
            self_mut.file_name.display(),
            result.diagnostics
        );
        let definition = result.component(&self_mut.component_name).expect("Cannot open component");
        let instance = definition.create()?;
        eprintln!(
            "Loaded component {} from {}",
            self_mut.component_name,
            self_mut.file_name.display()
        );
        self_mut.instance = Some(instance);
        drop(self_mut);
        Ok(self_rc)
    }

    /// Reload the component from the .slint file.
    /// If there is an error, it won't actually reload.
    /// Return false in case of errors
    pub fn reload(&mut self) -> bool {
        let result = self.build();
        #[cfg(feature = "display-diagnostics")]
        result.print_diagnostics();
        if result.has_errors() {
            return false;
        }

        if let Some(definition) = result.component(&self.component_name) {
            let window_adapter =
                i_slint_core::window::WindowInner::from_pub(self.instance().window())
                    .window_adapter();
            match definition.create_with_options(WindowOptions::UseExistingWindow(window_adapter)) {
                Ok(instance) => {
                    self.instance = Some(instance);
                }
                Err(e) => {
                    eprintln!("Error while creating the component: {e}");
                    return false;
                }
            }
        } else {
            eprintln!("Component {} not found", self.component_name);
            return false;
        }
        true
    }

    fn build(&self) -> crate::CompilationResult {
        let mut future = core::pin::pin!(self.compiler.build_from_path(&self.file_name));
        let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
        let std::task::Poll::Ready(result) = std::future::Future::poll(future.as_mut(), &mut cx)
        else {
            unreachable!("Compiler returned Pending")
        };
        Watcher::update_watched_paths(
            &self.watcher,
            std::iter::once(self.file_name.clone())
                .chain(result.watch_paths(i_slint_core::InternalToken).iter().cloned()),
        );
        result
    }

    /// Reload the properties and callbacks after a reload()
    pub fn reload_properties_and_callbacks(&self) {
        // Set the properties
        for (name, value) in self.properties.borrow_mut().iter() {
            if let Some((global, prop)) = name.split_once('.') {
                self.instance()
                    .set_global_property(global, prop, value.clone())
                    .unwrap_or_else(|e| panic!("Cannot set property {name}: {e}"));
            } else {
                self.instance()
                    .set_property(name, value.clone())
                    .unwrap_or_else(|e| panic!("Cannot set property {name}: {e}"));
            }
        }
        for (name, callback) in self.callbacks.borrow_mut().iter() {
            let callback = callback.clone();
            if let Some((global, prop)) = name.split_once('.') {
                self.instance()
                    .set_global_callback(global, prop, move |args| callback(args))
                    .unwrap_or_else(|e| panic!("Cannot set callback {name}: {e}"));
            } else {
                self.instance()
                    .set_callback(name, move |args| callback(args))
                    .unwrap_or_else(|e| panic!("Cannot set callback {name}: {e}"));
            }
        }

        eprintln!("Reloaded component {} from {}", self.component_name, self.file_name.display());
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
        let result = crate::spawn_local(std::future::poll_fn(move |cx| {
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
                let success = instance.borrow_mut().reload();
                if success {
                    instance.borrow().reload_properties_and_callbacks();
                };
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
                    std::thread::sleep(std::time::Duration::from_millis(15));
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
            compiler.set_default_translation_context(crate::DefaultTranslationContext::None);
        }
        Rc::into_raw(
            LiveReloadingComponent::new(
                compiler,
                std::path::PathBuf::from(std::str::from_utf8(&file_name).unwrap()),
                std::str::from_utf8(&component_name).unwrap().into(),
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
            crate::ffi::CallbackUserData::new(user_data, drop_user_data, callback_handler)
        };
        let handler = Rc::new(move |args: &[Value]| ud.call(args));
        let callback = std::str::from_utf8(&callback).unwrap();
        if let Some((global, prop)) = callback.split_once('.') {
            component.borrow_mut().set_global_callback(global, prop, handler);
        } else {
            component.borrow_mut().set_callback(callback, handler);
        }
    }

    /// Same precondition as slint_interpreter_component_instance_window
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_live_preview_window(
        component: &LiveReloadingComponentInner,
        out: *mut *const i_slint_core::window::ffi::WindowAdapterRcOpaque,
    ) {
        assert_eq!(
            core::mem::size_of::<Rc<dyn WindowAdapter>>(),
            core::mem::size_of::<i_slint_core::window::ffi::WindowAdapterRcOpaque>()
        );
        let borrow = component.borrow();
        let adapter = borrow.instance().inner.window_adapter_ref().unwrap();
        unsafe { core::ptr::write(out as *mut *const Rc<dyn WindowAdapter>, adapter as *const _) };
    }
}
