// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! This module contains the code serialize and desrialize `Value`s to JSON

use std::collections::HashMap;

use i_slint_compiler::langtype;
use i_slint_core::{
    graphics::Image,
    model::{Model, ModelRc},
    Brush, Color, SharedString, SharedVector,
};

use crate::Value;

/// Extension trait, adding JSON serialization methods
pub trait JsonExt
where
    Self: Sized,
{
    /// Convert to a JSON object
    fn to_json(&self) -> Result<serde_json::Value, String>;
    /// Convert to a JSON-encoded string
    fn to_json_string(&self) -> Result<String, String>;
    /// Convert to JSON object to `Self`
    fn from_json(t: &langtype::Type, value: &serde_json::Value) -> Result<Self, String>;
    /// Convert to JSON encoded string to `Self`
    fn from_json_str(t: &langtype::Type, value: &str) -> Result<Self, String>;
}

impl JsonExt for crate::Value {
    fn to_json(&self) -> Result<serde_json::Value, String> {
        value_to_json(self)
    }

    fn to_json_string(&self) -> Result<String, String> {
        value_to_json_string(self)
    }

    fn from_json(t: &langtype::Type, value: &serde_json::Value) -> Result<Self, String> {
        value_from_json(t, value)
    }

    fn from_json_str(t: &langtype::Type, value: &str) -> Result<Self, String> {
        value_from_json_str(t, value)
    }
}

/// Create a `Value` from a JSON Value
pub fn value_from_json(t: &langtype::Type, v: &serde_json::Value) -> Result<Value, String> {
    use smol_str::ToSmolStr;

    fn string_to_color(s: &str) -> Option<i_slint_core::Color> {
        i_slint_compiler::literals::parse_color_literal(s).map(Color::from_argb_encoded)
    }

    match v {
        serde_json::Value::Null => Ok(Value::Void),
        serde_json::Value::Bool(b) => Ok((*b).into()),
        serde_json::Value::Number(n) => Ok(Value::Number(n.as_f64().unwrap_or(f64::NAN))),
        serde_json::Value::String(s) => match t {
            langtype::Type::Enumeration(e) => {
                let s = s.to_smolstr();
                if e.values.contains(&s) {
                    Ok(Value::EnumerationValue(e.name.to_string(), s.into()))
                } else {
                    Err(format!("Unexpected value for enum '{}': {}", e.name, s))
                }
            }
            langtype::Type::Color => {
                if let Some(c) = string_to_color(s) {
                    Ok(Value::Brush(i_slint_core::Brush::SolidColor(c)))
                } else {
                    Err(format!("Failed to parse color: {s}"))
                }
            }
            langtype::Type::String => Ok(SharedString::from(s.as_str()).into()),
            langtype::Type::Image => match Image::load_from_path(std::path::Path::new(s)) {
                Ok(image) => Ok(image.into()),
                Err(e) => Err(format!("Failed to load image from path: {s}: {e}")),
            },
            langtype::Type::Brush => {
                if s.starts_with('#') {
                    if let Some(c) = string_to_color(s) {
                        Ok(Value::Brush(i_slint_core::Brush::SolidColor(c)))
                    } else {
                        Err(format!("Failed to parse color value {s}"))
                    }
                } else {
                    Err("Brush kind not supported".into())
                }
            }
            _ => Err("Value type not supported".into()),
        },
        serde_json::Value::Array(array) => match t {
            langtype::Type::Array(it) => {
                Ok(Value::Model(ModelRc::new(i_slint_core::model::SharedVectorModel::from(
                    array
                        .iter()
                        .map(|v| value_from_json(it, v))
                        .collect::<Result<SharedVector<Value>, String>>()?,
                ))))
            }
            _ => Err("Got an array where none was expected".into()),
        },
        serde_json::Value::Object(obj) => match t {
            langtype::Type::Struct(s) => Ok(crate::Struct(
                obj.iter()
                    .map(|(k, v)| {
                        let k = k.to_smolstr();
                        match s.fields.get(&k) {
                            Some(t) => value_from_json(t, v).map(|v| (k.to_string(), v)),
                            None => Err(format!("Found unknown field in struct: {k}")),
                        }
                    })
                    .collect::<Result<HashMap<String, Value>, _>>()?,
            )
            .into()),
            _ => Err("Got a struct where none was expected".into()),
        },
    }
}

/// Create a `Value` from a JSON string
pub fn value_from_json_str(t: &langtype::Type, v: &str) -> Result<Value, String> {
    let value = serde_json::from_str(v).map_err(|e| format!("Failed to parse JSON: {e}"))?;
    Value::from_json(t, &value)
}

