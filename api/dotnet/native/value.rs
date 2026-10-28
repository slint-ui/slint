// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_core::model::{Model, ModelRc, VecModel};
use slint_interpreter::{Brush, Color, Image, Value, ValueType};

use crate::{NameList, OwnedString, Utf8Str, guard, guard_bool, into_raw, release, utf8_slice};

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

/// The `ValueType` tag of `value`, or for values that have none:
/// 8 an enum value, 9 `keys`, 10 `styled-text`, 11 `data-transfer`. Matches `ValueConverter` in C#.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_value_kind(value: &Value) -> i32 {
    match value {
        Value::EnumerationValue(..) => 8,
        Value::Keys(_) => 9,
        Value::StyledText(_) => 10,
        Value::DataTransfer(_) => 11,
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

/// A struct with a field per name in `names`, in one call per struct.
/// Takes ownership of every value in `values`; a null value stands for the number at the same index of `numbers`,
/// so that numeric fields need no value of their own.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_value_new_struct(
    names: *const Utf8Str,
    values: *const *mut Value,
    numbers: *const f64,
    count: usize,
) -> *mut Value {
    let fields: Vec<Value> = if count == 0 {
        Vec::new()
    } else {
        let values = unsafe { std::slice::from_raw_parts(values, count) };
        let numbers = unsafe { std::slice::from_raw_parts(numbers, count) };
        values
            .iter()
            .zip(numbers)
            .map(
                |(v, n)| {
                    if v.is_null() { Value::Number(*n) } else { *unsafe { Box::from_raw(*v) } }
                },
            )
            .collect()
    };
    guard(std::ptr::null_mut(), || {
        let names = unsafe { utf8_slice(names, count) }?;
        Ok(into_raw(Value::Struct(names.into_iter().map(String::from).zip(fields).collect())))
    })
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

/// `pixels` has `width * height` pixels of 4 bytes, RGBA, or with `alpha` false 3 bytes, RGB.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_value_new_image_from_pixels(
    pixels: *const u8,
    len: usize,
    width: u32,
    height: u32,
    alpha: bool,
) -> *mut Value {
    use i_slint_core::graphics::{Rgb8Pixel, Rgba8Pixel, SharedPixelBuffer};
    guard(std::ptr::null_mut(), || {
        let bytes_per_pixel = if alpha { 4 } else { 3 };
        if len != width as usize * height as usize * bytes_per_pixel {
            return Err(format!(
                "{len} bytes don't make {width}×{height} pixels of {bytes_per_pixel} bytes"
            ));
        }
        let pixels =
            if len == 0 { &[][..] } else { unsafe { std::slice::from_raw_parts(pixels, len) } };
        let image = if alpha {
            Image::from_rgba8(SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
                pixels, width, height,
            ))
        } else {
            Image::from_rgb8(SharedPixelBuffer::<Rgb8Pixel>::clone_from_slice(
                pixels, width, height,
            ))
        };
        Ok(into_raw(Value::Image(image)))
    })
}

/// An image of an OpenGL texture of the window's context, which stays owned by the caller;
/// `bottom_left` flips it for a texture whose origin is at the bottom left, as OpenGL draws by default.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_value_new_image_from_gl_texture(
    texture_id: u32,
    width: u32,
    height: u32,
    bottom_left: bool,
) -> *mut Value {
    use i_slint_core::graphics::{
        BorrowedOpenGLTextureBuilder, BorrowedOpenGLTextureOrigin, IntSize,
    };
    guard(std::ptr::null_mut(), || {
        let texture_id =
            std::num::NonZeroU32::new(texture_id).ok_or("0 isn't an OpenGL texture")?;
        let origin = if bottom_left {
            BorrowedOpenGLTextureOrigin::BottomLeft
        } else {
            BorrowedOpenGLTextureOrigin::TopLeft
        };
        let image = unsafe {
            BorrowedOpenGLTextureBuilder::new_gl_2d_rgba_texture(
                texture_id,
                IntSize::new(width, height),
            )
        }
        .origin(origin)
        .build();
        Ok(into_raw(Value::Image(image)))
    })
}

/// Decodes `data`, such as a PNG, JPEG, or SVG file; an empty `format` guesses it from the data.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_value_new_image_from_data(
    data: *const u8,
    len: usize,
    format: Utf8Str,
) -> *mut Value {
    guard(std::ptr::null_mut(), || {
        let data =
            if len == 0 { &[][..] } else { unsafe { std::slice::from_raw_parts(data, len) } };
        let format = unsafe { format.as_str() }?;
        Image::load_from_data(data, (!format.is_empty()).then_some(format))
            .map(|image| into_raw(Value::Image(image)))
            .map_err(|_| "Failed to decode the image".to_string())
    })
}

/// Copies the pixels of an image as RGBA into `out`, which has `width * height * 4` bytes.
/// Fails for an image without pixels in memory, such as one that the GPU renders.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_value_image_to_rgba8(
    value: &Value,
    out: *mut u8,
    len: usize,
) -> bool {
    guard_bool(|| {
        let Value::Image(image) = value else { return Err("value isn't an image".into()) };
        let buffer = image.to_rgba8().ok_or("the image has no pixels to read")?;
        let bytes = buffer.as_bytes();
        if bytes.len() != len {
            return Err(format!("the image has {} bytes of pixels, not {len}", bytes.len()));
        }
        if len > 0 {
            unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), out, len) };
        }
        Ok(())
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
