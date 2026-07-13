// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Conversion between `slint_interpreter::Value` and `wasm_bindgen::JsValue`.

use std::rc::Rc;

use i_slint_compiler::langtype::Type;
use i_slint_core::graphics::{Image, Rgba8Pixel, SharedPixelBuffer};
use i_slint_core::model::{Model, ModelNotify, ModelRc, ModelTracker, SharedVectorModel};
use i_slint_core::{Brush, SharedVector};
use slint_interpreter::Value;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;

use crate::notify_from_id;
use crate::shared::{image_to_rgba8, parse_css_color};

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
            rust_model_to_js(model)
        }
        Value::Brush(brush) => brush_to_js(brush),
        Value::Image(image) => image_to_js(image),
        Value::Keys(keys) => crate::wasm::types::WasmKeys::new(keys.clone()).into(),
        Value::StyledText(styled_text) => {
            crate::wasm::types::WasmStyledText::new(styled_text.clone()).into()
        }
        // Internal types that shouldn't normally be exposed
        _ => JsValue::UNDEFINED,
    }
}

fn conversion_error(message: &str) -> JsValue {
    js_sys::Error::new(message).into()
}

/// The JS type of a value, using the same names as Node.js' `napi::ValueType`
/// so both bindings produce the same "expect Number, got: …" messages.
fn js_type_name(js: &JsValue) -> &'static str {
    if js.is_undefined() {
        "Undefined"
    } else if js.is_null() {
        "Null"
    } else if js.as_bool().is_some() {
        "Boolean"
    } else if js.as_f64().is_some() {
        "Number"
    } else if js.as_string().is_some() {
        "String"
    } else if js.is_function() {
        "Function"
    } else if js.is_object() {
        "Object"
    } else {
        "Unknown"
    }
}

