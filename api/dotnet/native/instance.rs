// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::ffi::c_void;
#[cfg(feature = "live-preview")]
use std::{cell::RefCell, rc::Rc};

#[cfg(feature = "live-preview")]
use i_slint_live_preview::live_component::LiveReloadingComponent;
use slint_interpreter::{ComponentHandle, ComponentInstance, Value};

use crate::timer::ManagedAction;
use crate::{Utf8Str, guard, guard_bool, into_raw, release};

/// The `ComponentInstance` of .NET: a component, or one that reloads its `.slint` file when it changes.
pub enum DotnetInstance {
    Static(ComponentInstance),
    #[cfg(feature = "live-preview")]
    Live(Rc<RefCell<LiveReloadingComponent>>),
}

impl DotnetInstance {
    /// Calls `f` with the component; for a live component, the one from the latest reload.
    fn with<R>(&self, f: impl FnOnce(&ComponentInstance) -> R) -> R {
        match self {
            Self::Static(instance) => f(instance),
            #[cfg(feature = "live-preview")]
            Self::Live(live) => f(live.borrow().instance()),
        }
    }

    // A live component remembers what it sets, to set it again after a reload. It panics for a
    // member that doesn't exist, so each setter first sets the member on the instance, which fails instead.

    fn set_property(&self, global: Option<&str>, name: &str, value: &Value) -> Result<(), String> {
        let set = |instance: &ComponentInstance| match global {
            Some(global) => instance.set_global_property(global, name, value.clone()),
            None => instance.set_property(name, value.clone()),
        };
        self.with(set).map_err(|e| e.to_string())?;
        #[cfg(feature = "live-preview")]
        if let Self::Live(live) = self {
            match global {
                Some(global) => live.borrow().set_global_property(global, name, value.clone()),
                None => live.borrow().set_property(name, value.clone()),
            }
        }
        Ok(())
    }

    fn set_callback(
        &self,
        global: Option<&str>,
        name: &str,
        callback: ManagedCallback,
    ) -> Result<(), String> {
        let handler: std::rc::Rc<dyn Fn(&[Value]) -> Value> =
            std::rc::Rc::new(move |args: &[Value]| callback.call(args));
        let first = handler.clone();
        let set = |instance: &ComponentInstance| match global {
            Some(global) => instance
                .set_global_callback(global, name, move |args| first(args))
                .map_err(|e| e.to_string()),
            None => instance.set_callback(name, move |args| first(args)).map_err(|e| e.to_string()),
        };
        self.with(set)?;
        #[cfg(feature = "live-preview")]
        if let Self::Live(live) = self {
            match global {
                Some(global) => live.borrow().set_global_callback(global, name, handler),
                None => live.borrow().set_callback(name, handler),
            }
        }
        #[cfg(not(feature = "live-preview"))]
        drop(handler);
        Ok(())
    }

