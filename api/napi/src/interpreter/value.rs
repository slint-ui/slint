use napi::bindgen_prelude::{FromNapiValue, ToNapiValue};

#[napi(js_name = "ValueType")]
pub enum JsValueType {
    Void,
    Number,
    String,
    Bool,
    Model,
    Struct,
    Brush,
    Image
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
    pub value_type: JsValueType
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