// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use napi::{
    bindgen_prelude::{FromNapiValue, Object},
    JsUnknown,
};

/// SlintPoint implements {@link Point}.
#[napi]
pub struct SlintPoint {
    pub x: f64,
    pub y: f64,
}

#[napi]
impl SlintPoint {
    /// Constructs new point from x and y.
    #[napi(constructor)]
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

impl FromNapiValue for SlintPoint {
    unsafe fn from_napi_value(
        env: napi::sys::napi_env,
        napi_val: napi::sys::napi_value,
    ) -> napi::Result<Self> {
        let obj = unsafe { Object::from_napi_value(env, napi_val)? };
        let x: f64 = obj
            .get::<_, JsUnknown>("x")
            .ok()
            .flatten()
            .and_then(|p| p.coerce_to_number().ok())
            .and_then(|f64_num| f64_num.try_into().ok())
            .ok_or_else(
                || napi::Error::from_reason(
                    "Cannot convert object to Point, because the provided object does not have an f64 x property".to_string()
            ))?;
        let y: f64 = obj
            .get::<_, JsUnknown>("y")
            .ok()
            .flatten()
            .and_then(|p| p.coerce_to_number().ok())
            .and_then(|f64_num| f64_num.try_into().ok())
            .ok_or_else(
                || napi::Error::from_reason(
                    "Cannot convert object to Point, because the provided object does not have an f64 y property".to_string()
            ))?;

        Ok(SlintPoint { x, y })
    }
}
