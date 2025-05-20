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
                let s = if let Some(suffix) = s.strip_prefix(&format!("{}.", e.name)) {
                    suffix.to_smolstr()
                } else {
                    s.to_smolstr()
                };

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
                fn string_to_brush(input: &str) -> Result<i_slint_core::graphics::Brush, String> {
                    fn parse_stops<'a>(
                        it: impl Iterator<Item = &'a str>,
                    ) -> Result<Vec<i_slint_core::graphics::GradientStop>, String>
                    {
                        it.filter(|part| !part.is_empty()).map(|part| {
                            let sub_parts = part.split_whitespace().collect::<Vec<_>>();
                            if sub_parts.len() != 2 {
                                Err("A gradient stop must consist of a color and a position in '%' separated by whitespace".into())
                            } else {
                                let color = string_to_color(sub_parts[0]);
                                let position = {
                                    if let Some(percent_value) = sub_parts[1].strip_suffix("%") {
                                        percent_value.parse::<f32>().map_err(|_| format!("Could not parse position '{}' as number", sub_parts[1]))
                                    } else {
                                        Err(format!("The position '{}' does not end in '%'", sub_parts[1]))
                                    }
                                };

                                match (color, position) {
                                    (Some(c), Ok(p)) => Ok(i_slint_core::graphics::GradientStop { color: c, position: p / 100.0}),
                                    (_, Err(e)) => Err(e),
                                    (None, _) => Err(format!("'{}' is not a color", sub_parts[0])),
                                }
                            }
                        }).collect()
                    }

                    let Some(input) = input.strip_suffix(')') else {
                        return Err(format!("No closing ')' in '{input}'"));
                    };

                    if let Some(linear) = input.strip_prefix("@linear-gradient(") {
                        let mut split = linear.split(',').map(|p| p.trim());

                        let angle = {
                            let Some(angle_part) = split.next() else {
                                return Err(
                                    "A linear gradient must start with an angle in 'deg'".into()
                                );
                            };

                            angle_part
                                .strip_suffix("deg")
                                .ok_or_else(|| {
                                    "A linear brush needs to start with an angle in 'deg'"
                                        .to_string()
                                })
                                .and_then(|no| {
                                    no.parse::<f32>()
                                        .map_err(|_| "Failed to parse angle value".into())
                                })
                        }?;

                        Ok(i_slint_core::graphics::LinearGradientBrush::new(
                            angle,
                            parse_stops(split)?.drain(..),
                        )
                        .into())
                    } else if let Some(radial) = input.strip_prefix("@radial-gradient(circle") {
                        let split = radial.split(',').map(|p| p.trim());

                        Ok(i_slint_core::graphics::RadialGradientBrush::new_circle(
                            parse_stops(split)?.drain(..),
                        )
                        .into())
                    } else {
                        Err(format!("Could not parse gradient from '{input}'"))
                    }
                }

                if s.starts_with('#') {
                    if let Some(c) = string_to_color(s) {
                        Ok(Value::Brush(i_slint_core::Brush::SolidColor(c)))
                    } else {
                        Err(format!("Failed to parse color value {s}"))
                    }
                } else {
                    Ok(Value::Brush(string_to_brush(s)?))
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
                        let k = crate::api::normalize_identifier(k);
                        match s.fields.get(&k) {
                            Some(t) => value_from_json(t, v).map(|v| (k, v)),
                            None => Err(format!("Found unknown field in struct: {k}")),
                        }
                    })
                    .collect::<Result<HashMap<smol_str::SmolStr, Value>, _>>()?,
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

    fn gradient_to_string_helper<'a>(
        prefix: String,
        stops: impl Iterator<Item = &'a i_slint_core::graphics::GradientStop>,
    ) -> serde_json::Value {
        let mut gradient = prefix;

        for stop in stops {
            gradient += &format!(", {} {}%", color_to_string(&stop.color), stop.position * 100.0);
        }

        gradient += ")";

        serde_json::Value::String(gradient)
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
            Brush::LinearGradient(lg) => Ok(gradient_to_string_helper(
                format!("@linear-gradient({}deg", lg.angle()),
                lg.stops(),
            )),
            Brush::RadialGradient(rg) => {
                Ok(gradient_to_string_helper("@radial-gradient(circle".into(), rg.stops()))
            }
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
    assert_eq!(v, Value::Brush(Brush::SolidColor(Color::from_argb_u8(0xff, 0x0a, 0xb0, 0xcd))));
    let v = value_from_json_str(
        &langtype::Type::Brush,
        "\"@linear-gradient(42deg, #ff0000ff 0%, #00ff00ff 50%, #0000ffff 100%)\"",
    )
    .unwrap();
    assert_eq!(
        v,
        Value::Brush(Brush::LinearGradient(i_slint_core::graphics::LinearGradientBrush::new(
            42.0,
            vec![
                i_slint_core::graphics::GradientStop {
                    position: 0.0,
                    color: Color::from_argb_u8(0xff, 0xff, 0x00, 0x00)
                },
                i_slint_core::graphics::GradientStop {
                    position: 0.5,
                    color: Color::from_argb_u8(0xff, 0x00, 0xff, 0x00)
                },
                i_slint_core::graphics::GradientStop {
                    position: 1.0,
                    color: Color::from_argb_u8(0xff, 0x00, 0x00, 0xff)
                }
            ]
            .drain(..)
        )))
    );
    assert!(value_from_json_str(
        &langtype::Type::Brush,
        "\"@linear-gradient(foobar, #ff0000ff 0%, #00ff00ff 50%, #0000ffff 100%)\""
    )
    .is_err());
    assert!(value_from_json_str(
        &langtype::Type::Brush,
        "\"@linear-gradient(#ff0000ff 0%, #00ff00ff 50%, #0000ffff 100%)\""
    )
    .is_err());
    assert!(value_from_json_str(
        &langtype::Type::Brush,
        "\"@linear-gradient(90turns, #ff0000ff 0%, #00ff00ff 50%, #0000ffff 100%)\""
    )
    .is_err());
    assert!(value_from_json_str(
        &langtype::Type::Brush,
        "\"@linear-gradient(xfdeg, #ff0000ff 0%, #00ff00ff 50%, #0000ffff 100%)\""
    )
    .is_err());
    assert!(value_from_json_str(
        &langtype::Type::Brush,
        "\"@linear-gradient(90deg, #xf0000ff 0%, #00ff00ff 50%, #0000ffff 100%)\""
    )
    .is_err());
    assert!(value_from_json_str(
        &langtype::Type::Brush,
        "\"@linear-gradient(90deg, #ff0000ff 0, #00ff00ff 50%, #0000ffff 100%)\""
    )
    .is_err());

    let v = value_from_json_str(
        &langtype::Type::Brush,
        "\"@radial-gradient(circle, #ff0000ff 0%, #00ff00ff 50%, #0000ffff 100%)\"",
    )
    .unwrap();
    assert_eq!(
        v,
        Value::Brush(Brush::RadialGradient(
            i_slint_core::graphics::RadialGradientBrush::new_circle(
                vec![
                    i_slint_core::graphics::GradientStop {
                        position: 0.0,
                        color: Color::from_argb_u8(0xff, 0xff, 0x00, 0x00)
                    },
                    i_slint_core::graphics::GradientStop {
                        position: 0.5,
                        color: Color::from_argb_u8(0xff, 0x00, 0xff, 0x00)
                    },
                    i_slint_core::graphics::GradientStop {
                        position: 1.0,
                        color: Color::from_argb_u8(0xff, 0x00, 0x00, 0xff)
                    }
                ]
                .drain(..)
            )
        ))
    );
    assert!(value_from_json_str(
        &langtype::Type::Brush,
        "\"@radial-gradient(foobar, #ff0000ff 0%, #00ff00ff 50%, #0000ffff 100%)\""
    )
    .is_err());
    assert!(value_from_json_str(
        &langtype::Type::Brush,
        "\"@radial-gradient(circle, #xf0000ff 0%, #00ff00ff 50%, #0000ffff 100%)\""
    )
    .is_err());
    assert!(value_from_json_str(
        &langtype::Type::Brush,
        "\"@radial-gradient(circle, #ff0000ff 1000px, #00ff00ff 50%, #0000ffff 100%)\""
    )
    .is_err());
    assert!(value_from_json_str(
        &langtype::Type::Brush,
        "\"@radial-gradient(circle, #ff0000ff 0% #00ff00ff 50%, #0000ffff 100%)\""
    )
    .is_err());
    assert!(value_from_json_str(
        &langtype::Type::Brush,
        "\"@radial-gradient(circle, #ff0000ff, #0000ffff)\""
    )
    .is_err());

    assert!(value_from_json_str(
        &langtype::Type::Brush,
        "\"@radial-gradient(conical, #ff0000ff 0%, #00ff00ff 50%, #0000ffff 100%)\""
    )
    .is_err());

    assert!(value_from_json_str(
        &langtype::Type::Brush,
        "\"@other-gradient(circle, #ff0000ff 0%, #00ff00ff 50%, #0000ffff 100%)\""
    )
    .is_err());
}

