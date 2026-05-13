// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Conversion between `slint_interpreter::Value` and `wasm_bindgen::JsValue`.

use i_slint_core::model::Model as _;
use slint_interpreter::{Value, ValueType};
use wasm_bindgen::prelude::*;

/// Convert a Slint `Value` to a JavaScript value.
pub fn value_to_js(value: &Value) -> JsValue {
    match value {
        Value::Void => JsValue::UNDEFINED,
        Value::Number(n) => JsValue::from_f64(*n),
        Value::String(s) => JsValue::from_str(s.as_str()),
        Value::Bool(b) => JsValue::from_bool(*b),
        Value::Struct(s) => {
            let obj = js_sys::Object::new();
            for (name, val) in s.iter() {
                js_sys::Reflect::set(&obj, &JsValue::from_str(name), &value_to_js(val))
                    .unwrap_throw();
            }
            obj.into()
        }
        Value::EnumerationValue(_enum_name, variant) => {
            JsValue::from_str(&variant.replace('-', "_"))
        }
        Value::Model(model) => {
            let arr = js_sys::Array::new();
            for i in 0..model.row_count() {
                if let Some(val) = model.row_data(i) {
                    arr.push(&value_to_js(&val));
                }
            }
            arr.into()
        }
        Value::Brush(brush) => {
            match brush {
                i_slint_core::Brush::SolidColor(color) => {
                    let obj = js_sys::Object::new();
                    js_sys::Reflect::set(
                        &obj,
                        &"red".into(),
                        &JsValue::from_f64(color.red() as f64),
                    )
                    .unwrap_throw();
                    js_sys::Reflect::set(
                        &obj,
                        &"green".into(),
                        &JsValue::from_f64(color.green() as f64),
                    )
                    .unwrap_throw();
                    js_sys::Reflect::set(
                        &obj,
                        &"blue".into(),
                        &JsValue::from_f64(color.blue() as f64),
                    )
                    .unwrap_throw();
                    js_sys::Reflect::set(
                        &obj,
                        &"alpha".into(),
                        &JsValue::from_f64(color.alpha() as f64),
                    )
                    .unwrap_throw();
                    obj.into()
                }
                _ => {
                    // For complex brushes, return a string representation
                    JsValue::from_str(&format!("{brush:?}"))
                }
            }
        }
        Value::Image(_) => {
            // Image conversion is complex; return a placeholder for now
            JsValue::from_str("[Image]")
        }
        // Internal types that shouldn't normally be exposed
        _ => JsValue::UNDEFINED,
    }
}

/// Convert a JavaScript value to a Slint `Value`.
/// The optional `ty` hint helps disambiguate (e.g. number vs bool).
pub fn js_to_value(js: &JsValue, ty: Option<&ValueType>) -> Value {
    if js.is_undefined() || js.is_null() {
        return Value::Void;
    }

    match ty {
        Some(ValueType::Number) => Value::Number(js.as_f64().unwrap_or(0.0)),
        Some(ValueType::String) => Value::String(js.as_string().unwrap_or_default().into()),
        Some(ValueType::Bool) => Value::Bool(js.as_bool().unwrap_or(false)),
        _ => {
            // Infer type from JS value
            if let Some(b) = js.as_bool() {
                Value::Bool(b)
            } else if let Some(n) = js.as_f64() {
                Value::Number(n)
            } else if let Some(s) = js.as_string() {
                // Could be a string or an enum value; without type info, treat as string
                Value::String(s.into())
            } else if js_sys::Array::is_array(js) {
                let arr = js_sys::Array::from(js);
                let values: Vec<Value> = arr.iter().map(|v| js_to_value(&v, None)).collect();
                Value::Model(std::rc::Rc::new(i_slint_core::model::VecModel::from(values)).into())
            } else if js.is_object() {
                let obj = js_sys::Object::from(js.clone());
                let entries = js_sys::Object::entries(&obj);
                let mut s = slint_interpreter::Struct::default();
                for i in 0..entries.length() {
                    let entry = js_sys::Array::from(&entries.get(i));
                    let key = entry.get(0).as_string().unwrap_or_default();
                    let val = js_to_value(&entry.get(1), None);
                    s.set_field(key, val);
                }
                Value::Struct(s)
            } else {
                Value::Void
            }
        }
    }
}