/// Convert a JavaScript value to a Slint `Value`, driven by the declared
/// `.slint` type. Mirrors the Node.js binding's `to_value`: wrong JS types
/// are errors (which the callers throw as exceptions), enum values are
/// validated, and struct fields and array elements convert with their
/// declared types.
pub fn js_to_value_typed(js: &JsValue, ty: &Type) -> Result<Value, JsValue> {
    match ty {
        Type::Float32
        | Type::Int32
        | Type::Duration
        | Type::Angle
        | Type::PhysicalLength
        | Type::LogicalLength
        | Type::Rem
        | Type::Percent
        | Type::UnitProduct(_) => js
            .as_f64()
            .map(Value::Number)
            .ok_or_else(|| conversion_error(&format!("expect Number, got: {}", js_type_name(js)))),
        Type::String => js
            .as_string()
            .map(|s| Value::String(s.into()))
            .ok_or_else(|| conversion_error(&format!("expect String, got: {}", js_type_name(js)))),
        Type::Bool => js.as_bool().map(Value::Bool).ok_or_else(|| {
            conversion_error(&format!("expect Boolean, got: {}", js_type_name(js)))
        }),
        Type::Color | Type::Brush => {
            if let Some(s) = js.as_string() {
                return parse_css_color(&s)
                    .map(|c| Value::Brush(Brush::SolidColor(c)))
                    .ok_or_else(|| conversion_error(&format!("Could not convert {s} to Brush.")));
            }
            if js.is_object() {
                let obj = js_sys::Object::from(js.clone());
                // The `color` property of the `Brush` interface is optional.
                if js_sys::Object::keys(&obj).length() == 0 {
                    return Ok(Value::Brush(Brush::default()));
                }
                if let Ok(color) = js_sys::Reflect::get(&obj, &"color".into())
                    && color.is_object()
                    && let Some(brush) = try_rgba_object(&js_sys::Object::from(color))
                {
                    return Ok(Value::Brush(brush));
                }
                if let Some(brush) = try_rgba_object(&obj) {
                    return Ok(Value::Brush(brush));
                }
            }
            Err(conversion_error(
                "Cannot convert object to brush, because the given object is neither a brush, color, nor a string",
            ))
        }
        Type::Image => js_to_image(js).ok_or_else(|| {
            conversion_error(
                "Cannot convert object to image, because the provided object is not an ImageData-shaped { width, height, data } object",
            )
        }),
        Type::Struct(s) => {
            if !js.is_object() {
                return Err(conversion_error(&format!(
                    "expect Object, got: {}",
                    js_type_name(js)
                )));
            }
            let obj = js_sys::Object::from(js.clone());
            let mut struct_value = slint_interpreter::Struct::default();
            for (field_name, field_type) in s.fields.iter() {
                let prop =
                    js_sys::Reflect::get(&obj, &JsValue::from_str(&field_name.replace('-', "_")))
                        .unwrap_or(JsValue::UNDEFINED);
                let value = if prop.is_undefined() {
                    slint_interpreter::default_value_for_type(field_type)
                } else {
                    js_to_value_typed(&prop, field_type)?
                };
                struct_value.set_field(field_name.to_string(), value);
            }
            Ok(Value::Struct(struct_value))
        }
        Type::Array(a) => {
            if js_sys::Array::is_array(js) {
                let arr = js_sys::Array::from(js);
                let mut vec = Vec::with_capacity(arr.length() as usize);
                for v in arr.iter() {
                    vec.push(js_to_value_typed(&v, a)?);
                }
                Ok(Value::Model(ModelRc::new(SharedVectorModel::from(SharedVector::from_slice(
                    &vec,
                )))))
            } else {
                js.is_object()
                    .then(|| try_wrap_js_model(&js_sys::Object::from(js.clone()), Some(a)))
                    .flatten()
                    .map(Value::Model)
                    .ok_or_else(|| {
                        conversion_error(
                            "expect an Array or an object implementing the Model interface",
                        )
                    })
            }
        }
        Type::Enumeration(e) => {
            let value = js.as_string().ok_or_else(|| {
                conversion_error(&format!("expect String, got: {}", js_type_name(js)))
            })?;
            // JS exposes enum values with underscores, while the .slint
            // declaration may use dashes; accept both spellings but store
            // the declared one.
            let dashed = value.replace('_', "-");
            if e.values.iter().any(|v| v == value.as_str()) {
                Ok(Value::EnumerationValue(e.name.to_string(), value))
            } else if e.values.iter().any(|v| v == dashed.as_str()) {
                Ok(Value::EnumerationValue(e.name.to_string(), dashed))
            } else {
                Err(conversion_error(&format!("{value} is not a value of enum {}", e.name)))
            }
        }
        Type::Keys => crate::wasm::types::try_keys_from_js(js)
            .map(Value::Keys)
            .ok_or_else(|| conversion_error("expect a Keys instance")),
        Type::StyledText => crate::wasm::types::try_styled_text_from_js(js)
            .map(Value::StyledText)
            .ok_or_else(|| conversion_error("expect a StyledText instance")),
        Type::Invalid
        | Type::Model
        | Type::Void
        | Type::InferredProperty
        | Type::InferredCallback
        | Type::Function { .. }
        | Type::Callback { .. }
        | Type::ComponentFactory
        | Type::Easing
        | Type::PathData
        | Type::LayoutCache
        | Type::ArrayOfU16
        | Type::DataTransfer
        | Type::ElementReference => Err(conversion_error(&format!(
            "values of type {ty} cannot be constructed from JavaScript"
        ))),
    }
}

