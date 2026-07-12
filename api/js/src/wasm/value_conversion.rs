// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Conversion between `slint_interpreter::Value` and `wasm_bindgen::JsValue`.

use std::rc::Rc;

use i_slint_core::graphics::{Image, Rgba8Pixel, SharedImageBuffer, SharedPixelBuffer};
use i_slint_core::model::{Model, ModelNotify, ModelRc, ModelTracker};
use i_slint_core::{Brush, ImageInner};
use slint_interpreter::{Value, ValueType};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;

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
        Value::Image(image) => image_to_js(image),
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
        Some(ValueType::Brush) => js_to_brush(js).unwrap_or(Value::Void),
        Some(ValueType::Image) => js_to_image(js).unwrap_or(Value::Void),
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
            let val = js_to_value(&entry.get(1), None);
            s.set_field(key, val);
        }
        Value::Struct(s)
    } else {
        Value::Void
    }
}

/// Convert a Slint image to a JS value: a browser `ImageData` when that global
/// exists (so the result can go straight to `putImageData`), otherwise a plain
/// `{ width, height, data }` object with the same shape. `data` is RGBA8.
fn image_to_js(image: &Image) -> JsValue {
    let size = image.size();
    let image_inner: &ImageInner = image.into();
    let rgba: Vec<u8> = match image_inner.render_to_buffer(None) {
        Some(SharedImageBuffer::RGBA8(buffer)) => buffer.as_bytes().to_vec(),
        Some(SharedImageBuffer::RGB8(buffer)) => {
            let mut rgba = Vec::with_capacity(buffer.as_bytes().len() / 3 * 4);
            for px in buffer.as_bytes().chunks_exact(3) {
                rgba.extend_from_slice(px);
                rgba.push(255);
            }
            rgba
        }
        Some(SharedImageBuffer::RGBA8Premultiplied(buffer)) => {
            let mut rgba = buffer.as_bytes().to_vec();
            for px in rgba.chunks_exact_mut(4) {
                let a = px[3] as u16;
                if a > 0 && a < 255 {
                    px[0] = (px[0] as u16 * 255 / a) as u8;
                    px[1] = (px[1] as u16 * 255 / a) as u8;
                    px[2] = (px[2] as u16 * 255 / a) as u8;
                }
            }
            rgba
        }
        None => vec![0; size.width as usize * size.height as usize * 4],
    };

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

/// Parse a CSS color literal: `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`,
/// `rgb(r, g, b)`, `rgba(r, g, b, a)`, `hsl(...)`, `hsla(...)`, or a named
/// color (e.g. `"red"`). Returns `None` if the string is not a valid color.
fn parse_css_color(s: &str) -> Option<i_slint_core::Color> {
    let c = s.trim().parse::<css_color_parser2::Color>().ok()?;
    Some(i_slint_core::Color::from_argb_u8((c.a * 255.0).round() as u8, c.r, c.g, c.b))
}

/// Try to detect a JS Model instance (one created from `@slint-ui/common`'s
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
    Some(ModelRc::new(WasmJsModel { js_impl: obj.clone().into(), notify }))
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
        let value = js_to_value(&image_data_like(2, 2, &bytes), Some(&ValueType::Image));
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
