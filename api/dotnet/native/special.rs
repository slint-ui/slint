// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The `keys`, `styled-text`, and `data-transfer` values.

use i_slint_core::data_transfer::DataTransfer;
use i_slint_core::input::Keys;
use i_slint_core::styled_text::StyledText;
use slint_interpreter::Value;

use crate::{OwnedString, Utf8Str, guard, guard_bool, into_raw, utf8_slice};

/// Parses key parts such as `Control`, `Shift?`, and `Z`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_value_new_keys(
    parts: *const Utf8Str,
    count: usize,
) -> *mut Value {
    guard(std::ptr::null_mut(), || {
        let parts = unsafe { utf8_slice(parts, count) }?;
        Keys::from_parts(parts).map(|keys| into_raw(Value::Keys(keys))).map_err(|e| e.to_string())
    })
}

/// The text of a `keys` or `styled-text` value, or no string for another value.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_value_describe(value: &Value) -> OwnedString {
    match value {
        Value::Keys(keys) => keys.to_string().into(),
        Value::StyledText(text) => format!("{text:?}").into(),
        _ => OwnedString::none(),
    }
}

/// `kind` selects 0 empty, 1 plain text, or 2 Markdown.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_value_new_styled_text(
    kind: i32,
    text: Utf8Str,
) -> *mut Value {
    guard(std::ptr::null_mut(), || {
        let text = unsafe { text.as_str() }?;
        let styled = match kind {
            0 => StyledText::default(),
            1 => StyledText::from_plain_text(text),
            _ => StyledText::from_markdown(text).map_err(|e| e.to_string())?,
        };
        Ok(into_raw(Value::StyledText(styled)))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn slint_dotnet_value_new_data_transfer() -> *mut Value {
    guard(std::ptr::null_mut(), || Ok(into_raw(Value::DataTransfer(DataTransfer::default()))))
}

fn data_transfer(value: &Value) -> Result<&DataTransfer, String> {
    match value {
        Value::DataTransfer(data) => Ok(data),
        _ => Err("value isn't a data transfer".into()),
    }
}

fn data_transfer_mut(value: &mut Value) -> Result<&mut DataTransfer, String> {
    match value {
        Value::DataTransfer(data) => Ok(data),
        _ => Err("value isn't a data transfer".into()),
    }
}

/// The plain text, or no string if there's none.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_data_transfer_plain_text(value: &Value) -> OwnedString {
    guard(OwnedString::none(), || {
        Ok(data_transfer(value)?
            .plain_text()
            .map_or_else(|_| OwnedString::none(), |text| text.to_string().into()))
    })
}

/// An empty `text` clears the plain text.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_data_transfer_set_plain_text(
    value: &mut Value,
    text: Utf8Str,
) -> bool {
    guard_bool(|| {
        let text = unsafe { text.as_str() }?;
        data_transfer_mut(value)?.set_plain_text(text.into());
        Ok(())
    })
}

/// The image as a new value, or null if there's none.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_data_transfer_image(value: &Value) -> *mut Value {
    guard(std::ptr::null_mut(), || {
        Ok(data_transfer(value)?
            .image()
            .map_or(std::ptr::null_mut(), |image| into_raw(Value::Image(image))))
    })
}

/// A null `image` clears the image.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_data_transfer_set_image(
    value: &mut Value,
    image: Option<&Value>,
) -> bool {
    guard_bool(|| {
        let image = match image {
            Some(Value::Image(image)) => image.clone(),
            Some(_) => return Err("value isn't an image".into()),
            None => Default::default(),
        };
        data_transfer_mut(value)?.set_image(image);
        Ok(())
    })
}

/// Bit 0: has plain text, bit 1: has an image, bit 2: is empty.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_data_transfer_state(value: &Value) -> i32 {
    guard(0, || {
        let data = data_transfer(value)?;
        Ok(i32::from(data.has_plain_text())
            | i32::from(data.has_image()) << 1
            | i32::from(data.is_empty()) << 2)
    })
}
