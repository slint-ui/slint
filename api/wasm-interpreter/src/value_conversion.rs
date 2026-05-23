// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Conversion between `slint_interpreter::Value` and `wasm_bindgen::JsValue`.

use std::rc::Rc;

use i_slint_core::Brush;
use i_slint_core::model::{Model, ModelNotify, ModelRc, ModelTracker};
use slint_interpreter::{Value, ValueType};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use crate::notify_from_id;

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
                js_sys::Reflect::set(
                    &obj,
                    &JsValue::from_str(&name.replace('-', "_")),
                    &value_to_js(val),
                )
                .unwrap_throw();
            }
            obj.into()
        }
        Value::EnumerationValue(_enum_name, variant) => {
            JsValue::from_str(&variant.replace('-', "_"))
        }
        Value::Model(model) => {
            // If the Slint model is a thin wrapper around a JS Model, return
            // the underlying JS object so JS-side identity is preserved.
            if let Some(js_model) = model.as_any().downcast_ref::<WasmJsModel>() {
                return js_model.js_impl.clone();
            }
            let arr = js_sys::Array::new();
            for i in 0..model.row_count() {
                if let Some(val) = model.row_data(i) {
                    arr.push(&value_to_js(&val));
                }
            }
            arr.into()
        }
        Value::Brush(brush) => brush_to_js(brush),
        Value::Image(_) => {
            // Image conversion is not yet implemented; return a placeholder.
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
        Some(ValueType::String) => {
            Value::String(js.as_string().unwrap_or_default().into())
        }
        Some(ValueType::Bool) => Value::Bool(js.as_bool().unwrap_or(false)),
        Some(ValueType::Brush) => js_to_brush(js).unwrap_or(Value::Void),
        _ => js_to_value_infer(js),
    }
}

