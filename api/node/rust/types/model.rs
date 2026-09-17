// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::rc::Rc;

use core::result::Result as CoreResult;
use i_slint_compiler::langtype::Type;
use i_slint_core::model::{Model, ModelError, ModelNotify, ModelRc};
use napi::bindgen_prelude::*;
use napi::{Env, JsValue, Result, ValueType};

use crate::weak_ref::WeakValueRef;
use crate::{JsAnchorOwner, to_js_unknown, to_value};

#[napi]
#[derive(Clone, Default)]
pub struct SharedModelNotify(Rc<ModelNotify>);

impl core::ops::Deref for SharedModelNotify {
    type Target = Rc<ModelNotify>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Take the pending JavaScript exception off the environment, so that later napi
/// calls succeed and the exception doesn't surface when the runtime returns to JS.
fn take_exception(env: &Env) {
    let mut exception = std::ptr::null_mut();
    // SAFETY: plain FFI call, napi-rs has no safe wrapper for it. The env is the valid
    // environment of the calling thread and the out-pointer is a valid local; the
    // exception value written to it stays owned by the VM and is discarded.
    unsafe { napi::sys::napi_get_and_clear_last_exception(env.raw(), &mut exception) };
}

/// An unsupported ModelError naming the JavaScript class of the model, falling
/// back to the name of the JsModel wrapper. Clears the exception thrown to signal
/// the rejection, so the class-name lookup and later napi calls work.
fn unsupported_error(js_model: &JsModel, model: &Object) -> ModelError {
    take_exception(&js_model.env);
    let class_name = || -> Option<String> {
        // The constructor is a JS function; fetch it untyped and coerce, a typed
        // `get_named_property::<Object>` rejects functions.
        let constructor: Unknown = model.get_named_property("constructor").ok()?;
        let constructor = constructor.coerce_to_object().ok()?;
        let name: String = constructor.get_named_property("name").ok()?;
        (!name.is_empty()).then_some(name)
    };
    match class_name() {
        Some(name) => ModelError::unsupported_by_name(name, i_slint_core::InternalToken),
        None => ModelError::unsupported(js_model),
    }
}

pub(crate) fn js_into_rust_model(
    env: &Env,
    maybe_js_impl: &Object,
    row_data_type: &Type,
    anchor_owner: &JsAnchorOwner,
) -> Result<ModelRc<slint_interpreter::Value>> {
    let shared_model_notify: ExternalRef<SharedModelNotify> =
        maybe_js_impl.get_named_property("modelNotify")?;
    let shared_model_notify: SharedModelNotify = (*shared_model_notify).clone();

    let anchor_id = anchor_owner.next_anchor_id();
    let prop_key = format!("__slint_model#{anchor_id}");

    // Register the model as a JS property on the owner so V8 keeps it alive
    // without creating an independent GC root.
    if let Some(mut obj) = crate::weak_ref::weak_ref_get_object::<crate::JsComponentInstance>(
        &anchor_owner.owner_weak,
        *env,
    ) {
        crate::set_hidden_property(&mut obj, &prop_key, maybe_js_impl)?;
    }

    Ok(Rc::new(JsModel {
        shared_model_notify,
        env: *env,
        js_impl: WeakValueRef::new(env, maybe_js_impl)?,
        row_data_type: row_data_type.clone(),
        prop_key,
        owner: anchor_owner.clone(),
    })
    .into())
}

pub(crate) fn rust_into_js_model<'a>(
    env: &'a Env,
    model: &ModelRc<slint_interpreter::Value>,
) -> Option<Result<Unknown<'a>>> {
    model.as_any().downcast_ref::<JsModel>().map(|rust_model| {
        rust_model
            .js_impl
            .get_unknown()
            .ok_or_else(|| napi::Error::from_reason("JS model has been garbage collected"))?
            .into_unknown(env)
    })
}

struct JsModel {
    shared_model_notify: SharedModelNotify,
    env: Env,
    js_impl: WeakValueRef,
    row_data_type: Type,
    prop_key: String,
    owner: JsAnchorOwner,
}

