// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

use i_slint_core::model::{Model, ModelRc, VecModel};
use napi::{bindgen_prelude::{External, ToNapiValue, Object}, Env};
use napi_derive::napi;
use slint_interpreter::Value;

pub struct JsModel {
    object: Object,
    env: Env
}

impl JsModel {
    pub fn new(env: Env, object: Object) -> Self {
        Self {
            env,
            object
        }
    }
}

impl Model for JsModel {
    type Data = slint_interpreter::Value;

    fn row_count(&self) -> usize {
        todo!()
    }

    fn row_data(&self, row: usize) -> Option<Self::Data> {
        todo!()
    }

    fn model_tracker(&self) -> &dyn i_slint_core::model::ModelTracker {
        todo!()
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
