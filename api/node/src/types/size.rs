// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use napi::{
    bindgen_prelude::{FromNapiValue, Object},
    JsUnknown, Result,
};

/// SlintPoint implements {@link Size}.
#[napi]
pub struct SlintSize {
    pub width: f64,
    pub height: f64,
}

#[napi]
impl SlintSize {
    /// Constructs a size from the given width and height.
    #[napi(constructor)]
    pub fn new(width: f64, height: f64) -> Result<Self> {
        if width < 0. {
            return Err(napi::Error::from_reason("width cannot be negative".to_string()));
        }

        if height < 0. {
            return Err(napi::Error::from_reason("height cannot be negative".to_string()));
        }

        Ok(Self { width, height })
    }
}

impl FromNapiValue for SlintSize {
    unsafe fn from_napi_value(
        env: napi::sys::napi_env,
        napi_val: napi::sys::napi_value,
    ) -> napi::Result<Self> {
        let obj = unsafe { Object::from_napi_value(env, napi_val)? };
        let width: f64 = obj
            .get::<_, JsUnknown>("width")
            .ok()
            .flatten()
            .and_then(|p| p.coerce_to_number().ok())
            .and_then(|f64_num| f64_num.try_into().ok())
            .ok_or_else(
                || napi::Error::from_reason(
                    "Cannot convert object to Size, because the provided object does not have an f64 width property".to_string()
            ))?;
        let height:  f64 = obj
            .get::<_, JsUnknown>("height")
            .ok()
            .flatten()
            .and_then(|p| p.coerce_to_number().ok())
            .and_then(|f64_num| f64_num.try_into().ok())
            .ok_or_else(
                || napi::Error::from_reason(
                    "Cannot convert object to Size, because the provided object does not have an f64 height property".to_string()
            ))?;

        Ok(SlintSize { width, height })
    }
}
