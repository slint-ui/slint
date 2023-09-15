// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

use std::rc::{Rc, Weak};

use i_slint_compiler::langtype::Type;
use i_slint_core::model::Model;
use napi::{bindgen_prelude::Object, Env, JsFunction, JsNumber, JsUnknown, NapiRaw};
use slint_interpreter::Value;

use crate::{to_js_unknown, to_value, RefCountedReference};

pub struct JsModel {
    model: RefCountedReference,
    env: Env,
    notify: i_slint_core::model::ModelNotify,
    data_type: Type,
}

impl JsModel {
    pub fn new<T: NapiRaw>(env: Env, model: T, data_type: Type) -> napi::Result<Rc<Self>> {
        // let model = RefCountedReference::new(&env, model)?;
        let js_model = Rc::new(Self {
            notify: Default::default(),
            env,
            model: RefCountedReference::new(&env, model)?,
            data_type,
        });

        let notfy = JsSlintModelNotify { model: Rc::downgrade(&js_model) };


        Ok(js_model)
    }

    pub fn model(&self) -> &RefCountedReference {
        &self.model
    }
}

impl Model for JsModel {
    type Data = slint_interpreter::Value;

    fn row_count(&self) -> usize {
        let model: Object = self.model.get().unwrap();
        model
            .get::<&str, JsFunction>("rowCount")
            .ok()
            .and_then(|callback| {
                callback.and_then(|callback| {
                    callback.call::<JsUnknown>(Some(&model), vec![].as_ref()).ok()
                })
            })
            .and_then(|res| res.coerce_to_number().ok())
            .map(|num| num.get_uint32().ok().map_or(0, |count| count as usize))
            .unwrap_or_default()
    }

    fn row_data(&self, row: usize) -> Option<Self::Data> {
        let model: Object = self.model.get().unwrap();
        model
            .get::<&str, JsFunction>("rowData")
            .ok()
            .and_then(|callback| {
                callback.and_then(|callback| {
                    callback
                        .call::<JsNumber>(
                            Some(&model),
                            &[self.env.create_double(row as f64).unwrap()],
                        )
                        .ok()
                })
            })
            .and_then(|res| to_value(&self.env, res, self.data_type.clone()).ok())
    }

    fn model_tracker(&self) -> &dyn i_slint_core::model::ModelTracker {
        &self.notify
    }

    fn set_row_data(&self, row: usize, data: Self::Data) {
        let model: Object = self.model.get().unwrap();
        model.get::<&str, JsFunction>("setRowData").ok().and_then(|callback| {
            callback.and_then(|callback| {
                callback
                    .call::<JsUnknown>(
                        Some(&model),
                        &[
                            to_js_unknown(&self.env, &Value::Number(row as f64)).unwrap(),
                            to_js_unknown(&self.env, &data).unwrap(),
                        ],
                    )
                    .ok()
            })
        });
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}

#[napi(js_name = "SlintModelNotify")]
pub struct JsSlintModelNotify {
    model: Weak<JsModel>,
}

#[napi]
impl JsSlintModelNotify {
    #[napi(constructor)]
    pub fn new() -> Self {
        Self { model: Weak::default() }
    }

    #[napi]
    pub fn row_data_changed(&self, row: u32) {}

    #[napi]
    pub fn row_added(&self, row: u32, count: u32) {}

    #[napi]
    pub fn row_removed(&self, row: u32, count: u32) {}

    #[napi]
    pub fn reset(&self) {}
}