    fn run(&self) -> Result<(), String> {
        match self {
            Self::Static(instance) => instance.run().map_err(|e| e.to_string()),
            // A reload replaces the instance during the event loop, so no borrow may outlive this call.
            #[cfg(feature = "live-preview")]
            Self::Live(_) => {
                self.with(|instance| instance.show()).map_err(|e| e.to_string())?;
                slint_interpreter::run_event_loop().map_err(|e| e.to_string())?;
                self.with(|instance| instance.hide()).map_err(|e| e.to_string())
            }
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_component_instance_free(instance: *mut DotnetInstance) {
    unsafe { release(instance) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_component_instance_show(instance: &DotnetInstance) -> bool {
    guard_bool(|| instance.with(|i| i.show()).map_err(|e| e.to_string()))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_component_instance_hide(instance: &DotnetInstance) -> bool {
    guard_bool(|| instance.with(|i| i.hide()).map_err(|e| e.to_string()))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_component_instance_run(instance: &DotnetInstance) -> bool {
    guard_bool(|| instance.run())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_component_instance_get_property(
    instance: &DotnetInstance,
    name: Utf8Str,
) -> *mut Value {
    guard(std::ptr::null_mut(), || {
        let name = unsafe { name.as_str() }?;
        instance.with(|i| i.get_property(name)).map(into_raw).map_err(|e| e.to_string())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_component_instance_set_property(
    instance: &DotnetInstance,
    name: Utf8Str,
    value: &Value,
) -> bool {
    guard_bool(|| instance.set_property(None, unsafe { name.as_str() }?, value))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_component_instance_get_global_property(
    instance: &DotnetInstance,
    global: Utf8Str,
    name: Utf8Str,
) -> *mut Value {
    guard(std::ptr::null_mut(), || {
        let (global, name) = unsafe { (global.as_str()?, name.as_str()?) };
        instance
            .with(|i| i.get_global_property(global, name))
            .map(into_raw)
            .map_err(|e| e.to_string())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_component_instance_set_global_property(
    instance: &DotnetInstance,
    global: Utf8Str,
    name: Utf8Str,
    value: &Value,
) -> bool {
    guard_bool(|| {
        let (global, name) = unsafe { (global.as_str()?, name.as_str()?) };
        instance.set_property(Some(global), name, value)
    })
}

unsafe fn borrowed_args(args: *const *const Value, count: usize) -> Vec<Value> {
    if count == 0 {
        return Vec::new();
    }
    unsafe { std::slice::from_raw_parts(args, count) }
        .iter()
        .map(|v| unsafe { (**v).clone() })
        .collect()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_component_instance_invoke(
    instance: &DotnetInstance,
    name: Utf8Str,
    args: *const *const Value,
    count: usize,
) -> *mut Value {
    guard(std::ptr::null_mut(), || {
        let args = unsafe { borrowed_args(args, count) };
        let name = unsafe { name.as_str() }?;
        instance.with(|i| i.invoke(name, &args)).map(into_raw).map_err(|e| e.to_string())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_component_instance_invoke_global(
    instance: &DotnetInstance,
    global: Utf8Str,
    name: Utf8Str,
    args: *const *const Value,
    count: usize,
) -> *mut Value {
    guard(std::ptr::null_mut(), || {
        let args = unsafe { borrowed_args(args, count) };
        let (global, name) = unsafe { (global.as_str()?, name.as_str()?) };
        instance
            .with(|i| i.invoke_global(global, name, &args))
            .map(into_raw)
            .map_err(|e| e.to_string())
    })
}

/// Receives borrowed arguments and stores an owned return value in `result`, or leaves it null for void.
pub type CallbackFn = unsafe extern "C" fn(
    user_data: *mut c_void,
    args: *const *const Value,
    count: usize,
    result: *mut *mut Value,
);

pub type DropUserDataFn = unsafe extern "C" fn(user_data: *mut c_void);

/// A managed delegate; dropping it releases `user_data` through `drop_user_data`.
struct ManagedCallback {
    callback: CallbackFn,
    user_data: *mut c_void,
    drop_user_data: DropUserDataFn,
}

impl ManagedCallback {
    fn call(&self, args: &[Value]) -> Value {
        let args: Vec<*const Value> = args.iter().map(|v| v as *const Value).collect();
        let mut result = std::ptr::null_mut();
        unsafe { (self.callback)(self.user_data, args.as_ptr(), args.len(), &mut result) };
        if result.is_null() { Value::Void } else { *unsafe { Box::from_raw(result) } }
    }
}

impl Drop for ManagedCallback {
    fn drop(&mut self) {
        unsafe { (self.drop_user_data)(self.user_data) }
    }
}

/// Takes ownership of `user_data`, also on failure.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_component_instance_set_callback(
    instance: &DotnetInstance,
    name: Utf8Str,
    callback: CallbackFn,
    user_data: *mut c_void,
    drop_user_data: DropUserDataFn,
) -> bool {
    let callback = ManagedCallback { callback, user_data, drop_user_data };
    guard_bool(|| instance.set_callback(None, unsafe { name.as_str() }?, callback))
}

/// Takes ownership of `user_data`, also on failure.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_component_instance_set_global_callback(
    instance: &DotnetInstance,
    global: Utf8Str,
    name: Utf8Str,
    callback: CallbackFn,
    user_data: *mut c_void,
    drop_user_data: DropUserDataFn,
) -> bool {
    let callback = ManagedCallback { callback, user_data, drop_user_data };
    guard_bool(|| {
        let (global, name) = unsafe { (global.as_str()?, name.as_str()?) };
        instance.set_callback(Some(global), name, callback)
    })
}

/// Compiles the component `component` of the file `path` and creates it,
/// then reloads it whenever the file or the files it imports change, while the event loop runs.
/// An empty `style` selects the default one.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_live_component_new(
    path: Utf8Str,
    component: Utf8Str,
    style: Utf8Str,
    include_paths: *const Utf8Str,
    include_count: usize,
) -> *mut DotnetInstance {
    guard(std::ptr::null_mut(), || {
        #[cfg(feature = "live-preview")]
        {
            let path = std::path::PathBuf::from(unsafe { path.as_str() }?);
            let component = unsafe { component.as_str() }?.to_string();
            let style = Some(unsafe { style.as_str() }?.to_string()).filter(|s| !s.is_empty());
            let include_paths = unsafe { crate::utf8_slice(include_paths, include_count) }?
                .into_iter()
                .map(std::path::PathBuf::from)
                .collect::<Vec<_>>();
            // The compiler runs on a thread of its own, which builds it from these.
            let compiler_factory = move || {
                let mut compiler = slint_interpreter::Compiler::default();
                compiler.set_include_paths(include_paths);
                if let Some(style) = style {
                    compiler.set_style(style);
                }
                compiler
            };
            LiveReloadingComponent::new(compiler_factory, path, Some(component))
                .map(|live| into_raw(DotnetInstance::Live(live)))
                .map_err(|e| e.to_string())
        }
        #[cfg(not(feature = "live-preview"))]
        {
            let _ = (path, component, style, include_paths, include_count);
            Err("This native library was built without the live-preview feature".into())
        }
    })
}

/// Calls `call` after each reload of a live component. Takes ownership of `user_data`, also on failure.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_live_component_set_reload_hook(
    instance: &DotnetInstance,
    call: unsafe extern "C" fn(*mut c_void),
    user_data: *mut c_void,
    drop_user_data: unsafe extern "C" fn(*mut c_void),
) -> bool {
    let action = ManagedAction { call, user_data, drop_user_data };
    guard_bool(|| match instance {
        #[cfg(feature = "live-preview")]
        DotnetInstance::Live(live) => {
            live.borrow_mut().set_post_reload_hook(move |_| action.call());
            Ok(())
        }
        _ => {
            drop(action);
            Err("The component doesn't reload".into())
        }
    })
}
