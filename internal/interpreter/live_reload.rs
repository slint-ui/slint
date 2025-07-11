// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! This is an internal module that contains the [`LiveReloadingComponent`] struct.

use crate::dynamic_item_tree::WindowOptions;
use core::cell::RefCell;
use core::task::Waker;
use i_slint_core::api::{ComponentHandle, PlatformError};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

//re-export for the generated code:
pub use crate::{Compiler, ComponentInstance, Value};

/// This struct is used to compile and instantiate a component from a .slint file on disk.
/// The file is watched for changes and the component is recompiled and instantiated
pub struct LiveReloadingComponent {
    // because new_cyclic cannot return error, we need to initialize the instance after
    instance: Option<ComponentInstance>,
    compiler: Compiler,
    file_name: PathBuf,
    component_name: String,
    properties: HashMap<String, Value>,
    callbacks: HashMap<String, Rc<dyn Fn(&[Value]) -> Value + 'static>>,
}

impl LiveReloadingComponent {
    /// Compile and instantiate a component from the specified .slint file and component.
    pub fn new(
        mut compiler: Compiler,
        file_name: PathBuf,
        component_name: String,
    ) -> Result<Rc<RefCell<Self>>, PlatformError> {
        let self_rc = Rc::<RefCell<Self>>::new_cyclic(move |self_weak| {
            let watcher = Watcher::new(self_weak.clone());
            if watcher.lock().unwrap().watcher.is_some() {
                let watcher_clone = watcher.clone();
                compiler.set_file_loader(move |path| {
                    watcher_clone.lock().unwrap().watch(path);
                    Box::pin(async { None })
                });
                watcher.lock().unwrap().watch(&file_name);
            }
            RefCell::new(Self {
                instance: None,
                compiler,
                file_name,
                component_name,
                properties: Default::default(),
                callbacks: Default::default(),
            })
        });

        let mut self_mut = self_rc.borrow_mut();
        let result = {
            let mut future =
                core::pin::pin!(self_mut.compiler.build_from_path(&self_mut.file_name));
            let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
            let std::task::Poll::Ready(result) =
                std::future::Future::poll(future.as_mut(), &mut cx)
            else {
                unreachable!("Compiler returned Pending")
            };
            result
        };
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
        let result = {
            let mut future = core::pin::pin!(self.compiler.build_from_path(&self.file_name));
            let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
            let std::task::Poll::Ready(result) =
                std::future::Future::poll(future.as_mut(), &mut cx)
            else {
                unreachable!("Compiler returned Pending")
            };
            result
        };
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

        // Set the properties
        for (name, value) in self.properties.iter() {
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
        for (name, callback) in self.callbacks.iter() {
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

        true
    }

    /// Return the instance
    pub fn instance(&self) -> &ComponentInstance {
        &self.instance.as_ref().expect("always set after Self is created from Rc::new_cyclic")
    }

    /// Set a property and remember its value for when the component is reloaded
    pub fn set_property(&mut self, name: &str, value: Value) {
        self.properties.insert(name.into(), value.clone());
        self.instance()
            .set_property(&name, value)
            .unwrap_or_else(|e| panic!("Cannot set property {name}: {e}"))
    }

    /// Forward to get_property
    pub fn get_property(&self, name: &str) -> Value {
        self.instance()
            .get_property(&name)
            .unwrap_or_else(|e| panic!("Cannot get property {name}: {e}"))
    }

    /// Forward to invoke
    pub fn invoke(&self, name: &str, args: &[Value]) -> Value {
        self.instance()
            .invoke(name, args)
            .unwrap_or_else(|e| panic!("Cannot invoke callback {name}: {e}"))
    }

    /// Forward to set_callback
    pub fn set_callback(&mut self, name: &str, callback: Rc<dyn Fn(&[Value]) -> Value + 'static>) {
        self.callbacks.insert(name.into(), callback.clone());
        self.instance()
            .set_callback(&name, move |args| callback(args))
            .unwrap_or_else(|e| panic!("Cannot set callback {name}: {e}"));
    }

    /// forward to set_global_property
    pub fn set_global_property(&mut self, global_name: &str, name: &str, value: Value) {
        self.properties.insert(format!("{global_name}.{name}"), value.clone());
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
        &mut self,
        global_name: &str,
        name: &str,
        callback: Rc<dyn Fn(&[Value]) -> Value + 'static>,
    ) {
        self.callbacks.insert(format!("{global_name}.{name}"), callback.clone());
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
    watcher: Option<notify::RecommendedWatcher>,
    state: WatcherState,
    files: HashSet<PathBuf>,
}

impl Watcher {
    fn new(component_weak: std::rc::Weak<RefCell<LiveReloadingComponent>>) -> Arc<Mutex<Self>> {
        let arc = Arc::new(Mutex::new(Self {
            state: WatcherState::Starting,
            watcher: None,
            files: Default::default(),
        }));

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
                instance.borrow_mut().reload();
            };
            std::task::Poll::Pending
        }));

