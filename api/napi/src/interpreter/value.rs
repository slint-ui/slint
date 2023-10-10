// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

use crate::{JsBrush, JsImageData, JsModel};
use i_slint_compiler::langtype::Type;
use i_slint_core::graphics::{Image, Rgba8Pixel, SharedPixelBuffer};
use i_slint_core::model::{Model, ModelRc, SharedVectorModel};
use i_slint_core::{Brush, Color, SharedVector};
use napi::{bindgen_prelude::*, Env, JsBoolean, JsNumber, JsObject, JsString, JsUnknown, Result};
use napi_derive::napi;
use slint_interpreter::Value;

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
        Value::Image(image) => {
            Ok(JsImageData::from(image.clone()).into_instance(*env)?.as_object(*env).into_unknown())
        }
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
            Ok(JsBrush::from(brush.clone()).into_instance(*env)?.as_object(*env).into_unknown())
        }
        Value::Model(model) => {
            if let Some(js_model) = model.as_any().downcast_ref::<JsModel>() {
                let model: Object = js_model.model().get()?;
                Ok(model.into_unknown())
            } else {
                let mut vec = vec![];

                for i in 0..model.row_count() {
                    vec.push(to_js_unknown(env, &model.row_data(i).unwrap())?);
                }

                Ok(Array::from_vec(env, vec)?.coerce_to_object()?.into_unknown())
            }
        }
        _ => env.get_undefined().map(|v| v.into_unknown()),
    }
}

pub fn to_value(env: &Env, unknown: JsUnknown, typ: Type) -> Result<Value> {
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
            let color_ref = env.create_reference(unknown.coerce_to_object()?)?;
            if let Some(js_color) = env
                .get_reference_value::<JsObject>(&color_ref)
                .ok()
                .and_then(|obj| obj.get("color").ok().flatten())
                .and_then(|brush_prop| env.get_value_external::<Brush>(&brush_prop).ok())
            {
                return Ok(Value::Brush(js_color.clone()));
            }

            if let Some(js_brush) = env
                .get_reference_value::<JsObject>(&color_ref)
                .ok()
                .and_then(|js_object| js_object.coerce_to_string().ok())
                .and_then(|string| string_to_brush(string).ok())
            {
                return Ok(js_brush);
            } else {
                return Err(napi::Error::from_reason(
                            format!("Cannot convert object to brush, because the given object is neither a brush nor a string")
                    ));
            }
        }
        Type::Brush => {
            let brush_ref = env.create_reference(unknown.coerce_to_object()?)?;
            if let Some(js_brush) = env
                .get_reference_value::<JsObject>(&brush_ref)
                .ok()
                .and_then(|obj| obj.get("brush").ok().flatten())
                .and_then(|brush_prop| env.get_value_external::<Brush>(&brush_prop).ok())
            {
                return Ok(Value::Brush(js_brush.clone()));
            }

            if let Some(js_brush) = env
                .get_reference_value::<JsObject>(&brush_ref)
                .ok()
                .and_then(|js_object| js_object.coerce_to_string().ok())
                .and_then(|string| string_to_brush(string).ok())
            {
                return Ok(js_brush);
            } else {
                return Err(napi::Error::from_reason(
                            format!("Cannot convert object to brush, because the given object is neither a brush nor a string")
                    ));
            }
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
                            napi::Error::from_reason(format!(
                                "data property does not have suitable array buffer type"
                            ))
                        })?;
                    const BPP: usize = core::mem::size_of::<Rgba8Pixel>();
                    let actual_size = buffer.as_ref().len();
                    let expected_size: usize = (width as usize) * (height as usize) * BPP;
                    if actual_size != expected_size {
                        return Err(napi::Error::from_reason(format!(
                            "data property does not have the correct size; expected {} (width) * {} (height) * {} = {}; got {}",
                            width, height, BPP, actual_size, expected_size
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
        Type::Struct { fields, name: _, node: _, rust_attributes: _ } => {
            let js_object = unknown.coerce_to_object()?;

            Ok(Value::Struct(
                fields
                    .iter()
                    .map(|(pro_name, pro_ty)| {
                        Ok((
                            pro_name.clone(),
                            to_value(
                                env,
                                js_object.get_property(
                                    env.create_string(&pro_name.replace('-', "_"))?,
                                )?,
                                pro_ty.clone(),
                            )?,
                        ))
                    })
                    .collect::<Result<_, _>>()?,
            ))
        }
        Type::Array(a) => {
            if unknown.is_array()? {
                let array = Array::from_unknown(unknown)?;
                let mut vec = vec![];

                for i in 0..array.len() {
                    vec.push(to_value(env, array.get(i)?.unwrap(), *a.to_owned())?);
                }
                Ok(Value::Model(ModelRc::new(SharedVectorModel::from(SharedVector::from_slice(
                    &vec,
                )))))
            } else {
                let model = unknown.coerce_to_object()?;
                let _: JsFunction = model.get("rowCount")?.unwrap();
                let _: JsFunction = model.get("rowData")?.unwrap();

                Ok(Value::Model(ModelRc::new(JsModel::new(*env, model, *a.to_owned())?)))
            }
        }
        Type::Enumeration(_) => todo!(),
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
