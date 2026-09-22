// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use slint::{Image, SharedString};

pub fn format_nine_slice_expression(
    path: SharedString,
    top: i32,
    right: i32,
    bottom: i32,
    left: i32,
) -> SharedString {
    format!(
        "@image-url(\"{}\", nine-slice({} {} {} {}))",
        escape_slint_string(path.as_str()),
        top.max(0),
        right.max(0),
        bottom.max(0),
        left.max(0)
    )
    .into()
}

fn escape_slint_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

pub fn nine_slice_preview_image(
    source: Image,
    top: i32,
    right: i32,
    bottom: i32,
    left: i32,
) -> Image {
    let mut image = source;
    image.set_nine_slice_edges(
        nine_slice_edge_to_u16(top),
        nine_slice_edge_to_u16(right),
        nine_slice_edge_to_u16(bottom),
        nine_slice_edge_to_u16(left),
    );
    image
}

pub fn nine_slice_edge_to_u16(value: i32) -> u16 {
    value.clamp(0, u16::MAX as i32) as u16
}
