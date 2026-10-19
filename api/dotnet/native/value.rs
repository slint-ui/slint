// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_core::model::{Model, ModelRc, VecModel};
use slint_interpreter::{Brush, Color, Image, Value, ValueType};

use crate::{NameList, OwnedString, Utf8Str, guard, guard_bool, into_raw, release};

/// Matches `Slint.ValueType` in C#.
pub fn value_type_tag(ty: ValueType) -> i32 {
    match ty {
        ValueType::Void => 0,
        ValueType::Number => 1,
        ValueType::String => 2,
        ValueType::Bool => 3,
        ValueType::Model => 4,
        ValueType::Struct => 5,
        ValueType::Brush => 6,
        ValueType::Image => 7,
        _ => -1,
    }
}

const KIND_ENUM: i32 = 8;

/// The `ValueType` tag of `value`, or 8 for an enum value.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_value_kind(value: &Value) -> i32 {
    match value {
        Value::EnumerationValue(..) => KIND_ENUM,
        _ => value_type_tag(value.value_type()),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_value_free(value: *mut Value) {
    unsafe { release(value) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_value_clone(value: &Value) -> *mut Value {
    guard(std::ptr::null_mut(), || Ok(into_raw(value.clone())))
}

#[unsafe(no_mangle)]
pub extern "C" fn slint_dotnet_value_new_void() -> *mut Value {
    guard(std::ptr::null_mut(), || Ok(into_raw(Value::Void)))
}

#[unsafe(no_mangle)]
pub extern "C" fn slint_dotnet_value_new_number(number: f64) -> *mut Value {
    guard(std::ptr::null_mut(), || Ok(into_raw(Value::Number(number))))
}

#[unsafe(no_mangle)]
pub extern "C" fn slint_dotnet_value_new_bool(b: bool) -> *mut Value {
    guard(std::ptr::null_mut(), || Ok(into_raw(Value::Bool(b))))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_value_new_string(s: Utf8Str) -> *mut Value {
    guard(std::ptr::null_mut(), || Ok(into_raw(Value::String(unsafe { s.as_str() }?.into()))))
}

/// `argb` is encoded as `0xAARRGGBB`.
#[unsafe(no_mangle)]
pub extern "C" fn slint_dotnet_value_new_color(argb: u32) -> *mut Value {
    guard(std::ptr::null_mut(), || {
        Ok(into_raw(Value::Brush(Brush::SolidColor(Color::from_argb_encoded(argb)))))
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_value_new_enum(
    enum_name: Utf8Str,
    value: Utf8Str,
) -> *mut Value {
    guard(std::ptr::null_mut(), || {
        let (enum_name, value) = unsafe { (enum_name.as_str()?, value.as_str()?) };
        Ok(into_raw(Value::EnumerationValue(enum_name.into(), value.into())))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn slint_dotnet_value_new_struct() -> *mut Value {
    guard(std::ptr::null_mut(), || Ok(into_raw(Value::Struct(Default::default()))))
}

/// Takes ownership of every value in `values`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_value_new_model(
    values: *const *mut Value,
    count: usize,
) -> *mut Value {
    let rows = if count == 0 {
        Vec::new()
    } else {
        unsafe { std::slice::from_raw_parts(values, count) }
            .iter()
            .map(|v| *unsafe { Box::from_raw(*v) })
            .collect()
    };
    guard(std::ptr::null_mut(), || Ok(into_raw(Value::Model(ModelRc::new(VecModel::from(rows))))))
}

#[unsafe(no_mangle)]
pub extern "C" fn slint_dotnet_value_new_image() -> *mut Value {
    guard(std::ptr::null_mut(), || Ok(into_raw(Value::Image(Image::default()))))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_value_new_image_from_path(path: Utf8Str) -> *mut Value {
    guard(std::ptr::null_mut(), || {
        let path = unsafe { path.as_str() }?;
        Image::load_from_path(std::path::Path::new(path))
            .map(|image| into_raw(Value::Image(image)))
            .map_err(|_| format!("Failed to load the image {path}"))
    })
}

/// Returns no string if `value` isn't an image loaded from a file.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_value_image_path(value: &Value) -> OwnedString {
    match value {
        Value::Image(image) => {
            image.path().map_or_else(OwnedString::none, |p| p.to_string_lossy().into_owned().into())
        }
        _ => OwnedString::none(),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_value_image_size(
    value: &Value,
    width: &mut u32,
    height: &mut u32,
) -> bool {
    match value {
        Value::Image(image) => {
            let size = image.size();
            (*width, *height) = (size.width, size.height);
            true
        }
        _ => false,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_value_to_number(value: &Value, out: &mut f64) -> bool {
    match value {
        Value::Number(n) => {
            *out = *n;
            true
        }
        _ => false,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_value_to_bool(value: &Value, out: &mut bool) -> bool {
    match value {
        Value::Bool(b) => {
            *out = *b;
            true
        }
        _ => false,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_value_to_string(value: &Value) -> OwnedString {
    match value {
        Value::String(s) => s.to_string().into(),
        _ => OwnedString::none(),
    }
}

/// Only succeeds for a solid color brush.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_value_to_color(value: &Value, out: &mut u32) -> bool {
    match value {
        Value::Brush(Brush::SolidColor(c)) => {
            *out = c.as_argb_encoded();
            true
        }
        _ => false,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_value_enum_name(value: &Value) -> OwnedString {
    match value {
        Value::EnumerationValue(name, _) => name.clone().into(),
        _ => OwnedString::none(),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_value_enum_value(value: &Value) -> OwnedString {
    match value {
        Value::EnumerationValue(_, v) => v.clone().into(),
        _ => OwnedString::none(),
    }
}

/// Returns -1 if `value` isn't a model.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_value_model_row_count(value: &Value) -> isize {
    guard(-1, || match value {
        Value::Model(m) => Ok(m.row_count() as isize),
        _ => Ok(-1),
    })
}

/// Returns null if `value` isn't a model or `row` is out of range.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_value_model_row_data(
    value: &Value,
    row: usize,
) -> *mut Value {
    guard(std::ptr::null_mut(), || match value {
        Value::Model(m) => Ok(m.row_data(row).map_or(std::ptr::null_mut(), into_raw)),
        _ => Ok(std::ptr::null_mut()),
    })
}

/// Returns null if `value` isn't a struct.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_value_struct_field_names(value: &Value) -> *mut NameList {
    match value {
        Value::Struct(s) => Box::into_raw(NameList::untagged(s.iter().map(|(n, _)| n.to_string()))),
        _ => std::ptr::null_mut(),
    }
}

/// Returns null if `value` isn't a struct or has no field `name`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_value_struct_get_field(
    value: &Value,
    name: Utf8Str,
) -> *mut Value {
    guard(std::ptr::null_mut(), || match value {
        Value::Struct(s) => Ok(s
            .get_field(unsafe { name.as_str() }?)
            .cloned()
            .map_or(std::ptr::null_mut(), into_raw)),
        _ => Ok(std::ptr::null_mut()),
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_value_struct_set_field(
    value: &mut Value,
    name: Utf8Str,
    field: &Value,
) -> bool {
    guard_bool(|| match value {
        Value::Struct(s) => {
            s.set_field(unsafe { name.as_str() }?.into(), field.clone());
            Ok(())
        }
        _ => Err("value isn't a struct".into()),
    })
}