impl Drop for JsModel {
    fn drop(&mut self) {
        // Pure Rust check (no NAPI calls).
        // Returns None once the owning JsComponentInstance's anchor_seq
        // Rc has been dropped,
        // which happens before `inner` (field declaration order).
        if self.owner.seq.upgrade().is_none() {
            return;
        }
        if let Some(mut obj) = crate::weak_ref::weak_ref_get_object::<crate::JsComponentInstance>(
            &self.owner.owner_weak,
            self.env,
        ) {
            let _ = obj.delete_named_property(&self.prop_key);
        }
    }
}

#[napi]
pub fn js_model_notify_new() -> Result<External<SharedModelNotify>> {
    Ok(External::new(Default::default()))
}

#[napi]
pub fn js_model_notify_row_data_changed(notify: ExternalRef<SharedModelNotify>, row: u32) {
    notify.row_changed(row as usize);
}

#[napi]
pub fn js_model_notify_row_added(notify: ExternalRef<SharedModelNotify>, row: u32, count: u32) {
    notify.row_added(row as usize, count as usize);
}

#[napi]
pub fn js_model_notify_row_removed(notify: ExternalRef<SharedModelNotify>, row: u32, count: u32) {
    notify.row_removed(row as usize, count as usize);
}

#[napi]
pub fn js_model_notify_reset(notify: ExternalRef<SharedModelNotify>) {
    notify.reset();
}

impl JsModel {
    /// Report a glue-level error to `console.error`. Clears any pending JS exception
    /// first, so the console call works and the exception doesn't surface in JS.
    fn report_error(&self, message: core::fmt::Arguments<'_>) {
        take_exception(&self.env);
        crate::print_to_console(self.env, "error", message);
    }
}

impl Model for JsModel {
    type Data = slint_interpreter::Value;

    fn row_count(&self) -> usize {
        let Some(model_unknown) = self.js_impl.get_unknown() else {
            self.report_error(format_args!(
                "Node.js: JavaScript Model<T>'s rowCount threw an exception"
            ));
            return 0;
        };

        let Ok(model) = model_unknown.coerce_to_object() else {
            self.report_error(format_args!("Node.js: JavaScript Model<T> is not an object"));
            return 0;
        };

        let row_count_fn: Function<(), Unknown> = match model.get_named_property("rowCount") {
            Ok(f) => f,
            Err(_) => {
                self.report_error(format_args!(
                    "Node.js: JavaScript Model<T> implementation is missing rowCount property"
                ));
                return 0;
            }
        };

        let Ok(row_count_result) = row_count_fn.apply(model, ()) else {
            self.report_error(format_args!(
                "Node.js: JavaScript Model<T>'s rowCount implementation call failed"
            ));
            return 0;
        };

        let Ok(row_count_number) = row_count_result.coerce_to_number() else {
            self.report_error(format_args!(
                "Node.js: JavaScript Model<T>'s rowCount function returned a value that cannot be coerced to a number"
            ));
            return 0;
        };

        let Ok(row_count) = row_count_number.get_uint32() else {
            self.report_error(format_args!(
                "Node.js: JavaScript Model<T>'s rowCount function returned a number that cannot be mapped to a uint32"
            ));
            return 0;
        };

        row_count as usize
    }

    fn row_data(&self, row: usize) -> Option<Self::Data> {
        let Some(model_unknown) = self.js_impl.get_unknown() else {
            self.report_error(format_args!(
                "Node.js: JavaScript Model<T>'s rowData threw an exception"
            ));
            return None;
        };

        let Ok(model) = model_unknown.coerce_to_object() else {
            self.report_error(format_args!("Node.js: JavaScript Model<T> is not an object"));
            return None;
        };

        let row_data_fn: Function<f64, Unknown> = match model.get_named_property("rowData") {
            Ok(f) => f,
            Err(_) => {
                self.report_error(format_args!(
                    "Node.js: JavaScript Model<T> implementation is missing rowData property"
                ));
                return None;
            }
        };

        let Ok(row_data) = row_data_fn.apply(model, row as f64) else {
            self.report_error(format_args!(
                "Node.js: JavaScript Model<T>'s rowData function threw an exception"
            ));
            return None;
        };

        if row_data.get_type().unwrap() == ValueType::Undefined {
            debug_assert!(row >= self.row_count());
            None
        } else {
            let Ok(js_value) = to_value(&self.env, row_data, &self.row_data_type, &self.owner)
            else {
                self.report_error(format_args!(
                    "Node.js: JavaScript Model<T>'s rowData function returned data type that cannot be represented in Rust"
                ));
                return None;
            };
            Some(js_value)
        }
    }