fn js_to_value_infer(js: &JsValue) -> Value {
    if let Some(b) = js.as_bool() {
        Value::Bool(b)
    } else if let Some(n) = js.as_f64() {
        Value::Number(n)
    } else if let Some(s) = js.as_string() {
        // Try CSS color first — if it parses, treat as Brush. Otherwise String.
        // This is how `appWindow.someBrushProp = "#ff0000"` works without a
        // type hint, e.g. on a struct field nested in a Model.
        match parse_css_color(&s) {
            Some(c) => Value::Brush(Brush::SolidColor(c)),
            None => Value::String(s.into()),
        }
    } else if js_sys::Array::is_array(js) {
        let arr = js_sys::Array::from(js);
        let values: Vec<Value> = arr.iter().map(|v| js_to_value(&v, None)).collect();
        Value::Model(Rc::new(i_slint_core::model::VecModel::from(values)).into())
    } else if js.is_object() {
        let obj = js_sys::Object::from(js.clone());
        // Detect a JS Model (carries a `modelNotify` of our exported type).
        if let Some(model) = try_wrap_js_model(&obj) {
            return Value::Model(model);
        }
        // Detect an RGBA object: { red, green, blue, alpha? }.
        if let Some(brush) = try_rgba_object(&obj) {
            return Value::Brush(brush);
        }
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

fn js_to_brush(js: &JsValue) -> Option<Value> {
    if let Some(s) = js.as_string() {
        return parse_css_color(&s).map(|c| Value::Brush(Brush::SolidColor(c)));
    }
    if js.is_object() {
        let obj = js_sys::Object::from(js.clone());
        if let Some(brush) = try_rgba_object(&obj) {
            return Some(Value::Brush(brush));
        }
    }
    None
}

fn try_rgba_object(obj: &js_sys::Object) -> Option<Brush> {
    let red = js_sys::Reflect::get(obj, &"red".into()).ok()?.as_f64()?;
    let green = js_sys::Reflect::get(obj, &"green".into()).ok()?.as_f64()?;
    let blue = js_sys::Reflect::get(obj, &"blue".into()).ok()?.as_f64()?;
    let alpha = js_sys::Reflect::get(obj, &"alpha".into())
        .ok()
        .and_then(|v| v.as_f64())
        .unwrap_or(255.0);
    Some(Brush::SolidColor(i_slint_core::Color::from_argb_u8(
        clamp_u8(alpha),
        clamp_u8(red),
        clamp_u8(green),
        clamp_u8(blue),
    )))
}

fn clamp_u8(v: f64) -> u8 {
    if v < 0.0 {
        0
    } else if v > 255.0 {
        255
    } else {
        v as u8
    }
}

fn brush_to_js(brush: &Brush) -> JsValue {
    // Always return a CSS color string for solid colors so that JS code can
    // round-trip a value through a property without losing the format
    // (mirrors `appWindow.foo = "#abcdef"; const c = appWindow.foo;`).
    let c = brush.color();
    let s = if c.alpha() == 255 {
        format!("#{:02x}{:02x}{:02x}", c.red(), c.green(), c.blue())
    } else {
        format!(
            "#{:02x}{:02x}{:02x}{:02x}",
            c.red(),
            c.green(),
            c.blue(),
            c.alpha()
        )
    };
    JsValue::from_str(&s)
}

/// Parse a CSS color literal: `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`,
/// `rgb(r, g, b)`, `rgba(r, g, b, a)`, `hsl(...)`, `hsla(...)`, or a named
/// color (e.g. `"red"`). Returns `None` if the string is not a valid color.
fn parse_css_color(s: &str) -> Option<i_slint_core::Color> {
    let c = s.trim().parse::<css_color_parser2::Color>().ok()?;
    Some(i_slint_core::Color::from_argb_u8(
        (c.a * 255.0).round() as u8,
        c.r,
        c.g,
        c.b,
    ))
}

/// Try to detect a JS Model instance (one created from `slint-js-common`'s
/// `Model` / `ArrayModel` classes) and wrap it as a Rust `ModelRc<Value>`
/// that forwards calls back to JS.
fn try_wrap_js_model(obj: &js_sys::Object) -> Option<ModelRc<Value>> {
    let notify_js = js_sys::Reflect::get(obj, &"modelNotify".into()).ok()?;
    if notify_js.is_undefined() || notify_js.is_null() {
        return None;
    }
    let id_js = js_sys::Reflect::get(&notify_js, &"id".into()).ok()?;
    let id = id_js.as_f64()? as u32;
    let notify = notify_from_id(id)?;
    Some(ModelRc::new(WasmJsModel {
        js_impl: obj.clone().into(),
        notify,
    }))
}

/// A `Model<Value>` implementation that wraps a JavaScript Model class.
///
/// Holds a strong reference to the JS object — the underlying object stays
/// alive as long as Slint holds the `ModelRc`. Cycles would require a
/// `WeakRef`-based design (deferred until needed).
pub(crate) struct WasmJsModel {
    pub(crate) js_impl: JsValue,
    notify: Rc<ModelNotify>,
}

impl Model for WasmJsModel {
    type Data = Value;

    fn row_count(&self) -> usize {
        let Ok(func) = js_sys::Reflect::get(&self.js_impl, &"rowCount".into()) else {
            return 0;
        };
        let Some(func) = func.dyn_ref::<js_sys::Function>() else {
            return 0;
        };
        let Ok(result) = func.call0(&self.js_impl) else {
            return 0;
        };
        result.as_f64().unwrap_or(0.0) as usize
    }

    fn row_data(&self, row: usize) -> Option<Self::Data> {
        let func =
            js_sys::Reflect::get(&self.js_impl, &"rowData".into()).ok()?;
        let func = func.dyn_ref::<js_sys::Function>()?;
        let result = func
            .call1(&self.js_impl, &JsValue::from_f64(row as f64))
            .ok()?;
        if result.is_undefined() || result.is_null() {
            None
        } else {
            Some(js_to_value(&result, None))
        }
    }

    fn set_row_data(&self, row: usize, data: Self::Data) {
        let Ok(func) = js_sys::Reflect::get(&self.js_impl, &"setRowData".into()) else {
            return;
        };
        let Some(func) = func.dyn_ref::<js_sys::Function>() else {
            return;
        };
        let _ = func.call2(
            &self.js_impl,
            &JsValue::from_f64(row as f64),
            &value_to_js(&data),
        );
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        &*self.notify
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}