/// Convert a JavaScript value to a Slint `Value` when no declared type is
/// available (rows of a model wrapped without an element type, or fields of
/// ad-hoc structs built by inference).
fn js_to_value_infer(js: &JsValue) -> Value {
    if js.is_undefined() || js.is_null() {
        return Value::Void;
    }
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
        let values: Vec<Value> = arr.iter().map(|v| js_to_value_infer(&v)).collect();
        Value::Model(Rc::new(i_slint_core::model::VecModel::from(values)).into())
    } else if js.is_object() {
        let obj = js_sys::Object::from(js.clone());
        // Detect a JS Model (carries a `modelNotify` of our exported type).
        if let Some(model) = try_wrap_js_model(&obj, None) {
            return Value::Model(model);
        }
        // Detect an RGBA object: { red, green, blue, alpha? }.
        if let Some(brush) = try_rgba_object(&obj) {
            return Value::Brush(brush);
        }
        if let Some(keys) = crate::wasm::types::try_keys_from_js(js) {
            return Value::Keys(keys);
        }
        if let Some(styled_text) = crate::wasm::types::try_styled_text_from_js(js) {
            return Value::StyledText(styled_text);
        }
        // A browser `ImageData` instance is unambiguous, so it converts even
        // without a type hint (e.g. nested in a model row or struct field).
        if obj.constructor().name() == "ImageData"
            && let Some(image) = js_to_image(js)
        {
            return image;
        }
        let entries = js_sys::Object::entries(&obj);
        let mut s = slint_interpreter::Struct::default();
        for i in 0..entries.length() {
            let entry = js_sys::Array::from(&entries.get(i));
            let key = entry.get(0).as_string().unwrap_or_default();
            let val = js_to_value_infer(&entry.get(1));
            s.set_field(key, val);
        }
        Value::Struct(s)
    } else {
        Value::Void
    }
}

/// Expose a Rust model to JS as a read-only model object with `rowCount()`,
/// `rowData(row)`, a warning-only `setRowData`, and the iterator protocol —
/// the same surface as the Node.js binding's ReadOnlyRustModel.
fn rust_model_to_js(model: &ModelRc<Value>) -> JsValue {
    let obj = js_sys::Object::new();

    let m = model.clone();
    let row_count = Closure::<dyn Fn() -> u32>::new(move || m.row_count() as u32);
    let m = model.clone();
    let row_data = Closure::<dyn Fn(u32) -> JsValue>::new(move |row| {
        m.row_data(row as usize).map(|v| value_to_js(&v)).unwrap_or(JsValue::UNDEFINED)
    });
    let set_row_data = Closure::<dyn Fn(u32, JsValue)>::new(move |_row, _data| {
        web_sys::console::log_1(
            &"setRowData called on a model which does not re-implement this method. \
              This happens when trying to modify a read-only model"
                .into(),
        );
    });
    let m = model.clone();
    let iterator = Closure::<dyn Fn() -> JsValue>::new(move || {
        let m = m.clone();
        let row = std::cell::Cell::new(0usize);
        let next = Closure::<dyn Fn() -> JsValue>::new(move || {
            let result = js_sys::Object::new();
            match m.row_data(row.get()) {
                Some(value) => {
                    row.set(row.get() + 1);
                    js_sys::Reflect::set(&result, &"done".into(), &false.into()).unwrap_throw();
                    js_sys::Reflect::set(&result, &"value".into(), &value_to_js(&value))
                        .unwrap_throw();
                }
                None => {
                    js_sys::Reflect::set(&result, &"done".into(), &true.into()).unwrap_throw();
                }
            }
            result.into()
        });
        let iter = js_sys::Object::new();
        js_sys::Reflect::set(&iter, &"next".into(), &next.into_js_value()).unwrap_throw();
        iter.into()
    });

    js_sys::Reflect::set(&obj, &"rowCount".into(), &row_count.into_js_value()).unwrap_throw();
    js_sys::Reflect::set(&obj, &"rowData".into(), &row_data.into_js_value()).unwrap_throw();
    js_sys::Reflect::set(&obj, &"setRowData".into(), &set_row_data.into_js_value()).unwrap_throw();
    js_sys::Reflect::set(&obj, &js_sys::Symbol::iterator(), &iterator.into_js_value())
        .unwrap_throw();
    obj.into()
}

