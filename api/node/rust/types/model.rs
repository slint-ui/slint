// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::rc::Rc;

use i_slint_compiler::langtype::Type;
use i_slint_core::model::{Model, ModelNotify, ModelRc};
use napi::bindgen_prelude::*;
use napi::{Env, JsValue, Result, ValueType};

use crate::{RefCountedReference, to_js_unknown, to_value};

#[napi]
#[derive(Clone, Default)]
pub struct SharedModelNotify(Rc<ModelNotify>);

impl core::ops::Deref for SharedModelNotify {
    type Target = Rc<ModelNotify>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub(crate) fn js_into_rust_model(
    env: &Env,
    maybe_js_impl: &Object,
    row_data_type: &Type,
) -> Result<ModelRc<slint_interpreter::Value>> {
    let shared_model_notify: ExternalRef<SharedModelNotify> =
        maybe_js_impl.get_named_property("modelNotify")?;
    let shared_model_notify: SharedModelNotify = (*shared_model_notify).clone();
    Ok(Rc::new(JsModel {
        shared_model_notify,
        env: *env,
        js_impl: RefCountedReference::new(env, maybe_js_impl)?,
        row_data_type: row_data_type.clone(),
    })
    .into())
}

pub(crate) fn rust_into_js_model<'a>(
    env: &'a Env,
    model: &ModelRc<slint_interpreter::Value>,
) -> Option<Result<Unknown<'a>>> {
    model
        .as_any()
        .downcast_ref::<JsModel>()
        .map(|rust_model| rust_model.js_impl.get_unknown()?.into_unknown(env))
}

struct JsModel {
    shared_model_notify: SharedModelNotify,
    env: Env,
    js_impl: RefCountedReference,
    row_data_type: Type,
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

impl Model for JsModel {
    type Data = slint_interpreter::Value;

    fn row_count(&self) -> usize {
        let Ok(model_unknown) = self.js_impl.get_unknown() else {
            eprintln!("Node.js: JavaScript Model<T>'s rowCount threw an exception");
            return 0;
        };

        let Ok(model) = model_unknown.coerce_to_object() else {
            eprintln!("Node.js: JavaScript Model<T> is not an object");
            return 0;
        };

        let row_count_fn: Function<(), Unknown> = match model.get_named_property("rowCount") {
            Ok(f) => f,
            Err(_) => {
                eprintln!(
                    "Node.js: JavaScript Model<T> implementation is missing rowCount property"
                );
                return 0;
            }
        };

        let Ok(row_count_result) = row_count_fn.apply(model, ()) else {
            eprintln!("Node.js: JavaScript Model<T>'s rowCount implementation call failed");
            return 0;
        };

        let Ok(row_count_number) = row_count_result.coerce_to_number() else {
            eprintln!(
                "Node.js: JavaScript Model<T>'s rowCount function returned a value that cannot be coerced to a number"
            );
            return 0;
        };

        let Ok(row_count) = row_count_number.get_uint32() else {
            eprintln!(
                "Node.js: JavaScript Model<T>'s rowCount function returned a number that cannot be mapped to a uint32"
            );
            return 0;
        };

        row_count as usize
    }

    fn row_data(&self, row: usize) -> Option<Self::Data> {
        let Ok(model_unknown) = self.js_impl.get_unknown() else {
            eprintln!("Node.js: JavaScript Model<T>'s rowData threw an exception");
            return None;
        };

        let Ok(model) = model_unknown.coerce_to_object() else {
            eprintln!("Node.js: JavaScript Model<T> is not an object");
            return None;
        };

        let row_data_fn: Function<f64, Unknown> = match model.get_named_property("rowData") {
            Ok(f) => f,
            Err(_) => {
                eprintln!(
                    "Node.js: JavaScript Model<T> implementation is missing rowData property"
                );
                return None;
            }
        };

        let Ok(row_data) = row_data_fn.apply(model, row as f64) else {
            eprintln!("Node.js: JavaScript Model<T>'s rowData function threw an exception");
            return None;
        };

        if row_data.get_type().unwrap() == ValueType::Undefined {
            debug_assert!(row >= self.row_count());
            None
        } else {
            let Ok(js_value) = to_value(&self.env, row_data, &self.row_data_type) else {
                eprintln!(
                    "Node.js: JavaScript Model<T>'s rowData function returned data type that cannot be represented in Rust"
                );
                return None;
            };
            Some(js_value)
        }
    }

    fn set_row_data(&self, row: usize, data: Self::Data) {
        let Ok(model_unknown) = self.js_impl.get_unknown() else {
            eprintln!("Node.js: JavaScript Model<T>'s setRowData threw an exception");
            return;
        };

        let Ok(model) = model_unknown.coerce_to_object() else {
            eprintln!("Node.js: JavaScript Model<T> is not an object");
            return;
        };

        let set_row_data_fn: Function<FnArgs<(f64, Unknown<'_>)>, Unknown> =
            match model.get_named_property("setRowData") {
                Ok(f) => f,
                Err(_) => {
                    eprintln!(
                        "Node.js: JavaScript Model<T> implementation is missing setRowData property"
                    );
                    return;
                }
            };

        let Ok(js_data) = to_js_unknown(&self.env, &data) else {
            eprintln!(
                "Node.js: Model<T>'s set_row_data called by Rust with data type that can't be represented in JavaScript"
            );
            return;
        };

        if let Err(exception) = set_row_data_fn.apply(model, FnArgs::from((row as f64, js_data))) {
            eprintln!(
                "Node.js: JavaScript Model<T>'s setRowData function threw an exception: {exception}"
            );
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
    pub fn set_row_data(&self, _env: &Env, _row: u32, _data: Unknown<'_>) {
        eprintln!(
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
