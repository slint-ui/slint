use napi::{bindgen_prelude::*, JsBoolean, JsNumber, JsString};
use napi::{Env, JsUnknown, Result};
use slint_interpreter::{Value, ValueType};

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

// #[napi(js_name = "Value")]
// pub enum JsValue {
//     Void,
//     Number(f64),
//     String(SharedString),
//     Bool(bool),
//     Image(Image),
//     Model(ModelRc<Value>),
//     Struct(Struct),
//     Brush(Brush),
//     // some variants omitted
// }

pub fn to_js_unknown(env: &Env, value: &Value) -> Result<JsUnknown> {
    match value {
        Value::Void => env.get_null().map(|v| v.into_unknown()),
        Value::Number(number) => env.create_double(*number).map(|v| v.into_unknown()),
        Value::String(string) => env.create_string(string).map(|v| v.into_unknown()),
        Value::Bool(value) => env.get_boolean(*value).map(|v| v.into_unknown()),
        //                        Image(image) => {} TODO: https://github.com/slint-ui/slint/issues/2474 - return struct that has same properties/etc./shape as ImageData: https://developer.mozilla.org/en-US/docs/Web/API/ImageData
        //                        Model(model) => {} TODO: Try to create a Rust type that stores ModelRc<Value> and exposes it in a nice JS API (see Model<T> interface in api/node/lib/index.ts)
        Value::Struct(struct_value) => {
            let mut o = env.create_object()?;
            for (field_name, field_value) in struct_value.iter() {
                o.set_property(env.create_string(field_name)?, to_js_unknown(env, field_value)?)?;
            }
            Ok(o.into_unknown())
        }
        //                      Brush(brush) => {}
        _ => env.get_undefined().map(|v| v.into_unknown()),
    }
}

pub fn to_value(_env: &Env, unknown: JsUnknown, type_hint: ValueType) -> Result<Value> {
    Ok(match type_hint {
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
            todo!("Pretend unknown is a JS object that has ImageData properties; read them and try to create slint::Image")
        }
        ValueType::Model => {
            todo!("Instantiate a Rust type that implements Model<Value>, stores JsUnknown as JsObject and treats it as if it implements the Model<T> interface")
        }
        ValueType::Struct => {
            todo!("Use private interpreter API to find out what fields are expected; Then create slint_interpreter::Struct")
        }
        ValueType::Brush => {
            todo!()
        }
        _ => {
            todo!()
        }
    })
}