    fn set_row_data(&self, row: usize, data: Self::Data) {
        let Some(model_unknown) = self.js_impl.get_unknown() else {
            self.report_error(format_args!(
                "Node.js: JavaScript Model<T>'s setRowData threw an exception"
            ));
            return;
        };

        let Ok(model) = model_unknown.coerce_to_object() else {
            self.report_error(format_args!("Node.js: JavaScript Model<T> is not an object"));
            return;
        };

        let set_row_data_fn: Function<FnArgs<(f64, Unknown<'_>)>, Unknown> =
            match model.get_named_property("setRowData") {
                Ok(f) => f,
                Err(_) => {
                    self.report_error(format_args!(
                        "Node.js: JavaScript Model<T> implementation is missing setRowData property"
                    ));
                    return;
                }
            };

        let Ok(js_data) = to_js_unknown(&self.env, &data) else {
            self.report_error(format_args!(
                "Node.js: Model<T>'s set_row_data called by Rust with data type that can't be represented in JavaScript"
            ));
            return;
        };

        // A rejected modification is reported by throwing.
        if set_row_data_fn.apply(model, FnArgs::from((row as f64, js_data))).is_err() {
            let error = unsupported_error(self, &model);
            i_slint_core::debug_log!("setRowData(): {error}");
        }
    }

    fn push_row(&self, data: Self::Data) -> CoreResult<(), ModelError> {
        let Some(model_unknown) = self.js_impl.get_unknown() else {
            self.report_error(format_args!(
                "Node.js: JavaScript Model<T>'s pushRow threw an exception"
            ));
            return Err(ModelError::unsupported(self));
        };

        let Ok(model) = model_unknown.coerce_to_object() else {
            self.report_error(format_args!("Node.js: JavaScript Model<T> is not an object"));
            return Err(ModelError::unsupported(self));
        };

        let push_row_fn: Function<Unknown<'_>, Unknown> = match model.get_named_property("pushRow")
        {
            Ok(f) => f,
            Err(e) => {
                self.report_error(format_args!(
                    "Node.js: JavaScript Model<T> implementation is missing pushRow property: {e}"
                ));
                return Err(ModelError::unsupported(self));
            }
        };

        let Ok(js_data) = to_js_unknown(&self.env, &data) else {
            self.report_error(format_args!(
                "Node.js: Model<T>'s push_row called by Rust with data type that can't be represented in JavaScript"
            ));
            return Err(ModelError::unsupported(self));
        };

        // A rejected modification is reported by throwing.
        match push_row_fn.apply(model, js_data) {
            Ok(_) => Ok(()),
            Err(_) => Err(unsupported_error(self, &model)),
        }
    }

    fn remove_row(&self, row: usize) -> CoreResult<(), ModelError> {
        let row_count = self.row_count();
        if row >= row_count {
            return Err(ModelError::out_of_bounds(row_count));
        }

        let Some(model_unknown) = self.js_impl.get_unknown() else {
            self.report_error(format_args!(
                "Node.js: JavaScript Model<T>'s removeRow threw an exception"
            ));
            return Err(ModelError::unsupported(self));
        };

        let Ok(model) = model_unknown.coerce_to_object() else {
            self.report_error(format_args!("Node.js: JavaScript Model<T> is not an object"));
            return Err(ModelError::unsupported(self));
        };

        let remove_row_fn: Function<f64, Unknown> = match model.get_named_property("removeRow") {
            Ok(f) => f,
            Err(e) => {
                self.report_error(format_args!(
                    "Node.js: JavaScript Model<T> implementation is missing removeRow property: {e}"
                ));
                return Err(ModelError::unsupported(self));
            }
        };

        // A rejected modification is reported by throwing.
        match remove_row_fn.apply(model, row as f64) {
            Ok(_) => Ok(()),
            Err(_) => Err(unsupported_error(self, &model)),
        }
    }

    fn insert_row(&self, row: usize, data: Self::Data) -> CoreResult<(), ModelError> {
        let row_count = self.row_count();
        if row > row_count {
            return Err(ModelError::out_of_bounds(row_count));
        }

        let Some(model_unknown) = self.js_impl.get_unknown() else {
            self.report_error(format_args!(
                "Node.js: JavaScript Model<T>'s insertRow threw an exception"
            ));
            return Err(ModelError::unsupported(self));
        };

        let Ok(model) = model_unknown.coerce_to_object() else {
            self.report_error(format_args!("Node.js: JavaScript Model<T> is not an object"));
            return Err(ModelError::unsupported(self));
        };

        let insert_row_fn: Function<FnArgs<(f64, Unknown<'_>)>, Unknown> =
            match model.get_named_property("insertRow") {
                Ok(f) => f,
                Err(e) => {
                    self.report_error(format_args!(
                    "Node.js: JavaScript Model<T> implementation is missing insertRow property: {e}"
                ));
                    return Err(ModelError::unsupported(self));
                }
            };

        let Ok(js_data) = to_js_unknown(&self.env, &data) else {
            self.report_error(format_args!(
                "Node.js: Model<T>'s insert_row called by Rust with data type that can't be represented in JavaScript"
            ));
            return Err(ModelError::unsupported(self));
        };

        // A rejected modification is reported by throwing.
        match insert_row_fn.apply(model, FnArgs::from((row as f64, js_data))) {
            Ok(_) => Ok(()),
            Err(_) => Err(unsupported_error(self, &model)),
        }
    }

    fn model_tracker(&self) -> &dyn i_slint_core::model::ModelTracker {
        &**self.shared_model_notify
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}

#[napi]
pub struct ReadOnlyRustModel(ModelRc<slint_interpreter::Value>);

impl From<ModelRc<slint_interpreter::Value>> for ReadOnlyRustModel {
    fn from(model: ModelRc<slint_interpreter::Value>) -> Self {
        Self(model)
    }
}

// Implement minimal Model<T> interface
#[napi]
impl ReadOnlyRustModel {
    #[napi]
    pub fn row_count(&self) -> u32 {
        self.0.row_count() as u32
    }

    #[napi]
    pub fn row_data<'a>(&self, env: &'a Env, row: u32) -> Result<Unknown<'a>> {
        let Some(data) = self.0.row_data(row as usize) else {
            return ().into_unknown(env);
        };
        crate::to_js_unknown(env, &data)
    }

    #[napi]
    pub fn set_row_data(&self, env: &Env, _row: u32, _data: Unknown<'_>) {
        crate::console_err!(
            *env,
            "setRowData called on a model which does not re-implement this method. This happens when trying to modify a read-only model"
        )
    }

    pub fn into_js<'a>(self, env: &'a Env) -> Result<Unknown<'a>> {
        let model = self.0.clone();

