// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Code generator for the Slint SC (safety-critical) runtime.

use crate::CompilerConfiguration;
use crate::expression_tree::Expression;
use crate::langtype::Type;
use crate::object_tree::Document;
use itertools::Either;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

/// Public entry point called from `generator::generate`.
pub fn generate(
    doc: &Document,
    _compiler_config: &CompilerConfiguration,
) -> std::io::Result<TokenStream> {
    let mut output = TokenStream::new();

    for (export_name, export) in doc.exports.iter() {
        let Either::Left(component) = export else { continue };
        if component.is_global() {
            continue;
        }
        // FIXME: only the window background is drawn for now. Instead of
        // special-casing that property here, we should visit the item tree
        // and generate rendering code for every item.
        let background = component
            .root_element
            .borrow()
            .bindings
            .get("background")
            .and_then(|b| extract_color_literal(&b.borrow().expression))
            .unwrap_or(0xffff_ffff);
        let [_, red, green, blue] = background.to_be_bytes();
        let name = format_ident!("{}", export_name.name.as_str());
        output.extend(quote! {
            pub struct #name;
            impl #name {
                pub fn new() -> Self {
                    Self
                }

                /// Render the window into a frame buffer of packed RGB triplets,
                /// whose length must be `width * height * 3`.
                pub fn render_rgb8(&self, width: u32, height: u32, frame_buffer: &mut [u8]) -> Result<(), slint_sc::RenderError> {
                    if frame_buffer.len() != width as usize * height as usize * 3 {
                        return Err(slint_sc::RenderError::InvalidFrameBufferSize);
                    }
                    slint_sc::private_unstable_api::renderer::fill_rgb8(frame_buffer, #red, #green, #blue);
                    Ok(())
                }
            }
        });
    }

    Ok(output)
}

/// Return the ARGB value if the expression is a color literal, possibly cast to a brush.
fn extract_color_literal(expr: &Expression) -> Option<u32> {
    let Expression::Cast { from, to: Type::Color | Type::Brush } = expr else {
        return None;
    };
    match from.as_ref() {
        Expression::NumberLiteral(value, _) => Some(*value as u32),
        from => extract_color_literal(from),
    }
}
