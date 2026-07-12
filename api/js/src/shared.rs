// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Engine-agnostic logic shared by the Node.js (napi) and browser
//! (wasm-bindgen) modules. Everything here works on Slint and plain Rust
//! types; turning the results into engine values stays in the respective
//! module.

use i_slint_compiler::langtype::Type;
use i_slint_core::ImageInner;
use i_slint_core::graphics::{Image, SharedImageBuffer};

/// The struct types a compilation exports, as (name, struct of per-field
/// default values). Only structs declared in .slint are included.
pub fn extract_structs<'a>(
    types: impl Iterator<Item = &'a Type>,
) -> impl Iterator<Item = (String, slint_interpreter::Struct)> {
    types.filter_map(|ty| match ty {
        Type::Struct(s) if s.node().is_some() => {
            let name = s.name.slint_name()?;
            Some((
                name.to_string(),
                slint_interpreter::Struct::from_iter(s.fields.iter().map(
                    |(field_name, field_type)| {
                        (
                            field_name.to_string(),
                            slint_interpreter::default_value_for_type(field_type),
                        )
                    },
                )),
            ))
        }
        _ => None,
    })
}

/// The enum types a compilation exports, as (name, JS variant names — with
/// dashes replaced by underscores).
pub fn extract_enums<'a>(
    types: impl Iterator<Item = &'a Type>,
) -> impl Iterator<Item = (String, Vec<String>)> {
    types.filter_map(|ty| match ty {
        Type::Enumeration(en) => {
            Some((en.name.to_string(), en.values.iter().map(|v| v.replace('-', "_")).collect()))
        }
        _ => None,
    })
}

/// A compiler diagnostic flattened to plain data.
pub struct DiagnosticData {
    pub level: slint_interpreter::DiagnosticLevel,
    pub message: String,
    /// Starts at 1.
    pub line_number: usize,
    /// Starts at 1.
    pub column_number: usize,
    pub file_name: Option<String>,
}

impl From<&slint_interpreter::Diagnostic> for DiagnosticData {
    fn from(d: &slint_interpreter::Diagnostic) -> Self {
        let (line_number, column_number) = d.line_column();
        Self {
            level: d.level(),
            message: d.message().into(),
            line_number,
            column_number,
            file_name: d.source_file().map(|path| path.to_string_lossy().into()),
        }
    }
}

/// Parse a CSS color literal: `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`,
/// `rgb(r, g, b)`, `rgba(r, g, b, a)`, `hsl(...)`, `hsla(...)`, or a named
/// color (e.g. `"red"`). Returns `None` if the string is not a valid color.
pub fn parse_css_color(s: &str) -> Option<i_slint_core::Color> {
    let c = s.trim().parse::<css_color_parser2::Color>().ok()?;
    Some(i_slint_core::Color::from_argb_u8((c.a * 255.0).round() as u8, c.r, c.g, c.b))
}

/// Render an image to non-premultiplied RGBA8 bytes, the format the JS APIs
/// expose (Buffer on Node.js, ImageData in the browser).
pub fn image_to_rgba8(image: &Image) -> Vec<u8> {
    let size = image.size();
    let image_inner: &ImageInner = image.into();
    match image_inner.render_to_buffer(None) {
        Some(SharedImageBuffer::RGBA8(buffer)) => buffer.as_bytes().to_vec(),
        Some(SharedImageBuffer::RGB8(buffer)) => {
            let mut rgba = Vec::with_capacity(buffer.as_bytes().len() / 3 * 4);
            for px in buffer.as_bytes().chunks_exact(3) {
                rgba.extend_from_slice(px);
                rgba.push(255);
            }
            rgba
        }
        Some(SharedImageBuffer::RGBA8Premultiplied(buffer)) => {
            let mut rgba = buffer.as_bytes().to_vec();
            for px in rgba.chunks_exact_mut(4) {
                let a = px[3] as u16;
                if a > 0 && a < 255 {
                    px[0] = (px[0] as u16 * 255 / a) as u8;
                    px[1] = (px[1] as u16 * 255 / a) as u8;
                    px[2] = (px[2] as u16 * 255 / a) as u8;
                }
            }
            rgba
        }
        None => vec![0; size.width as usize * size.height as usize * 4],
    }
}