        let mut obj = self.into_instance(env)?.as_object(env);

        // Implement Iterator protocol by hand until it's stable in napi-rs
        let global = env.get_global()?;
        let symbol_function: Unknown = global.get_named_property("Symbol")?;
        let symbol_obj = symbol_function.coerce_to_object()?;
        let iterator_symbol: napi::JsSymbol = symbol_obj.get_named_property("iterator")?;

        obj.set_property(
            iterator_symbol,
            env.create_function_from_closure::<(), ModelIterator, _>(
                "rust model iterator",
                move |ctx| Ok(ModelIterator { model: model.clone(), row: 0, env: *ctx.env }),
            )?,
        )?;

        obj.into_unknown(env)
    }
}

#[napi]
pub struct ModelIterator {
    model: ModelRc<slint_interpreter::Value>,
    row: usize,
    env: Env,
}

#[napi]
impl ModelIterator {
    // Implements the JS iterator protocol — name must be `next`.
    #[allow(clippy::should_implement_trait)]
    #[napi]
    pub fn next(&mut self) -> Result<Unknown<'_>> {
        let mut result = Object::new(&self.env)?;
        if self.row >= self.model.row_count() {
            result.set_named_property("done", true)?;
        } else {
            let row = self.row;
            self.row += 1;
            result.set_named_property(
                "value",
                self.model.row_data(row).and_then(|value| to_js_unknown(&self.env, &value).ok()),
            )?
        }
        result.into_unknown(&self.env)
    }
}
