use crate::{JsBrush, JsImageData, JsModel};
use i_slint_core::{graphics::Image, model::ModelRc, Brush};
use napi::{bindgen_prelude::*, Env, JsBoolean, JsExternal, JsNumber, JsString, JsUnknown, Result};
use slint_interpreter::{ComponentInstance, Struct, Value, ValueType};

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
        Value::String(string) => { println!("string");env.create_string(string).map(|v| v.into_unknown())},
        Value::Bool(value) => env.get_boolean(*value).map(|v| v.into_unknown()),
        Value::Image(image) => {
            Ok(JsImageData::from(image.clone()).into_instance(*env)?.as_object(*env).into_unknown())
        }
        //                        Model(model) => {} TODO: Try to create a Rust type that stores ModelRc<Value> and exposes it in a nice JS API (see Model<T> interface in api/node/lib/index.ts)
        Value::Struct(struct_value) => {
            let mut o = env.create_object()?;
            for (field_name, field_value) in struct_value.iter() {
                o.set_property(env.create_string(field_name)?, to_js_unknown(env, field_value)?)?;
            }
            Ok(o.into_unknown())
        }
        Value::Brush(brush) => {
            Ok(JsBrush::from(brush.clone()).into_instance(*env)?.as_object(*env).into_unknown())
        }
        Value::Model(model) => {
            Ok(JsModel::from(model.clone()).into_instance(*env)?.as_object(*env).into_unknown())
        }
        _ => env.get_undefined().map(|v| v.into_unknown()),
    }
}

pub fn to_value(
    env: &Env,
    instance: &ComponentInstance,
    unknown: JsUnknown,
    value: &Value,
) -> Result<Value> {
    Ok(match value.value_type() {
        ValueType::Void => Value::Void,
        ValueType::Number => {
            let js_number: JsNumber = unknown.try_into()?;
            Value::Number(js_number.get_double()?)
        }
        ValueType::String => {
            let js_string: JsString = unknown.try_into()?;
            Value::String(js_string.into_utf8()?.as_str()?.into())
        }
        ValueType::Bool => {
            let js_bool: JsBoolean = unknown.try_into()?;
            Value::Bool(js_bool.get_value()?)
        }
        ValueType::Image => {
            let js_image: JsExternal = unknown.coerce_to_object()?.get("image")?.unwrap();
            Value::Image(env.get_value_external::<Image>(&js_image)?.clone())
        }
        ValueType::Model => {
            let js_model: JsExternal = unknown.coerce_to_object()?.get("model")?.unwrap();
            Value::Model(env.get_value_external::<ModelRc<Value>>(&js_model)?.clone())
        }
        ValueType::Struct => {
            let js_object = unknown.coerce_to_object()?;
            let property_names = js_object.get_property_names()?;
            let property_len = property_names.get_array_length()?;
            let mut s_vec = vec![];

            if let Value::Struct(s) = value {
                for i in 0..property_len {
                    let key = property_names.get_element::<JsString>(i)?;
                    let key_string = key.into_utf8()?.into_owned()?;

                    if let Some(value) = s.get_field(&key_string) {
                        s_vec.push((
                            key_string,
                            to_value(env, instance, js_object.get_property(key)?, value)?,
                        ));
                    }
                }
            }

            Value::Struct(s_vec.iter().cloned().collect::<Struct>().into())
        }
        ValueType::Brush => {
            let js_brush: JsExternal = unknown.coerce_to_object()?.get("brush")?.unwrap();
            Value::Brush(env.get_value_external::<Brush>(&js_brush)?.clone())
        }
        _ => {
            todo!()
        }
    })
}

pub fn js_unknown_to_value(env: Env, unknown: JsUnknown) -> Result<Value> {
    match unknown.get_type()? {
        napi::ValueType::Boolean => Ok(Value::Bool(unknown.coerce_to_bool()?.get_value()?)),
        napi::ValueType::Number => Ok(Value::Number(unknown.coerce_to_number()?.get_double()?)),
        napi::ValueType::String => {
            Ok(Value::String(unknown.coerce_to_string()?.into_utf8()?.as_str()?.into()))
        }
        napi::ValueType::Object => {
            let js_object = unknown.coerce_to_object()?;
            let image: Option<JsExternal> = js_object.get("image")?;

            if let Some(image) = image {
                Ok(Value::Image(env.get_value_external::<Image>(&image)?.clone()))
            } else {
                let brush: Option<JsExternal> = js_object.get("brush")?;

                if let Some(brush) = brush {
                    Ok(Value::Brush(env.get_value_external::<Brush>(&brush)?.clone()))
                } else {
                    let property_names = js_object.get_property_names()?;
                    let mut slint_struct = slint_interpreter::Struct::default();

                    for i in 0..property_names.get_array_length()? {
                        let key = property_names.get_element::<JsString>(i)?;

                        slint_struct.set_field(
                            key.into_utf8()?.into_owned()?,
                            js_unknown_to_value(env, js_object.get_property(key)?)?,
                        );
                    }

                    Ok(Value::Struct(slint_struct))
                }
            }
        }
        _ => Ok(Value::Void),
    }
}
