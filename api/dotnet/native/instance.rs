// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::ffi::c_void;

use i_slint_core::properties::ChangeTracker;
use slint_interpreter::{ComponentHandle, ComponentInstance, Value};

use crate::timer::ManagedAction;
use crate::{Utf8Str, guard, guard_bool, into_raw, release};

/// The `ComponentInstance` of .NET.
pub struct DotnetInstance(pub ComponentInstance);

impl DotnetInstance {
    /// Calls `f` with the component.
    pub(crate) fn with<R>(&self, f: impl FnOnce(&ComponentInstance) -> R) -> R {
        f(&self.0)
    }

    fn set_property(&self, global: Option<&str>, name: &str, value: &Value) -> Result<(), String> {
        let value = value.clone();
        match global {
            Some(global) => self.0.set_global_property(global, name, value),
            None => self.0.set_property(name, value),
        }
        .map_err(|e| e.to_string())
    }

    fn set_callback(
        &self,
        global: Option<&str>,
        name: &str,
        callback: ManagedCallback,
    ) -> Result<(), String> {
        match global {
            Some(global) => {
                self.0.set_global_callback(global, name, move |args| callback.call(args))
            }
            None => self.0.set_callback(name, move |args| callback.call(args)),
        }
        .map_err(|e| e.to_string())
    }

    fn run(&self) -> Result<(), String> {
        self.0.run().map_err(|e| e.to_string())
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

/// False for a component that inherits `SystemTrayIcon`, which has no window.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_component_instance_is_window(
    instance: &DotnetInstance,
) -> bool {
    guard(false, || Ok(instance.with(|i| i.definition().is_window())))
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

/// Where a tracker reads its property.
struct TrackedInstance(slint_interpreter::Weak<ComponentInstance>);

impl TrackedInstance {
    fn read(&self, global: Option<&str>, name: &str) -> Value {
        self.0
            .upgrade()
            .and_then(|instance| match global {
                Some(global) => instance.get_global_property(global, name).ok(),
                None => instance.get_property(name).ok(),
            })
            .unwrap_or_default()
    }
}

/// Calls `changed` with `user_data` after the property changes, the property of `global` unless it's empty.
/// Slint calls it while it runs its change handlers, once per frame.
/// Takes ownership of `user_data`, also on failure.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_property_tracker_new(
    instance: &DotnetInstance,
    global: Utf8Str,
    name: Utf8Str,
    changed: unsafe extern "C" fn(user_data: *mut c_void),
    user_data: *mut c_void,
    drop_user_data: unsafe extern "C" fn(user_data: *mut c_void),
) -> *mut ChangeTracker {
    let action = ManagedAction { call: changed, user_data, drop_user_data };
    guard(std::ptr::null_mut(), move || {
        let (global, name) = unsafe { (global.as_str()?, name.as_str()?) };
        let global = (!global.is_empty()).then(|| global.to_owned());
        let name = name.to_owned();
        let tracked = TrackedInstance(instance.0.as_weak());
        instance
            .with(|i| match &global {
                Some(global) => i.get_global_property(global, &name),
                None => i.get_property(&name),
            })
            .map_err(|e| e.to_string())?;
        let tracker = ChangeTracker::default();
        tracker.init(
            (tracked, global, name),
            |(tracked, global, name)| tracked.read(global.as_deref(), name),
            move |_, _| action.call(),
        );
        Ok(into_raw(tracker))
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_property_tracker_free(tracker: *mut ChangeTracker) {
    unsafe { release(tracker) }
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
