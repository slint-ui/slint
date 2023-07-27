// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

use i_slint_core::model::{Model, ModelRc, VecModel};
use napi::bindgen_prelude::External;
use slint_interpreter::Value;

#[napi(js_name = Model)]
pub struct JsModel {
    inner: ModelRc<Value>,
}

impl From<ModelRc<Value>> for JsModel {
    fn from(model: ModelRc<Value>) -> Self {
        Self { inner: model }
    }
}

#[napi]
impl JsModel {
    #[napi(constructor)]
    pub fn new() -> Self {
        Self { inner: ModelRc::new(VecModel::default()) }
    }

    #[napi(getter)]
    pub fn row_count(&self) -> u32 {
        self.inner.row_count() as u32
    }

    #[napi(getter)]
    pub fn model(&self) -> External<ModelRc<Value>> {
        External::new(self.inner.clone())
    }
}
