// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Code generator for the Slint SC (safety-critical) runtime.

use crate::CompilerConfiguration;
use crate::expression_tree::{Expression, Unit};
use crate::langtype::Type;
use crate::namedreference::NamedReference;
use crate::object_tree::{Document, ElementRc, PropertyVisibility};
use itertools::Either;
use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use std::rc::Rc;

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
        let has_fields = properties.iter().any(|p| p.field.is_some());
        let (struct_decl, new_body) = if has_fields {
            let fields = properties.iter().filter_map(|p| {
                let (field, ty) = (p.field.as_ref()?, &p.ty);
                // A settable bound property is stored in an `Option`, `None`
                // until set; anything else is stored directly.
                Some(match p.kind {
                    PropertyKind::Binding(_) => quote!(#field: Option<#ty>,),
                    PropertyKind::Stored(_) => quote!(#field: #ty,),
                })
            });
            let init = properties.iter().filter_map(|p| {
                let field = p.field.as_ref()?;
                Some(match &p.kind {
                    PropertyKind::Stored(init) => quote!(#field: #init,),
                    PropertyKind::Binding(_) => quote!(#field: None,),
                })
            });
            (quote!(pub struct #name { #(#fields)* }), quote!(Self { #(#init)* }))
        } else {
            (quote!(pub struct #name;), quote!(Self))
        };
        let accessors = properties.iter().map(|p| {
            let ty = &p.ty;
            let getter = p.getter.as_ref().map(|getter| {
                let body = match &p.kind {
                    PropertyKind::Stored(_) => {
                        let field = p.field.as_ref().unwrap();
                        quote!(self.#field)
                    }
                    // A settable bound property falls back to the binding until
                    // set; a non-settable one always evaluates it.
                    PropertyKind::Binding(default) if p.setter.is_some() => {
                        let field = p.field.as_ref().unwrap();
                        quote!(self.#field.unwrap_or_else(|| #default))
                    }
                    PropertyKind::Binding(default) => quote!(#default),
                };
                quote!(pub fn #getter(&self) -> #ty { #body })
            });
            let setter = p.setter.as_ref().map(|setter| {
                let field = p.field.as_ref().expect("a setter belongs to a field");
                let assign = match p.kind {
                    PropertyKind::Binding(_) => quote!(self.#field = Some(value);),
                    PropertyKind::Stored(_) => quote!(self.#field = value;),
                };
                quote!(pub fn #setter(&mut self, value: #ty) { #assign })
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
    /// The value type `T`: `i32` for a length, `slint_sc::Color` for a color.
    ty: TokenStream,
    /// The struct field `property_foo`.
    /// A non-settable bound property has none: nothing can set it, so the getter
    /// always evaluates the binding.
    field: Option<Ident>,
    kind: PropertyKind,
    /// `get_foo`, unless the property is private.
    getter: Option<Ident>,
    /// `set_foo`, for an `in` or `in-out` property.
    setter: Option<Ident>,
}

/// How a declared property is stored and read.
///
/// The binding, if any, lives in the getter (which has `self`), never in the
/// field's initial value, so `new()` never needs to evaluate it.
enum PropertyKind {
    /// No binding: a `T` field the getter reads and the setter writes, its
    /// initial value the type default. The token is that default.
    Stored(TokenStream),
    /// A binding. A settable property holds it in an `Option<T>` field, `None`
    /// until set, so the getter falls back to the binding and the property
    /// tracks it until set; a non-settable one evaluates the binding on read.
    /// The token is the compiled binding.
    Binding(TokenStream),
}

/// Whether a property with this visibility is settable (`in`/`in-out`).
fn is_settable(visibility: &PropertyVisibility) -> bool {
    matches!(visibility, PropertyVisibility::Input | PropertyVisibility::InOut)
}

/// The properties declared in the source on the component's root element.
/// Compiler-introduced declarations have no syntax node and are skipped.
fn declared_properties(root: &ElementRc) -> Vec<DeclaredProperty> {
    let root_borrowed = root.borrow();
    root_borrowed
        .property_declarations
        .iter()
        .filter(|(_, decl)| decl.node.is_some())
        .map(|(name, decl)| {
            let snake = name.replace('-', "_");
            let ty = rust_type(&decl.property_type);
            let settable = is_settable(&decl.visibility);
            let has_getter = matches!(
                decl.visibility,
                PropertyVisibility::Input | PropertyVisibility::Output | PropertyVisibility::InOut
            );
            let kind = match root_borrowed.binding_cell_including_synthetic(name) {
                Some(b) => PropertyKind::Binding(compile_expression(&b.borrow().expression, root)),
                None => PropertyKind::Stored(default_value(&decl.property_type)),
            };
            // A non-settable binding is never stored, so it needs no field.
            let has_field = settable || matches!(kind, PropertyKind::Stored(_));
            DeclaredProperty {
                field: has_field.then(|| format_ident!("property_{snake}")),
                getter: has_getter.then(|| format_ident!("get_{snake}")),
                setter: settable.then(|| format_ident!("set_{snake}")),
                ty,
                kind,
            }
        })
        .collect()
}

/// The Rust type holding a value of the given Slint type.
fn rust_type(ty: &Type) -> TokenStream {
    match ty {
        Type::Int32 | Type::LogicalLength => quote!(i32),
        Type::Bool => quote!(bool),
        Type::Color => quote!(slint_sc::Color),
        // brush is not a declarable property type, and every other type was
        // rejected by the compiler
        _ => unreachable!(),
    }
}

/// The Rust value a property of the given type defaults to.
fn default_value(ty: &Type) -> TokenStream {
    match ty {
        Type::Int32 | Type::LogicalLength => quote!(0i32),
        Type::Bool => quote!(false),
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
        Expression::BoolLiteral(value) => quote!(#value),
        Expression::Cast { from, to: Type::Color | Type::Brush } => match from.as_ref() {
            Expression::NumberLiteral(value, _) => {
                let argb = *value as u32;
                quote!(slint_sc::Color::from_argb_encoded(#argb))
            }
            from => compile_expression(from, root),
        },
        // `int`, `float`, and `length` are all `i32` at runtime, so a cast
        // between them lowers to the operand itself.
        Expression::Cast { from, to: Type::Int32 | Type::Float32 } => {
            compile_expression(from, root)
        }
        Expression::PropertyReference(nr) => {
            // An unbound property has its type's default value.
            compile_property_reference(nr, root).unwrap_or_else(|| default_value(&nr.ty()))
        }
        Expression::BinaryExpression { lhs, rhs, op } => {
            let lhs = compile_expression(lhs, root);
            let rhs = compile_expression(rhs, root);
            // Arithmetic saturates at the `i32` bounds. `/` only comes from
            // compiler-synthesized geometry, whose divisor is a non-zero constant
            // (`saturating_div` still panics on a zero divisor).
            match op {
                '+' => quote!((#lhs).saturating_add(#rhs)),
                '-' => quote!((#lhs).saturating_sub(#rhs)),
                '*' => quote!((#lhs).saturating_mul(#rhs)),
                '/' => quote!((#lhs).saturating_div(#rhs)),
                // `&&` is `'&'` and `||` is `'|'`.
                '&' => quote!((#lhs) && (#rhs)),
                '|' => quote!((#lhs) || (#rhs)),
                // Comparison produces a `bool`. `==` is `'='` and `!=` is `'!'`;
                // `<=` is `'≤'` and `>=` is `'≥'`.
                '=' => quote!((#lhs) == (#rhs)),
                '!' => quote!((#lhs) != (#rhs)),
                '<' => quote!((#lhs) < (#rhs)),
                '>' => quote!((#lhs) > (#rhs)),
                '≤' => quote!((#lhs) <= (#rhs)),
                '≥' => quote!((#lhs) >= (#rhs)),
                _ => unreachable!(),
            }
        }
        Expression::UnaryOp { sub, op } => {
            let sub = compile_expression(sub, root);
            match op {
                // Negation saturates so that negating `i32::MIN` is defined.
                '-' => quote!((#sub).saturating_neg()),
                '+' => sub,
                '!' => quote!(!(#sub)),
                _ => unreachable!(),
            }
        }
        Expression::Condition { condition, true_expr, false_expr } => {
            let condition = compile_expression(condition, root);
            let true_expr = compile_expression(true_expr, root);
            let false_expr = compile_expression(false_expr, root);
            quote!(if #condition { #true_expr } else { #false_expr })
        }
        // A property read that appears more than once is hoisted into a local
        // variable, so a code block evaluates it once and reads it back.
        Expression::CodeBlock(statements) => {
            let statements = statements.iter().map(|s| compile_expression(s, root));
            quote!({ #(#statements)* })
        }
        Expression::StoreLocalVariable { name, value } => {
            let name = format_ident!("{}", name.replace('-', "_"));
            let value = compile_expression(value, root);
            quote!(let #name = #value;)
        }
        Expression::ReadLocalVariable { name, .. } => {
            let name = format_ident!("{}", name.replace('-', "_"));
            quote!(#name)
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
    let background = elem
        .borrow()
        .binding_cell_including_synthetic("background")
        .map(|b| b.borrow().expression.clone());
    let mut color = background.map(|expr| compile_expression(&expr, root));
    if Rc::ptr_eq(elem, root) {
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

/// Compile a reference to a property.
fn compile_property_reference(nr: &NamedReference, root: &ElementRc) -> Option<TokenStream> {
    let element = nr.element();
    let is_root = Rc::ptr_eq(&element, root);
    if is_root {
        let root_borrowed = root.borrow();
        if let Some(decl) =
            root_borrowed.property_declarations.get(nr.name()).filter(|d| d.node.is_some())
        {
            let binding = root_borrowed.binding_cell_including_synthetic(nr.name());
            let snake = nr.name().replace('-', "_");
            return Some(match binding {
                None => {
                    let field = format_ident!("property_{snake}");
                    quote!(self.#field)
                }
                Some(_) if is_settable(&decl.visibility) => {
                    let getter = format_ident!("get_{snake}");
                    quote!(self.#getter())
                }
                Some(b) => compile_expression(&b.borrow().expression, root),
            });
        }
    }
    match element.borrow().binding_cell_including_synthetic(nr.name()) {
        Some(b) => Some(compile_expression(&b.borrow().expression, root)),
        None if is_root => match nr.name().as_str() {
            "width" => Some(quote!((width as i32))),
            "height" => Some(quote!((height as i32))),
            _ => None,
        },
        None => None,
    }
}
