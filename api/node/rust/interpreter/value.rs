// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::{
    js_into_rust_model, rust_into_js_model, ReadOnlyRustModel, RgbaColor, SlintBrush,
    SlintImageData,
};
use i_slint_compiler::langtype::Type;
use i_slint_core::graphics::{Image, Rgba8Pixel, SharedPixelBuffer};
use i_slint_core::model::{ModelRc, SharedVectorModel};
use i_slint_core::{Brush, Color, SharedVector};
use napi::bindgen_prelude::*;
use napi::{Env, JsBoolean, JsNumber, JsObject, JsString, JsUnknown, Result};
use napi_derive::napi;
use slint_interpreter::Value;
use smol_str::SmolStr;

#[napi(js_name = "ValueType")]
pub enum JsValueType {
    Void,
    Number,
    String,
    Bool,
    Model,
    Struct,
    Brush,
    Image,
}

impl From<slint_interpreter::ValueType> for JsValueType {
    fn from(value_type: slint_interpreter::ValueType) -> Self {
        match value_type {
            slint_interpreter::ValueType::Number => JsValueType::Number,
            slint_interpreter::ValueType::String => JsValueType::String,
            slint_interpreter::ValueType::Bool => JsValueType::Bool,
            slint_interpreter::ValueType::Model => JsValueType::Model,
            slint_interpreter::ValueType::Struct => JsValueType::Struct,
            slint_interpreter::ValueType::Brush => JsValueType::Brush,
            slint_interpreter::ValueType::Image => JsValueType::Image,
            _ => JsValueType::Void,
        }
    }
}

#[napi(js_name = "Property")]
pub struct JsProperty {
    pub name: String,
    pub value_type: JsValueType,
}

pub fn to_js_unknown(env: &Env, value: &Value) -> Result<JsUnknown> {
    match value {
        Value::Void => env.get_null().map(|v| v.into_unknown()),
        Value::Number(number) => env.create_double(*number).map(|v| v.into_unknown()),
        Value::String(string) => env.create_string(string).map(|v| v.into_unknown()),
        Value::Bool(value) => env.get_boolean(*value).map(|v| v.into_unknown()),
        Value::Image(image) => Ok(SlintImageData::from(image.clone())
            .into_instance(*env)?
            .as_object(*env)
            .into_unknown()),
        Value::Struct(struct_value) => {
            let mut o = env.create_object()?;
            for (field_name, field_value) in struct_value.iter() {
                o.set_property(
                    env.create_string(&field_name.replace('-', "_"))?,
                    to_js_unknown(env, field_value)?,
                )?;
            }
            Ok(o.into_unknown())
        }
        Value::Brush(brush) => {
            Ok(SlintBrush::from(brush.clone()).into_instance(*env)?.as_object(*env).into_unknown())
        }
        Value::Model(model) => {
            if let Some(maybe_js_model) = rust_into_js_model(model) {
                maybe_js_model
            } else {
                let model_wrapper: ReadOnlyRustModel = model.clone().into();
                model_wrapper.into_js(env)
            }
        }
        Value::EnumerationValue(_, value) => env.create_string(value).map(|v| v.into_unknown()),
        _ => env.get_undefined().map(|v| v.into_unknown()),
    }
}

