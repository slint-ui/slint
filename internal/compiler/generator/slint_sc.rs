// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Code generator for the Slint SC (safety-critical) runtime.

use crate::CompilerConfiguration;
use crate::expression_tree::{Expression, Unit};
use crate::langtype::Type;
use crate::namedreference::NamedReference;
use crate::object_tree::{Document, ElementRc};
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
        let root = &component.root_element;
        let render_tree = emit_element(root, root);
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
                    let offset_x = 0i32;
                    let offset_y = 0i32;
                    #render_tree
                    Ok(())
                }
            }
        });
    }

    Ok(output)
}

/// Emit the render code for `elem` and its descendants: a block that adds the
/// element's position to the running `offset_x`/`offset_y`, paints its
/// background if it has one, and nests the children's blocks so that later
/// and deeper elements paint on top. An element without anything to paint in
/// its subtree emits nothing.
fn emit_element(elem: &ElementRc, root: &ElementRc) -> TokenStream {
    let geometry = elem.borrow().geometry_props.clone();
    let resolve = |nr: Option<&NamedReference>| nr.and_then(|nr| resolve_length(nr, root));
    // The default_geometry pass leaves a size binding on every element, and
    // the root's size resolves to the window size
    let w = resolve(geometry.as_ref().map(|g| &g.width)).expect("element without a width");
    let h = resolve(geometry.as_ref().map(|g| &g.height)).expect("element without a height");
    let x = resolve(geometry.as_ref().map(|g| &g.x)).unwrap_or_else(|| quote!(0i32));
    let y = resolve(geometry.as_ref().map(|g| &g.y)).unwrap_or_else(|| quote!(0i32));
    let mut background = elem
        .borrow()
        .bindings
        .get("background")
        .and_then(|b| extract_color_literal(&b.borrow().expression));
    if std::rc::Rc::ptr_eq(elem, root) {
        // The window background defaults to black, so that the whole frame
        // buffer is always painted
        background = Some(background.unwrap_or(0xff00_0000));
    }
    let fill = background.map(|color| {
        let [_, red, green, blue] = color.to_be_bytes();
        quote!(
            slint_sc::private_unstable_api::renderer::fill_rect(frame_buffer, [width, height],
                [offset_x, offset_y], [#w, #h], [#red, #green, #blue]);
        )
    });
    let children: Vec<TokenStream> =
        elem.borrow().children.iter().map(|child| emit_element(child, root)).collect();
    if fill.is_none() && children.iter().all(|c| c.is_empty()) {
        return TokenStream::new();
    }
    quote! {
        {
            let offset_x = offset_x + #x;
            let offset_y = offset_y + #y;
            #fill
            #(#children)*
        }
    }
}

/// Resolve the binding of a geometry property, following property references
/// until reaching a px literal or the window size. Compiler-generated bindings
/// can also contain arithmetic, e.g. centering an element in its parent.
fn resolve_length(nr: &NamedReference, root: &ElementRc) -> Option<TokenStream> {
    let element = nr.element();
    let binding = element.borrow().bindings.get(nr.name()).map(|b| b.borrow().expression.clone());
    match binding {
        Some(expr) => resolve_length_expression(&expr, root),
        None if std::rc::Rc::ptr_eq(&element, root) => match nr.name().as_str() {
            "width" => Some(quote!((width as i32))),
            "height" => Some(quote!((height as i32))),
            _ => None,
        },
        None => None,
    }
}

fn resolve_length_expression(expr: &Expression, root: &ElementRc) -> Option<TokenStream> {
    match expr {
        Expression::NumberLiteral(value, Unit::Px | Unit::None) => {
            let value = *value as i32;
            Some(quote!(#value))
        }
        Expression::PropertyReference(nr) => resolve_length(nr, root),
        Expression::BinaryExpression { lhs, rhs, op } => {
            let lhs = resolve_length_expression(lhs, root)?;
            let rhs = resolve_length_expression(rhs, root)?;
            match op {
                '+' => Some(quote!((#lhs + #rhs))),
                '-' => Some(quote!((#lhs - #rhs))),
                '*' => Some(quote!((#lhs * #rhs))),
                '/' => Some(quote!((#lhs / #rhs))),
                _ => None,
            }
        }
        _ => None,
    }
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
