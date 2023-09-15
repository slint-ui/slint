// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

use i_slint_core::model::{Model, ModelRc, VecModel};
use napi::{bindgen_prelude::{External, ToNapiValue, Object}, Env, JsObject, NapiRaw};
use napi_derive::napi;
use slint_interpreter::Value;

use crate::RefCountedReference;

pub struct JsModel {
    model: RefCountedReference,
    env: Env,
    notify: i_slint_core::model::ModelNotify,
}

impl JsModel {
    pub fn new<T: NapiRaw>(env: Env, model: T) -> napi::Result<Self> {
        Ok(Self {
            notify: Default::default(),
            env,
            model: RefCountedReference::new(&env, model)?,

        })
    }

    pub fn model(&self) -> &RefCountedReference {
        &self.model
    }
}

impl Model for JsModel {
    type Data = slint_interpreter::Value;

    fn row_count(&self) -> usize {
        0
    }

    fn row_data(&self, row: usize) -> Option<Self::Data> {
        None
    }

    fn model_tracker(&self) -> &dyn i_slint_core::model::ModelTracker {
        &self.notify
    }

    fn set_row_data(&self, row: usize, data: Self::Data) {
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}

// #[napi]
// impl JsModel {
//     #[napi(constructor)]
//     pub fn new() -> Self {
//         Self { inner: ModelRc::new(VecModel::default()) }
//     }

//     #[napi(getter)]
//     pub fn row_count(&self) -> u32 {
//         self.inner.row_count() as u32
//     }

//     #[napi(getter)]
//     pub fn model(&self) -> External<ModelRc<Value>> {
//         External::new(self.inner.clone())
//     }
// }
