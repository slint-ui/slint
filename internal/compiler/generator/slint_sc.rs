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
        let properties = declared_properties(root);
        let name = format_ident!("{}", export_name.name.as_str());
        let (struct_decl, new_body) = if properties.is_empty() {
            (quote!(pub struct #name;), quote!(Self))
        } else {
            let fields = properties.iter().map(|p| {
                let (field, ty) = (&p.field, &p.ty);
                quote!(#field: #ty,)
            });
            let init = properties.iter().map(|p| {
                let (field, init) = (&p.field, &p.init);
                quote!(#field: #init,)
            });
            (quote!(pub struct #name { #(#fields)* }), quote!(Self { #(#init)* }))
        };
        let accessors = properties.iter().map(|p| {
            let (field, ty) = (&p.field, &p.ty);
            let getter = p.getter.as_ref().map(|getter| {
                quote! {
                    pub fn #getter(&self) -> #ty {
                        self.#field
                    }
                }
            });
            let setter = p.setter.as_ref().map(|setter| {
                quote! {
                    pub fn #setter(&mut self, value: #ty) {
                        self.#field = value;
                    }
                }
            });
            quote!(#getter #setter)
        });
        output.extend(quote! {
            #struct_decl
            impl #name {
                pub fn new() -> Self {
                    #new_body
                }

                #(#accessors)*

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

struct DeclaredProperty {
    /// The struct field holding the value, `property_foo`.
    field: proc_macro2::Ident,
    /// `get_foo`, unless the property is private.
    getter: Option<proc_macro2::Ident>,
    /// `set_foo`, for `in` and `in-out` properties.
    setter: Option<proc_macro2::Ident>,
    /// The Rust type: `i32` for a length, `slint_sc::Color` for a color.
    ty: TokenStream,
    /// The initial value: the property's binding, or the type's default.
    init: TokenStream,
}

/// The properties declared in the source on the component's root element.
/// Compiler-introduced declarations have no syntax node and are skipped.
fn declared_properties(root: &ElementRc) -> Vec<DeclaredProperty> {
    use crate::object_tree::PropertyVisibility;
    let root_borrowed = root.borrow();
    root_borrowed
        .property_declarations
        .iter()
        .filter(|(_, decl)| decl.node.is_some())
        .map(|(name, decl)| {
            let init = root_borrowed
                .bindings
                .get(name)
                .map(|b| compile_expression(&b.borrow().expression, root))
                .unwrap_or_else(|| default_value(&decl.property_type));
            let snake = name.replace('-', "_");
            let (getter, setter) = match decl.visibility {
                PropertyVisibility::Input | PropertyVisibility::InOut => (true, true),
                PropertyVisibility::Output => (true, false),
                _ => (false, false),
            };
            DeclaredProperty {
                field: format_ident!("property_{snake}"),
                getter: getter.then(|| format_ident!("get_{snake}")),
                setter: setter.then(|| format_ident!("set_{snake}")),
                ty: rust_type(&decl.property_type),
                init,
            }
        })
        .collect()
}

/// The Rust type holding a value of the given Slint type.
fn rust_type(ty: &Type) -> TokenStream {
    match ty {
        Type::LogicalLength => quote!(i32),
        Type::Color => quote!(slint_sc::Color),
        // brush is not a declarable property type, and every other type was
        // rejected by the compiler
        _ => unreachable!(),
    }
}

/// The Rust value a property of the given type defaults to.
fn default_value(ty: &Type) -> TokenStream {
    match ty {
        Type::LogicalLength => quote!(0i32),
        Type::Color => quote!(slint_sc::Color::default()),
        _ => unreachable!(),
    }
}

/// Compile an expression of the Slint SC subset into Rust code. Lengths
/// become `i32` expressions and colors `slint_sc::Color` values. Besides the
/// literals of the subset, this handles the expressions that compiler passes
/// generate, like the centering arithmetic of default_geometry.
fn compile_expression(expr: &Expression, root: &ElementRc) -> TokenStream {
    match expr {
        Expression::NumberLiteral(value, Unit::Px | Unit::None) => {
            let value = *value as i32;
            quote!(#value)
        }
        Expression::Cast { from, to: Type::Color | Type::Brush } => match from.as_ref() {
            Expression::NumberLiteral(value, _) => {
                let argb = *value as u32;
                quote!(slint_sc::Color::from_argb_encoded(#argb))
            }
            from => compile_expression(from, root),
        },
        Expression::PropertyReference(nr) => {
            compile_property_reference(nr, root).expect("reference to an unresolved property")
        }
        Expression::BinaryExpression { lhs, rhs, op } => {
            let lhs = compile_expression(lhs, root);
            let rhs = compile_expression(rhs, root);
            match op {
                '+' => quote!((#lhs + #rhs)),
                '-' => quote!((#lhs - #rhs)),
                '*' => quote!((#lhs * #rhs)),
                '/' => quote!((#lhs / #rhs)),
                _ => unreachable!(),
            }
        }
        // Everything else was rejected by the compiler
        _ => unreachable!(),
    }
}

/// Emit the render code for `elem` and its descendants: a block that adds the
/// element's position to the running `offset_x`/`offset_y`, paints its
/// background if it has one, and nests the children's blocks so that later
/// and deeper elements paint on top. An element without anything to paint in
/// its subtree emits nothing.
fn emit_element(elem: &ElementRc, root: &ElementRc) -> TokenStream {
    let geometry = elem.borrow().geometry_props.clone();
    let resolve =
        |nr: Option<&NamedReference>| nr.and_then(|nr| compile_property_reference(nr, root));
    // The default_geometry pass leaves a size binding on every element, and
    // the root's size resolves to the window size
    let w = resolve(geometry.as_ref().map(|g| &g.width)).expect("element without a width");
    let h = resolve(geometry.as_ref().map(|g| &g.height)).expect("element without a height");
    let x = resolve(geometry.as_ref().map(|g| &g.x)).unwrap_or_else(|| quote!(0i32));
    let y = resolve(geometry.as_ref().map(|g| &g.y)).unwrap_or_else(|| quote!(0i32));
    let background =
        elem.borrow().bindings.get("background").map(|b| b.borrow().expression.clone());
    let mut color = background.map(|expr| compile_expression(&expr, root));
    if std::rc::Rc::ptr_eq(elem, root) {
        // The window background defaults to black, so that the whole frame
        // buffer is always painted
        color = Some(
            color.unwrap_or_else(|| quote!(slint_sc::Color::from_argb_encoded(0xff000000u32))),
        );
    }
    let fill = color.map(|color| {
        quote!(
            slint_sc::private_unstable_api::renderer::fill_rect(frame_buffer, [width, height],
                [offset_x, offset_y], [#w, #h], #color);
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

/// Compile a reference to a property: its compiled binding, or the window
/// size for the root's unbound width and height.
fn compile_property_reference(nr: &NamedReference, root: &ElementRc) -> Option<TokenStream> {
    let element = nr.element();
    let binding = element.borrow().bindings.get(nr.name()).map(|b| b.borrow().expression.clone());
    match binding {
        Some(expr) => Some(compile_expression(&expr, root)),
        None if std::rc::Rc::ptr_eq(&element, root) => match nr.name().as_str() {
            "width" => Some(quote!((width as i32))),
            "height" => Some(quote!((height as i32))),
            _ => None,
        },
        None => None,
    }
}