/// Convert a Slint image to a JS value: a browser `ImageData` when that global
/// exists (so the result can go straight to `putImageData`), otherwise a plain
/// `{ width, height, data }` object with the same shape. `data` is RGBA8.
fn image_to_js(image: &Image) -> JsValue {
    let size = image.size();
    let rgba = image_to_rgba8(image);

    let data = js_sys::Uint8ClampedArray::new_with_length(rgba.len() as u32);
    data.copy_from(&rgba);

    let width = JsValue::from_f64(size.width as f64);
    let height = JsValue::from_f64(size.height as f64);
    let obj: JsValue = js_sys::Reflect::get(&js_sys::global(), &"ImageData".into())
        .ok()
        .and_then(|ctor| ctor.dyn_into::<js_sys::Function>().ok())
        .and_then(|ctor| {
            let args = js_sys::Array::of3(&data, &width, &height);
            js_sys::Reflect::construct(&ctor, &args).ok()
        })
        .map(Into::into)
        .unwrap_or_else(|| {
            let obj = js_sys::Object::new();
            js_sys::Reflect::set(&obj, &"width".into(), &width).unwrap_throw();
            js_sys::Reflect::set(&obj, &"height".into(), &height).unwrap_throw();
            js_sys::Reflect::set(&obj, &"data".into(), &data).unwrap_throw();
            obj.into()
        });
    if let Some(path) = image.path() {
        let _ =
            js_sys::Reflect::set(&obj, &"path".into(), &JsValue::from_str(&path.to_string_lossy()));
    }
    obj
}

/// Convert an `ImageData`-shaped JS object (`{ width, height, data }`, with
/// `data` as RGBA8 in a `Uint8ClampedArray` or `Uint8Array` of exactly
/// width * height * 4 bytes) to a Slint image.
fn js_to_image(js: &JsValue) -> Option<Value> {
    if !js.is_object() {
        return None;
    }
    let obj = js_sys::Object::from(js.clone());
    let width = js_sys::Reflect::get(&obj, &"width".into()).ok()?.as_f64()? as u32;
    let height = js_sys::Reflect::get(&obj, &"height".into()).ok()?.as_f64()? as u32;
    let data = js_sys::Reflect::get(&obj, &"data".into()).ok()?;
    let bytes: Vec<u8> = if let Some(a) = data.dyn_ref::<js_sys::Uint8ClampedArray>() {
        a.to_vec()
    } else if let Some(a) = data.dyn_ref::<js_sys::Uint8Array>() {
        a.to_vec()
    } else {
        return None;
    };
    if bytes.len() != width as usize * height as usize * 4 {
        return None;
    }
    Some(Value::Image(Image::from_rgba8(SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
        &bytes, width, height,
    ))))
}

fn try_rgba_object(obj: &js_sys::Object) -> Option<Brush> {
    let red = js_sys::Reflect::get(obj, &"red".into()).ok()?.as_f64()?;
    let green = js_sys::Reflect::get(obj, &"green".into()).ok()?.as_f64()?;
    let blue = js_sys::Reflect::get(obj, &"blue".into()).ok()?.as_f64()?;
    let alpha =
        js_sys::Reflect::get(obj, &"alpha".into()).ok().and_then(|v| v.as_f64()).unwrap_or(255.0);
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
        format!("#{:02x}{:02x}{:02x}{:02x}", c.red(), c.green(), c.blue(), c.alpha())
    };
    JsValue::from_str(&s)
}