pub fn to_value(env: &Env, unknown: JsUnknown, typ: &Type) -> Result<Value> {
    match typ {
        Type::Float32
        | Type::Int32
        | Type::Duration
        | Type::Angle
        | Type::PhysicalLength
        | Type::LogicalLength
        | Type::Rem
        | Type::Percent
        | Type::UnitProduct(_) => {
            let js_number: Result<JsNumber> = unknown.try_into();
            Ok(Value::Number(js_number?.get_double()?))
        }
        Type::String => {
            let js_string: JsString = unknown.try_into()?;
            Ok(Value::String(js_string.into_utf8()?.as_str()?.into()))
        }
        Type::Bool => {
            let js_bool: JsBoolean = unknown.try_into()?;
            Ok(Value::Bool(js_bool.get_value()?))
        }
        Type::Color => {
            match unknown.get_type() {
                Ok(ValueType::String) => {
                    return Ok(unknown.coerce_to_string().and_then(|str| string_to_brush(str))?);
                }
                Ok(ValueType::Object) => {
                    if let Ok(rgb_color) = unknown.coerce_to_object() {
                        return brush_from_color(rgb_color);
                    }
                }
                _ => {}
            }
            Err(napi::Error::from_reason(
                            "Cannot convert object to brush, because the given object is neither a brush, color, nor a string".to_string()
                    ))
        }
        Type::Brush => {
            match unknown.get_type() {
                Ok(ValueType::String) => {
                    return Ok(unknown.coerce_to_string().and_then(|str| string_to_brush(str))?);
                }
                Ok(ValueType::Object) => {
                    if let Ok(obj) = unknown.coerce_to_object() {
                        // this is used to make the color property of the `Brush` interface optional.
                        let properties = obj.get_property_names()?;
                        if properties.get_array_length()? == 0 {
                            return Ok(Value::Brush(Brush::default()));
                        }
                        if let Some(color) = obj.get::<&str, RgbaColor>("color").ok().flatten() {
                            if color.red() < 0.
                                || color.green() < 0.
                                || color.blue() < 0.
                                || color.alpha() < 0.
                            {
                                return Err(Error::from_reason(
                                    "A channel of Color cannot be negative",
                                ));
                            }

                            return Ok(Value::Brush(Brush::SolidColor(Color::from_argb_u8(
                                color.alpha() as u8,
                                color.red() as u8,
                                color.green() as u8,
                                color.blue() as u8,
                            ))));
                        } else {
                            return brush_from_color(obj);
                        }
                    }
                }
                _ => {}
            }
            Err(napi::Error::from_reason(
                            "Cannot convert object to brush, because the given object is neither a brush, color, nor a string".to_string()
                    ))
        }
        Type::Image => {
            let object = unknown.coerce_to_object()?;
            if let Some(direct_image) = object.get("image").ok().flatten() {
                Ok(Value::Image(env.get_value_external::<Image>(&direct_image)?.clone()))
            } else {
                let get_size_prop = |name| {
                    object
                    .get::<_, JsUnknown>(name)
                    .ok()
                    .flatten()
                    .and_then(|prop| prop.coerce_to_number().ok())
                    .and_then(|number| number.get_int64().ok())
                    .and_then(|i64_num| i64_num.try_into().ok())
                    .ok_or_else(
                        || napi::Error::from_reason(
                            format!("Cannot convert object to image, because the provided object does not have an u32 `{name}` property")
                    ))
                };

                fn try_convert_image<BufferType: AsRef<[u8]> + FromNapiValue>(
                    object: &JsObject,
                    width: u32,
                    height: u32,
                ) -> Result<SharedPixelBuffer<Rgba8Pixel>> {
                    let buffer =
                        object.get::<_, BufferType>("data").ok().flatten().ok_or_else(|| {
                            napi::Error::from_reason(
                                "data property does not have suitable array buffer type"
                                    .to_string(),
                            )
                        })?;
                    const BPP: usize = core::mem::size_of::<Rgba8Pixel>();
                    let actual_size = buffer.as_ref().len();
                    let expected_size: usize = (width as usize) * (height as usize) * BPP;
                    if actual_size != expected_size {
                        return Err(napi::Error::from_reason(format!(
                            "data property does not have the correct size; expected {width} (width) * {height} (height) * {BPP} = {actual_size}; got {expected_size}"
                        )));
                    }

                    Ok(SharedPixelBuffer::clone_from_slice(buffer.as_ref(), width, height))
                }

                let width: u32 = get_size_prop("width")?;
                let height: u32 = get_size_prop("height")?;

                let pixel_buffer =
                    try_convert_image::<Uint8ClampedArray>(&object, width, height)
                        .or_else(|_| try_convert_image::<Buffer>(&object, width, height))?;

                Ok(Value::Image(Image::from_rgba8(pixel_buffer)))
            }
        }
        Type::Struct(s) => {
            let js_object = unknown.coerce_to_object()?;

            Ok(Value::Struct(
                s.fields
                    .iter()
                    .map(|(pro_name, pro_ty)| {
                        let prop: JsUnknown = js_object
                            .get_property(env.create_string(&pro_name.replace('-', "_"))?)?;
                        let prop_value = if prop.get_type()? == napi::ValueType::Undefined {
                            slint_interpreter::default_value_for_type(pro_ty)
                        } else {
                            to_value(env, prop, pro_ty)?
                        };
                        Ok((pro_name.to_string(), prop_value))
                    })
                    .collect::<Result<_, _>>()?,
            ))
        }
        Type::Array(a) => {
            if unknown.is_array()? {
                let array = Array::from_unknown(unknown)?;
                let mut vec = vec![];

                for i in 0..array.len() {
                    vec.push(to_value(
                        env,
                        array.get(i)?.ok_or(napi::Error::from_reason(format!(
                            "Cannot access array element at index {i}"
                        )))?,
                        a,
                    )?);
                }
                Ok(Value::Model(ModelRc::new(SharedVectorModel::from(SharedVector::from_slice(
                    &vec,
                )))))
            } else {
                let rust_model =
                    unknown.coerce_to_object().and_then(|obj| js_into_rust_model(env, &obj, a))?;
                Ok(Value::Model(rust_model))
            }
        }
        Type::Enumeration(e) => {
            let js_string: JsString = unknown.try_into()?;
            let value: SmolStr = js_string.into_utf8()?.as_str()?.into();

            if !e.values.contains(&value) {
                return Err(napi::Error::from_reason(format!(
                    "{value} is not a value of enum {}",
                    e.name
                )));
            }

            Ok(Value::EnumerationValue(e.name.to_string(), value.to_string()))
        }
        Type::Invalid
        | Type::Model
        | Type::Void
        | Type::InferredProperty
        | Type::InferredCallback
        | Type::Function { .. }
        | Type::Callback { .. }
        | Type::ComponentFactory { .. }
        | Type::Easing
        | Type::PathData
        | Type::LayoutCache
        | Type::ElementReference => Err(napi::Error::from_reason("reason")),
    }
}

fn string_to_brush(js_string: JsString) -> Result<Value> {
    let string = js_string.into_utf8()?.as_str()?.to_string();

    let c = string
        .parse::<css_color_parser2::Color>()
        .map_err(|_| napi::Error::from_reason(format!("Could not convert {string} to Brush.")))?;

    Ok(Value::Brush(Brush::from(Color::from_argb_u8((c.a * 255.) as u8, c.r, c.g, c.b)).into()))
}

fn brush_from_color(rgb_color: Object) -> Result<Value> {
    let red: f64 = rgb_color.get("red")?.ok_or(Error::from_reason("Property red is missing"))?;
    let green: f64 =
        rgb_color.get("green")?.ok_or(Error::from_reason("Property green is missing"))?;
    let blue: f64 = rgb_color.get("blue")?.ok_or(Error::from_reason("Property blue is missing"))?;
    let alpha: f64 = rgb_color.get("alpha")?.unwrap_or(255.);

    if red < 0. || green < 0. || blue < 0. || alpha < 0. {
        return Err(Error::from_reason("A channel of Color cannot be negative"));
    }

    Ok(Value::Brush(Brush::SolidColor(Color::from_argb_u8(
        alpha as u8,
        red as u8,
        green as u8,
        blue as u8,
    ))))
}
