// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

use std::rc::Rc;

use i_slint_compiler::langtype::Type;
use i_slint_core::model::{Model, ModelNotify, ModelRc};
use napi::bindgen_prelude::*;
use napi::{Env, JsExternal, JsFunction, JsNumber, JsObject, JsUnknown, Result, ValueType};

use crate::{to_js_unknown, to_value, RefCountedReference};

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
    maybe_js_impl: &JsObject,
    row_data_type: &Type,
) -> Result<ModelRc<slint_interpreter::Value>> {
    let shared_model_notify = maybe_js_impl
        .get("modelNotify")
        .and_then(|prop| {
            prop.ok_or_else(|| {
                napi::Error::from_reason(
                    "Could not convert value to slint model: missing modelNotify property",
                )
            })
        })
        .and_then(|shared_model_notify: JsExternal| {
            env.get_value_external::<SharedModelNotify>(&shared_model_notify).cloned()
        })?;
    Ok(Rc::new(JsModel {
        shared_model_notify,
        env: env.clone(),
        js_impl: RefCountedReference::new(env, maybe_js_impl)?,
        row_data_type: row_data_type.clone(),
    })
    .into())
}

pub(crate) fn rust_into_js_model(
    model: &ModelRc<slint_interpreter::Value>,
) -> Option<Result<JsUnknown>> {
    model.as_any().downcast_ref::<JsModel>().map(|rust_model| rust_model.js_impl.get())
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
pub fn js_model_notify_row_data_changed(notify: External<SharedModelNotify>, row: u32) {
    notify.row_changed(row as usize);
}

#[napi]
pub fn js_model_notify_row_added(notify: External<SharedModelNotify>, row: u32, count: u32) {
    notify.row_added(row as usize, count as usize);
}

#[napi]
pub fn js_model_notify_row_removed(notify: External<SharedModelNotify>, row: u32, count: u32) {
    notify.row_removed(row as usize, count as usize);
}

#[napi]
pub fn js_model_notify_reset(notify: External<SharedModelNotify>) {
    notify.reset();
}

impl Model for JsModel {
    type Data = slint_interpreter::Value;

    fn row_count(&self) -> usize {
        let model: Object = self.js_impl.get().unwrap();
        model
            .get::<&str, JsFunction>("rowCount")
            .ok()
            .and_then(|callback| {
                callback.and_then(|callback| callback.call_without_args(Some(&model)).ok())
            })
            .and_then(|res| res.coerce_to_number().ok())
            .map(|num| num.get_uint32().ok().map_or(0, |count| count as usize))
            .unwrap_or_default()
    }

    fn row_data(&self, row: usize) -> Option<Self::Data> {
        let model: Object = self.js_impl.get().unwrap();
        let row_data_fn = model
            .get::<&str, JsFunction>("rowData")
            .expect("Node.js: JavaScript Model<T> implementation is missing rowData property")
            .expect("Node.js: Model<T> implementation's rowData property is not a function");
        let row_data = row_data_fn
            .call::<JsNumber>(Some(&model), &[self.env.create_double(row as f64).unwrap()])
            .expect("Node.js: JavaScript Model<T>'s rowData function threw an exception");
        if row_data.get_type().unwrap() == ValueType::Undefined {
            debug_assert!(row >= self.row_count());
            None
        } else {
            Some(to_value(&self.env, row_data, &self.row_data_type).expect("Node.js: JavaScript Model<T>'s rowData function returned data type that cannot be represented in Rust"))
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
    pub fn row_count(&self, env: Env) -> Result<JsNumber> {
        env.create_uint32(self.0.row_count() as u32)
    }

    #[napi]
    pub fn row_data(&self, env: Env, row: u32) -> Result<JsUnknown> {
        let Some(data) = self.0.row_data(row as usize) else {
            return env.get_undefined().map(|v| v.into_unknown());
        };
        to_js_unknown(&env, &data)
    }

    #[napi]
    pub fn set_row_data(&self, _env: Env, _row: u32, _data: JsUnknown) {
        eprintln!("setRowData called on a model which does not re-implement this method. This happens when trying to modify a read-only model")
    }
}