/// Try to detect a JS Model instance (one created from `@slint-ui/common`'s
/// `Model` / `ArrayModel` classes) and wrap it as a Rust `ModelRc<Value>`
/// that forwards calls back to JS. `row_type` is the declared element type;
/// rows convert by inference when it is `None`.
fn try_wrap_js_model(obj: &js_sys::Object, row_type: Option<&Type>) -> Option<ModelRc<Value>> {
    let notify_js = js_sys::Reflect::get(obj, &"modelNotify".into()).ok()?;
    if notify_js.is_undefined() || notify_js.is_null() {
        return None;
    }
    let id_js = js_sys::Reflect::get(&notify_js, &"id".into()).ok()?;
    let id = id_js.as_f64()? as u32;
    let notify = notify_from_id(id)?;
    Some(ModelRc::new(WasmJsModel {
        js_impl: obj.clone().into(),
        row_type: row_type.cloned(),
        notify,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::wasm_bindgen_test;

    fn image_data_like(width: u32, height: u32, bytes: &[u8]) -> JsValue {
        let obj = js_sys::Object::new();
        let data = js_sys::Uint8ClampedArray::new_with_length(bytes.len() as u32);
        data.copy_from(bytes);
        js_sys::Reflect::set(&obj, &"width".into(), &JsValue::from_f64(width as f64)).unwrap();
        js_sys::Reflect::set(&obj, &"height".into(), &JsValue::from_f64(height as f64)).unwrap();
        js_sys::Reflect::set(&obj, &"data".into(), &data).unwrap();
        obj.into()
    }

    #[wasm_bindgen_test]
    fn image_roundtrip() {
        let bytes: Vec<u8> = (0..16).map(|i| i * 3).collect();
        let value = js_to_value_typed(&image_data_like(2, 2, &bytes), &Type::Image).unwrap();
        let Value::Image(ref image) = value else {
            panic!("expected an image value");
        };
        assert_eq!(image.size().width, 2);
        assert_eq!(image.size().height, 2);

        let js = value_to_js(&value);
        let get = |name: &str| js_sys::Reflect::get(&js, &name.into()).unwrap();
        assert_eq!(get("width").as_f64(), Some(2.0));
        assert_eq!(get("height").as_f64(), Some(2.0));
        let data: js_sys::Uint8ClampedArray = get("data").dyn_into().unwrap();
        assert_eq!(data.to_vec(), bytes);
    }

    #[wasm_bindgen_test]
    fn image_rejects_wrong_buffer_size() {
        let bytes = [0u8; 12]; // 2x2 RGBA needs 16
        assert!(js_to_image(&image_data_like(2, 2, &bytes)).is_none());
    }
}

/// A `Model<Value>` implementation that wraps a JavaScript Model class.
///
/// Holds a strong reference to the JS object — the underlying object stays
/// alive as long as Slint holds the `ModelRc`. Cycles would require a
/// `WeakRef`-based design (deferred until needed).
pub(crate) struct WasmJsModel {
    pub(crate) js_impl: JsValue,
    /// The declared element type, when the model was set on a typed
    /// property; rows convert by inference otherwise.
    row_type: Option<Type>,
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
        let func = js_sys::Reflect::get(&self.js_impl, &"rowData".into()).ok()?;
        let func = func.dyn_ref::<js_sys::Function>()?;
        let result = func.call1(&self.js_impl, &JsValue::from_f64(row as f64)).ok()?;
        if result.is_undefined() || result.is_null() {
            None
        } else if let Some(ty) = &self.row_type {
            match js_to_value_typed(&result, ty) {
                Ok(value) => Some(value),
                Err(_) => {
                    web_sys::console::error_1(
                        &"JavaScript Model<T>'s rowData function returned data type that cannot be represented in Rust"
                            .into(),
                    );
                    None
                }
            }
        } else {
            Some(js_to_value_infer(&result))
        }
    }

    fn set_row_data(&self, row: usize, data: Self::Data) {
        let Ok(func) = js_sys::Reflect::get(&self.js_impl, &"setRowData".into()) else {
            return;
        };
        let Some(func) = func.dyn_ref::<js_sys::Function>() else {
            return;
        };
        let _ = func.call2(&self.js_impl, &JsValue::from_f64(row as f64), &value_to_js(&data));
    }

    fn push_row(&self, data: Self::Data) {
        let Ok(func) = js_sys::Reflect::get(&self.js_impl, &"pushRow".into()) else {
            return;
        };
        let Some(func) = func.dyn_ref::<js_sys::Function>() else {
            return;
        };
        let _ = func.call1(&self.js_impl, &value_to_js(&data));
    }

    fn remove_row(&self, row: isize) {
        let Ok(func) = js_sys::Reflect::get(&self.js_impl, &"removeRow".into()) else {
            return;
        };
        let Some(func) = func.dyn_ref::<js_sys::Function>() else {
            return;
        };
        let _ = func.call1(&self.js_impl, &JsValue::from_f64(row as f64));
    }

    fn insert_row(&self, row: isize, data: Self::Data) {
        let Ok(func) = js_sys::Reflect::get(&self.js_impl, &"insertRow".into()) else {
            return;
        };
        let Some(func) = func.dyn_ref::<js_sys::Function>() else {
            return;
        };
        let _ = func.call2(&self.js_impl, &JsValue::from_f64(row as f64), &value_to_js(&data));
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        &*self.notify
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}
