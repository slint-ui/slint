// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::ffi::c_void;
use std::rc::{Rc, Weak};

use i_slint_core::model::{Model, ModelError, ModelNotify, ModelRc, ModelTracker};
use slint_interpreter::Value;

use crate::{Utf8Str, guard, guard_bool, into_raw, release};

/// The functions through which Rust calls a `Slint.Model<T>`.
/// Each receives the model's `GCHandle` as `user_data`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ModelVTable {
    row_count: unsafe extern "C" fn(user_data: *mut c_void) -> usize,
    /// Stores an owned value in `result`, or leaves it null if there's no row `row`.
    row_data: unsafe extern "C" fn(user_data: *mut c_void, row: usize, result: *mut *mut Value),
    set_row_data: unsafe extern "C" fn(user_data: *mut c_void, row: usize, value: *const Value),
    /// Returns false if the model doesn't support inserting rows.
    insert_row:
        unsafe extern "C" fn(user_data: *mut c_void, row: usize, value: *const Value) -> bool,
    /// Returns false if the model doesn't support removing rows.
    remove_row: unsafe extern "C" fn(user_data: *mut c_void, row: usize) -> bool,
    drop_user_data: unsafe extern "C" fn(user_data: *mut c_void),
}

/// A model implemented in .NET.
/// It keeps the .NET object alive while Slint uses it;
/// the .NET object only holds a `Weak` to it, so that there's no cycle across the two heaps.
pub struct ManagedModel {
    vtable: ModelVTable,
    user_data: *mut c_void,
    notify: ModelNotify,
    /// The name of the .NET type, for the error of a modification that it doesn't support.
    type_name: String,
}

impl ManagedModel {
    fn unsupported(&self) -> ModelError {
        ModelError::unsupported_by_name(self.type_name.clone(), i_slint_core::InternalToken)
    }
}

impl Model for ManagedModel {
    type Data = Value;

    fn row_count(&self) -> usize {
        unsafe { (self.vtable.row_count)(self.user_data) }
    }

    fn row_data(&self, row: usize) -> Option<Value> {
        let mut result = std::ptr::null_mut();
        unsafe { (self.vtable.row_data)(self.user_data, row, &mut result) };
        (!result.is_null()).then(|| *unsafe { Box::from_raw(result) })
    }

    fn set_row_data(&self, row: usize, data: Value) {
        unsafe { (self.vtable.set_row_data)(self.user_data, row, &data) }
    }

    fn insert_row(&self, row: usize, data: Value) -> Result<(), ModelError> {
        let row_count = self.row_count();
        if row > row_count {
            return Err(ModelError::out_of_bounds(row_count));
        }
        if unsafe { (self.vtable.insert_row)(self.user_data, row, &data) } {
            Ok(())
        } else {
            Err(self.unsupported())
        }
    }

    fn remove_row(&self, row: usize) -> Result<(), ModelError> {
        let row_count = self.row_count();
        if row >= row_count {
            return Err(ModelError::out_of_bounds(row_count));
        }
        if unsafe { (self.vtable.remove_row)(self.user_data, row) } {
            Ok(())
        } else {
            Err(self.unsupported())
        }
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        &self.notify
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}

impl Drop for ManagedModel {
    fn drop(&mut self) {
        unsafe { (self.vtable.drop_user_data)(self.user_data) }
    }
}

/// Wraps a .NET model in a new value, taking ownership of `user_data`.
/// The returned `Weak` handle notifies Slint of changes; free it with `slint_dotnet_model_weak_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_model_new(
    vtable: ModelVTable,
    user_data: *mut c_void,
    type_name: Utf8Str,
    weak: &mut *mut Weak<ManagedModel>,
) -> *mut Value {
    let type_name = unsafe { type_name.as_str() }.unwrap_or_default().to_owned();
    let model = Rc::new(ManagedModel { vtable, user_data, notify: Default::default(), type_name });
    guard(std::ptr::null_mut(), || {
        *weak = into_raw(Rc::downgrade(&model));
        Ok(into_raw(Value::Model(ModelRc::from(model))))
    })
}

/// Returns a value with the model, or null if Slint no longer uses it.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_model_weak_upgrade(weak: &Weak<ManagedModel>) -> *mut Value {
    guard(std::ptr::null_mut(), || {
        Ok(weak
            .upgrade()
            .map_or(std::ptr::null_mut(), |m| into_raw(Value::Model(ModelRc::from(m)))))
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_model_weak_free(weak: *mut Weak<ManagedModel>) {
    unsafe { release(weak) }
}

fn notify(weak: &Weak<ManagedModel>, f: impl FnOnce(&ModelNotify)) -> bool {
    guard_bool(|| {
        if let Some(model) = weak.upgrade() {
            f(&model.notify);
        }
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_model_notify_row_changed(
    weak: &Weak<ManagedModel>,
    row: usize,
) -> bool {
    notify(weak, |n| n.row_changed(row))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_model_notify_row_added(
    weak: &Weak<ManagedModel>,
    index: usize,
    count: usize,
) -> bool {
    notify(weak, |n| n.row_added(index, count))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_model_notify_row_removed(
    weak: &Weak<ManagedModel>,
    index: usize,
    count: usize,
) -> bool {
    notify(weak, |n| n.row_removed(index, count))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_model_notify_reset(weak: &Weak<ManagedModel>) -> bool {
    notify(weak, |n| n.reset())
}

/// Returns the `user_data` of a model implemented in .NET, or null for any other value.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_value_managed_model(value: &Value) -> *mut c_void {
    match value {
        Value::Model(m) => {
            m.as_any().downcast_ref::<ManagedModel>().map_or(std::ptr::null_mut(), |m| m.user_data)
        }
        _ => std::ptr::null_mut(),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_value_model_set_row_data(
    value: &Value,
    row: usize,
    data: &Value,
) -> bool {
    guard_bool(|| match value {
        Value::Model(m) if row < m.row_count() => {
            m.set_row_data(row, data.clone());
            Ok(())
        }
        Value::Model(_) => Err(format!("row {row} is out of range")),
        _ => Err("value isn't a model".into()),
    })
}