#[test]
fn test_to_json() {
    let v = value_to_json_string(&Value::Void).unwrap();
    assert_eq!(&v, "null");
    let v = Value::Void.to_json_string().unwrap();
    assert_eq!(&v, "null");

    let v = value_to_json_string(&Value::Number(23.0)).unwrap();
    assert_eq!(&v, "23");

    let v = value_to_json_string(&Value::Number(4.2)).unwrap();
    assert_eq!(&v, "4.2");

    let v = value_to_json_string(&Value::EnumerationValue("Foo".to_string(), "bar".to_string()))
        .unwrap();
    assert_eq!(&v, "\"Foo.bar\"");

    let v = value_to_json_string(&Value::String("Hello World with \\ escaped".into())).unwrap();
    assert_eq!(&v, "\"Hello World with \\\\ escaped\"");

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
    assert_eq!(&v, "true");

    let v = value_to_json_string(&Value::Bool(false)).unwrap();
    assert_eq!(&v, "false");

    let model: ModelRc<Value> = std::rc::Rc::new(i_slint_core::model::VecModel::from(vec![
        Value::Bool(true),
        Value::Bool(false),
    ]))
    .into();
    let v = value_to_json_string(&Value::Model(model)).unwrap();
    assert_eq!(&v, "[true,false]");

    let v = value_to_json_string(&Value::Struct(crate::Struct::from_iter([
        ("kind".to_string(), Value::EnumerationValue("test".to_string(), "foo".to_string())),
        ("is_bool".to_string(), Value::Bool(false)),
        ("string-value".to_string(), Value::String("some string".into())),
    ])))
    .unwrap();
    assert_eq!(&v, "{\"is-bool\":false,\"kind\":\"test.foo\",\"string-value\":\"some string\"}");

    let v = value_to_json_string(&Value::Brush(Brush::SolidColor(Color::from_argb_u8(
        0xff, 0x0a, 0xb0, 0xcd,
    ))))
    .unwrap();
    assert_eq!(v, "\"#0ab0cdff\"".to_string());

    let v = value_to_json_string(&Value::Brush(Brush::LinearGradient(
        i_slint_core::graphics::LinearGradientBrush::new(
            42.0,
            vec![
                i_slint_core::graphics::GradientStop {
                    position: 0.0,
                    color: Color::from_argb_u8(0xff, 0xff, 0x00, 0x00),
                },
                i_slint_core::graphics::GradientStop {
                    position: 0.5,
                    color: Color::from_argb_u8(0xff, 0x00, 0xff, 0x00),
                },
                i_slint_core::graphics::GradientStop {
                    position: 1.0,
                    color: Color::from_argb_u8(0xff, 0x00, 0x00, 0xff),
                },
            ]
            .drain(..),
        ),
    )))
    .unwrap();
    assert_eq!(&v, "\"@linear-gradient(42deg, #ff0000ff 0%, #00ff00ff 50%, #0000ffff 100%)\"");

    let v = value_to_json_string(&Value::Brush(Brush::RadialGradient(
        i_slint_core::graphics::RadialGradientBrush::new_circle(
            vec![
                i_slint_core::graphics::GradientStop {
                    position: 0.0,
                    color: Color::from_argb_u8(0xff, 0xff, 0x00, 0x00),
                },
                i_slint_core::graphics::GradientStop {
                    position: 0.5,
                    color: Color::from_argb_u8(0xff, 0x00, 0xff, 0x00),
                },
                i_slint_core::graphics::GradientStop {
                    position: 1.0,
                    color: Color::from_argb_u8(0xff, 0x00, 0x00, 0xff),
                },
            ]
            .drain(..),
        ),
    )))
    .unwrap();
    assert_eq!(&v, "\"@radial-gradient(circle, #ff0000ff 0%, #00ff00ff 50%, #0000ffff 100%)\"");
}