/// Write the `Value` out into a JSON value
pub fn value_to_json(value: &Value) -> Result<serde_json::Value, String> {
    fn color_to_string(color: &Color) -> String {
        let a = color.alpha();
        let r = color.red();
        let g = color.green();
        let b = color.blue();

        format!("#{r:02x}{g:02x}{b:02x}{a:02x}")
    }

    match value {
        Value::Void => Ok(serde_json::Value::Null),
        Value::Bool(b) => Ok((*b).into()),
        Value::Number(n) => {
            let r = if *n == n.round() {
                if *n >= 0.0 {
                    serde_json::Number::from_u128(*n as u128)
                } else {
                    serde_json::Number::from_i128(*n as i128)
                }
            } else {
                serde_json::Number::from_f64(*n)
            };
            if let Some(r) = r {
                Ok(serde_json::Value::Number(r))
            } else {
                Err(format!("Could not convert {n} into a number"))
            }
        }
        Value::EnumerationValue(e, v) => Ok(serde_json::Value::String(format!("{e}.{v}"))),
        Value::String(shared_string) => Ok(serde_json::Value::String(shared_string.to_string())),
        Value::Image(image) => {
            if let Some(p) = image.path() {
                Ok(serde_json::Value::String(format!("{}", p.to_string_lossy())))
            } else {
                Err("Cannot serialize an image without a path".into())
            }
        }
        Value::Model(model_rc) => Ok(serde_json::Value::Array(
            model_rc.iter().map(|v| v.to_json()).collect::<Result<Vec<_>, _>>()?,
        )),
        Value::Struct(s) => Ok(serde_json::Value::Object(
            s.iter()
                .map(|(k, v)| v.to_json().map(|v| (k.to_string(), v)))
                .collect::<Result<serde_json::Map<_, _>, _>>()?,
        )),
        Value::Brush(brush) => match brush {
            Brush::SolidColor(color) => Ok(serde_json::Value::String(color_to_string(color))),
            Brush::LinearGradient(_) => Err("Cannot serialize a linear gradient".into()),
            Brush::RadialGradient(_) => Err("Cannot serialize a radial gradient".into()),
            _ => Err("Cannot serialize an unknown brush type".into()),
        },
        Value::PathData(_) => Err("Cannot serialize path data".into()),
        Value::EasingCurve(_) => Err("Cannot serialize a easing curve".into()),
        _ => Err("Cannot serialize an unknown value type".into()),
    }
}

/// Write the `Value` out into a JSON string
pub fn value_to_json_string(value: &Value) -> Result<String, String> {
    Ok(value_to_json(value)?.to_string())
}

#[test]
fn test_from_json() {
    let v = value_from_json_str(&langtype::Type::Void, "null").unwrap();
    assert_eq!(v, Value::Void);
    let v = Value::from_json_str(&langtype::Type::Void, "null").unwrap();
    assert_eq!(v, Value::Void);

    let v = value_from_json_str(&langtype::Type::Float32, "42.0").unwrap();
    assert_eq!(v, Value::Number(42.0));

    let v = value_from_json_str(&langtype::Type::Int32, "23").unwrap();
    assert_eq!(v, Value::Number(23.0));

    let v = value_from_json_str(&langtype::Type::String, "\"a string with \\\\ escape\"").unwrap();
    assert_eq!(v, Value::String("a string with \\ escape".into()));

    let v = value_from_json_str(&langtype::Type::Color, "\"#0ab0cdff\"").unwrap();
    assert_eq!(v, Value::Brush(Brush::SolidColor(Color::from_argb_u8(0xff, 0x0a, 0xb0, 0xcd))));
    let v = value_from_json_str(&langtype::Type::Brush, "\"#0ab0cdff\"").unwrap();
    assert_eq!(v, Value::Brush(Brush::SolidColor(Color::from_argb_u8(0xff, 0x0a, 0xb0, 0xcd))));
}

#[test]
fn test_to_json() {
    let v = value_to_json_string(&Value::Void).unwrap();
    assert_eq!(v, "null".to_string());
    let v = Value::Void.to_json_string().unwrap();
    assert_eq!(v, "null".to_string());

    let v = value_to_json_string(&Value::Number(23.0)).unwrap();
    assert_eq!(v, "23".to_string());

    let v = value_to_json_string(&Value::Number(4.2)).unwrap();
    assert_eq!(v, "4.2".to_string());

    let v = value_to_json_string(&Value::EnumerationValue("Foo".to_string(), "bar".to_string()))
        .unwrap();
    assert_eq!(v, "\"Foo.bar\"".to_string());

    let v = value_to_json_string(&Value::String("Hello World with \\ escaped".into())).unwrap();
    assert_eq!(v, "\"Hello World with \\\\ escaped\"".to_string());

    // Image without path:
    let buffer = i_slint_core::graphics::SharedPixelBuffer::new(2, 2);
    assert!(value_to_json_string(&Value::Image(Image::from_rgb8(buffer))).is_err());

    // Image with path
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../logo/MadeWithSlint-logo-dark.png")
        .canonicalize()
        .unwrap();
    let v = value_to_json_string(&Value::Image(Image::load_from_path(&path).unwrap())).unwrap();
    // We are looking at the JSON string which needs to be escaped!
    let path = path.to_string_lossy().replace("\\", "\\\\");
    assert_eq!(v, format!("\"{path}\""));

    let v = value_to_json_string(&Value::Bool(true)).unwrap();
    assert_eq!(v, "true".to_string());

    let v = value_to_json_string(&Value::Bool(false)).unwrap();
    assert_eq!(v, "false".to_string());

    let model: ModelRc<Value> = std::rc::Rc::new(i_slint_core::model::VecModel::from(vec![
        Value::Bool(true),
        Value::Bool(false),
    ]))
    .into();
    let v = value_to_json_string(&Value::Model(model)).unwrap();
    assert_eq!(v, "[true,false]".to_string());

    let v = value_to_json_string(&Value::Struct(crate::Struct::from_iter([
        ("kind".to_string(), Value::EnumerationValue("test".to_string(), "foo".to_string())),
        ("is_bool".to_string(), Value::Bool(false)),
        ("string-value".to_string(), Value::String("some string".into())),
    ])))
    .unwrap();
    assert_eq!(
        v,
        "{\"is-bool\":false,\"kind\":\"test.foo\",\"string-value\":\"some string\"}".to_string()
    );

    let v = value_to_json_string(&Value::Brush(Brush::SolidColor(Color::from_argb_u8(
        0xff, 0x0a, 0xb0, 0xcd,
    ))))
    .unwrap();
    assert_eq!(v, "\"#0ab0cdff\"".to_string());
}