        // no event loop, no need to start a watcher
        if !result.is_ok() {
            return arc;
        }

        let watcher_weak = Arc::downgrade(&arc);
        arc.lock().unwrap().watcher =
            notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
                use notify::{event::ModifyKind, EventKind as K};
                let Ok(event) = event else { return };
                let Some(watcher) = watcher_weak.upgrade() else { return };
                if matches!(event.kind, K::Modify(ModifyKind::Data(_)) | K::Create(_))
                    && watcher.lock().is_ok_and(|w| event.paths.iter().any(|p| w.files.contains(p)))
                {
                    if let WatcherState::Waiting(waker) =
                        std::mem::replace(&mut watcher.lock().unwrap().state, WatcherState::Changed)
                    {
                        // Wait a bit to let the time to write multiple files
                        std::thread::sleep(std::time::Duration::from_millis(15));
                        waker.wake();
                    }
                }
            })
            .ok();
        arc
    }

    fn watch(&mut self, path: &Path) {
        let Some(watcher) = self.watcher.as_mut() else { return };
        let Some(parent) = path.parent() else { return };
        notify::Watcher::watch(watcher, parent, notify::RecursiveMode::NonRecursive)
            .unwrap_or_else(|err| {
                eprintln!("Warning: error while watching {}: {:?}", path.display(), err)
            });
        self.files.insert(path.into());
    }
}

#[cfg(feature = "ffi")]
mod ffi {
    use super::*;
    use core::ffi::c_void;
    use i_slint_core::window::WindowAdapter;
    use i_slint_core::{slice::Slice, SharedString, SharedVector};
    type LiveReloadingComponentInner = RefCell<LiveReloadingComponent>;

    #[unsafe(no_mangle)]
    /// LibraryPath is an array of string that have in the form `lib=...`
    pub extern "C" fn slint_live_reload_new(
        file_name: Slice<u8>,
        component_name: Slice<u8>,
        include_paths: &SharedVector<SharedString>,
        library_paths: &SharedVector<SharedString>,
        style: Slice<u8>,
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
    pub unsafe extern "C" fn slint_live_reload_clone(
        component: *const LiveReloadingComponentInner,
    ) {
        Rc::increment_strong_count(component);
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_live_reload_drop(component: *const LiveReloadingComponentInner) {
        Rc::decrement_strong_count(component);
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn slint_live_reload_set_property(
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
    pub extern "C" fn slint_live_reload_get_property(
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
    pub extern "C" fn slint_live_reload_invoke(
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
    pub unsafe extern "C" fn slint_live_reload_set_callback(
        component: &LiveReloadingComponentInner,
        callback: Slice<u8>,
        callback_handler: extern "C" fn(
            user_data: *mut c_void,
            arg: Slice<Box<Value>>,
        ) -> Box<Value>,
        user_data: *mut c_void,
        drop_user_data: Option<extern "C" fn(*mut c_void)>,
    ) {
        let ud = crate::ffi::CallbackUserData::new(user_data, drop_user_data, callback_handler);
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
    pub unsafe extern "C" fn slint_live_reload_window(
        component: &LiveReloadingComponentInner,
        out: *mut *const i_slint_core::window::ffi::WindowAdapterRcOpaque,
    ) {
        assert_eq!(
            core::mem::size_of::<Rc<dyn WindowAdapter>>(),
            core::mem::size_of::<i_slint_core::window::ffi::WindowAdapterRcOpaque>()
        );
        let borrow = component.borrow();
        let adapter = borrow.instance().inner.window_adapter_ref().unwrap();
        core::ptr::write(out as *mut *const Rc<dyn WindowAdapter>, adapter as *const _)
    }
}
