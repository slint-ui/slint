// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

// cSpell: ignore conv gdata powf punct vref

/*! module for the Rust code generator

Some convention used in the generated code:
 - `_self` is of type `Pin<&ComponentType>`  where ComponentType is the type of the generated sub component,
    this is existing for any evaluation of a binding
 - `self_rc` is of type `VRc<ComponentVTable, ComponentType>` or `Rc<ComponentType>` for globals
    this is usually a local variable to the init code that shouldn't rbe relied upon by the binding code.
*/

use crate::expression_tree::{BuiltinFunction, EasingCurve, OperatorClass};
use crate::langtype::{ElementType, Type};
use crate::layout::Orientation;
use crate::llr::{
    self, EvaluationContext as llr_EvaluationContext, Expression, ParentCtx as llr_ParentCtx,
    TypeResolutionContext as _,
};
use crate::object_tree::Document;
use itertools::Either;
use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use std::collections::BTreeMap;
use std::num::NonZeroUsize;

type EvaluationContext<'a> = llr_EvaluationContext<'a, TokenStream>;
type ParentCtx<'a> = llr_ParentCtx<'a, TokenStream>;

fn ident(ident: &str) -> proc_macro2::Ident {
    if ident.contains('-') {
        format_ident!("r#{}", ident.replace('-', "_"))
    } else {
        format_ident!("r#{}", ident)
    }
}

impl quote::ToTokens for Orientation {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let tks = match self {
            Orientation::Horizontal => {
                quote!(slint::private_unstable_api::re_exports::Orientation::Horizontal)
            }
            Orientation::Vertical => {
                quote!(slint::private_unstable_api::re_exports::Orientation::Vertical)
            }
        };
        tokens.extend(tks);
    }
}

impl quote::ToTokens for crate::embedded_resources::PixelFormat {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        use crate::embedded_resources::PixelFormat::*;
        let tks = match self {
            Rgb => quote!(slint::private_unstable_api::re_exports::PixelFormat::Rgb),
            Rgba => quote!(slint::private_unstable_api::re_exports::PixelFormat::Rgba),
            RgbaPremultiplied => {
                quote!(slint::private_unstable_api::re_exports::PixelFormat::RgbaPremultiplied)
            }
            AlphaMap(_) => quote!(slint::private_unstable_api::re_exports::PixelFormat::AlphaMap),
        };
        tokens.extend(tks);
    }
}

fn rust_primitive_type(ty: &Type) -> Option<proc_macro2::TokenStream> {
    match ty {
        Type::Void => Some(quote!(())),
        Type::Int32 => Some(quote!(i32)),
        Type::Float32 => Some(quote!(f32)),
        Type::String => Some(quote!(slint::private_unstable_api::re_exports::SharedString)),
        Type::Color => Some(quote!(slint::private_unstable_api::re_exports::Color)),
        Type::Duration => Some(quote!(i64)),
        Type::Angle => Some(quote!(f32)),
        Type::PhysicalLength => Some(quote!(slint::private_unstable_api::re_exports::Coord)),
        Type::LogicalLength => Some(quote!(slint::private_unstable_api::re_exports::Coord)),
        Type::Rem => Some(quote!(f32)),
        Type::Percent => Some(quote!(f32)),
        Type::Bool => Some(quote!(bool)),
        Type::Image => Some(quote!(slint::private_unstable_api::re_exports::Image)),
        Type::Struct { fields, name: None, .. } => {
            let elem = fields.values().map(rust_primitive_type).collect::<Option<Vec<_>>>()?;
            // This will produce a tuple
            Some(quote!((#(#elem,)*)))
        }
        Type::Struct { name: Some(name), .. } => Some(struct_name_to_tokens(name)),
        Type::Array(o) => {
            let inner = rust_primitive_type(o)?;
            Some(quote!(slint::private_unstable_api::re_exports::ModelRc<#inner>))
        }
        Type::Enumeration(e) => {
            let e = ident(&e.name);
            Some(quote!(slint::private_unstable_api::re_exports::#e))
        }
        Type::Brush => Some(quote!(slint::Brush)),
        Type::LayoutCache => Some(quote!(
            slint::private_unstable_api::re_exports::SharedVector<
                slint::private_unstable_api::re_exports::Coord,
            >
        )),
        _ => None,
    }
}

fn rust_property_type(ty: &Type) -> Option<proc_macro2::TokenStream> {
    match ty {
        Type::LogicalLength => Some(quote!(slint::private_unstable_api::re_exports::LogicalLength)),
        _ => rust_primitive_type(ty),
    }
}

fn primitive_property_value(ty: &Type, property_accessor: TokenStream) -> TokenStream {
    let value = quote!(#property_accessor.get());
    match ty {
        Type::LogicalLength => quote!(#value.get()),
        _ => value,
    }
}

fn set_primitive_property_value(ty: &Type, value_expression: TokenStream) -> TokenStream {
    match ty {
        Type::LogicalLength => {
            let rust_ty = rust_primitive_type(ty).unwrap_or(quote!(_));
            quote!(slint::private_unstable_api::re_exports::LogicalLength::new(#value_expression as #rust_ty))
        }
        _ => value_expression,
    }
}

/// Generate the rust code for the given component.
pub fn generate(doc: &Document) -> TokenStream {
    let (structs_ids, structs): (Vec<_>, Vec<_>) = doc
        .root_component
        .used_types
        .borrow()
        .structs
        .iter()
        .filter_map(|ty| {
            if let Type::Struct { fields, name: Some(name), node: Some(_) } = ty {
                Some((ident(name), generate_struct(name, fields)))
            } else {
                None
            }
        })
        .unzip();

    if matches!(
        doc.root_component.root_element.borrow().base_type,
        ElementType::Error | ElementType::Global
    ) {
        // empty document, nothing to generate
        return TokenStream::default();
    }

    let llr = crate::llr::lower_to_item_tree::lower_to_item_tree(&doc.root_component);

    let sub_compos = llr
        .sub_components
        .iter()
        .map(|sub_compo| generate_sub_component(sub_compo, &llr, None, quote!(), None, false))
        .collect::<Vec<_>>();

    let compo = generate_public_component(&llr);
    let compo_id = public_component_id(&llr.item_tree.root);
    let compo_module = format_ident!("slint_generated{}", compo_id);
    let version_check = format_ident!(
        "VersionCheck_{}_{}_{}",
        env!("CARGO_PKG_VERSION_MAJOR"),
        env!("CARGO_PKG_VERSION_MINOR"),
        env!("CARGO_PKG_VERSION_PATCH"),
    );

    let globals =
        llr.globals.iter().filter(|glob| !glob.is_builtin).map(|glob| generate_global(glob, &llr));
    let globals_ids = llr.globals.iter().filter(|glob| glob.exported).flat_map(|glob| {
        std::iter::once(ident(&glob.name)).chain(glob.aliases.iter().map(|x| ident(x)))
    });

    #[cfg(feature = "software-renderer")]
    let link_section =
        std::env::var("SLINT_ASSET_SECTION").ok().map(|section| quote!(#[link_section = #section]));

    let resource_symbols = doc.root_component
        .embedded_file_resources
        .borrow()
        .iter()
        .map(|(path, er)| {
            let symbol = format_ident!("SLINT_EMBEDDED_RESOURCE_{}", er.id);
            match &er.kind {
                crate::embedded_resources::EmbeddedResourcesKind::RawData => {
                    let data = embedded_file_tokens(path);
                    quote!(static #symbol: &'static [u8] = #data;)
                }
                #[cfg(feature = "software-renderer")]
                crate::embedded_resources::EmbeddedResourcesKind::TextureData(crate::embedded_resources::Texture {
                    data, format, rect,
                    total_size: crate::embedded_resources::Size{width, height},
                    original_size: crate::embedded_resources::Size{width: unscaled_width, height: unscaled_height},
                }) => {
                    let (r_x, r_y, r_w, r_h) = (rect.x(), rect.y(), rect.width(), rect.height());
                    let color = if let crate::embedded_resources::PixelFormat::AlphaMap([r, g, b]) = format {
                        quote!(slint::private_unstable_api::re_exports::Color::from_rgb_u8(#r, #g, #b))
                    } else {
                        quote!(slint::private_unstable_api::re_exports::Color::from_argb_encoded(0))
                    };
                    let symbol_data = format_ident!("SLINT_EMBEDDED_RESOURCE_{}_DATA", er.id);
                    let data_size = data.len();
                    quote!(
                        #link_section
                        static #symbol_data : [u8; #data_size]= [#(#data),*];
                        #link_section
                        static #symbol: slint::private_unstable_api::re_exports::StaticTextures = slint::private_unstable_api::re_exports::StaticTextures{
                            size: slint::private_unstable_api::re_exports::IntSize::new(#width as _, #height as _),
                            original_size: slint::private_unstable_api::re_exports::IntSize::new(#unscaled_width as _, #unscaled_height as _),
                            data: Slice::from_slice(&#symbol_data),
                            textures: Slice::from_slice(&[
                                slint::private_unstable_api::re_exports::StaticTexture {
                                    rect: slint::private_unstable_api::re_exports::euclid::rect(#r_x as _, #r_y as _, #r_w as _, #r_h as _),
                                    format: #format,
                                    color: #color,
                                    index: 0,
                                }
                            ])
                        };
                    )
                },
                #[cfg(feature = "software-renderer")]
                crate::embedded_resources::EmbeddedResourcesKind::BitmapFontData(crate::embedded_resources::BitmapFont { family_name, character_map, units_per_em, ascent, descent, glyphs, weight, italic }) => {

                    let character_map_size = character_map.len();

                     let character_map = character_map.iter().map(|crate::embedded_resources::CharacterMapEntry{code_point, glyph_index}| quote!(slint::private_unstable_api::re_exports::CharacterMapEntry { code_point: #code_point, glyph_index: #glyph_index }));

                    let glyphs_size = glyphs.len();

                    let glyphs = glyphs.iter().map(|crate::embedded_resources::BitmapGlyphs{pixel_size, glyph_data}| {
                        let glyph_data_size = glyph_data.len();
                        let glyph_data = glyph_data.iter().map(|crate::embedded_resources::BitmapGlyph{x, y, width, height, x_advance, data}|{
                            let data_size = data.len();
                            quote!(
                                slint::private_unstable_api::re_exports::BitmapGlyph {
                                    x: #x,
                                    y: #y,
                                    width: #width,
                                    height: #height,
                                    x_advance: #x_advance,
                                    data: Slice::from_slice({
                                        #link_section
                                        static DATA : [u8; #data_size] = [#(#data),*];
                                        &DATA
                                    }),
                                }
                            )
                        });

                        quote!(
                            slint::private_unstable_api::re_exports::BitmapGlyphs {
                                pixel_size: #pixel_size,
                                glyph_data: Slice::from_slice({
                                    #link_section
                                    static GDATA : [slint::private_unstable_api::re_exports::BitmapGlyph; #glyph_data_size] = [#(#glyph_data),*];
                                    &GDATA
                                }),
                            }
                        )
                    });


                    quote!(
                        #link_section
                        static #symbol: slint::private_unstable_api::re_exports::BitmapFont = slint::private_unstable_api::re_exports::BitmapFont {
                            family_name: Slice::from_slice(#family_name.as_bytes()),
                            character_map: Slice::from_slice({
                                #link_section
                                static CM : [slint::private_unstable_api::re_exports::CharacterMapEntry; #character_map_size] = [#(#character_map),*];
                                &CM
                            }),
                            units_per_em: #units_per_em,
                            ascent: #ascent,
                            descent: #descent,
                            glyphs: Slice::from_slice({
                                #link_section
                                static GLYPHS : [slint::private_unstable_api::re_exports::BitmapGlyphs; #glyphs_size] = [#(#glyphs),*];
                                &GLYPHS
                            }),
                            weight: #weight,
                            italic: #italic,
                        };
                    )
                },
            }
        }).collect::<Vec<_>>();

    quote! {
        #[allow(non_snake_case)]
        #[allow(non_camel_case_types)]
         // These make code generation easier
        #[allow(clippy::style)]
        #[allow(clippy::complexity)]
        #[allow(unused_braces)]
        #[allow(clippy::erasing_op)]
        #[allow(clippy::approx_constant)] // We may get those from .slint inputs!
        #[allow(clippy::eq_op)] // The generated code will compare/subtract/etc. equal values
        #[allow(clippy::cmp_owned)] // The generated code will do this
        #[allow(clippy::redundant_clone)] // TODO: We clone properties more often then needed
                                          // according to clippy!
        mod #compo_module {
            use slint::private_unstable_api::re_exports::*;
            #(#structs)*
            #(#globals)*
            #(#sub_compos)*
            #compo
            #(#resource_symbols)*
            const _THE_SAME_VERSION_MUST_BE_USED_FOR_THE_COMPILER_AND_THE_RUNTIME : slint::#version_check = slint::#version_check;
        }
        pub use #compo_module::{#compo_id #(,#structs_ids)* #(,#globals_ids)* };
        pub use slint::{ComponentHandle as _, Global as _, ModelExt as _};
    }
}

fn generate_public_component(llr: &llr::PublicComponent) -> TokenStream {
    let public_component_id = public_component_id(&llr.item_tree.root);
    let inner_component_id = inner_component_id(&llr.item_tree.root);
    let global_container_id = format_ident!("Globals_{}", public_component_id);

    let component =
        generate_item_tree(&llr.item_tree, llr, None, quote!(globals: #global_container_id), None);

    let ctx = EvaluationContext {
        public_component: llr,
        current_sub_component: Some(&llr.item_tree.root),
        current_global: None,
        generator_state: quote!(_self),
        parent: None,
        argument_types: &[],
    };

    let property_and_callback_accessors = public_api(
        &llr.public_properties,
        &llr.private_properties,
        quote!(vtable::VRc::as_pin_ref(&self.0)),
        &ctx,
    );

    let global_names =
        llr.globals.iter().map(|g| format_ident!("global_{}", ident(&g.name))).collect::<Vec<_>>();
    let global_types = llr.globals.iter().map(global_inner_name).collect::<Vec<_>>();

    quote!(
        #component
        pub struct #public_component_id(vtable::VRc<slint::private_unstable_api::re_exports::ComponentVTable, #inner_component_id>);

        impl #public_component_id {
            pub fn new() -> core::result::Result<Self, slint::PlatformError> {
                let inner = #inner_component_id::new()?;
                #(inner.globals.#global_names.clone().init(&inner);)*
                #inner_component_id::user_init(slint::private_unstable_api::re_exports::VRc::map(inner.clone(), |x| x));
                core::result::Result::Ok(Self(inner))
            }

            #property_and_callback_accessors
        }

        impl From<#public_component_id> for vtable::VRc<slint::private_unstable_api::re_exports::ComponentVTable, #inner_component_id> {
            fn from(value: #public_component_id) -> Self {
                value.0
            }
        }

        impl slint::ComponentHandle for #public_component_id {
            type Inner = #inner_component_id;
            fn as_weak(&self) -> slint::Weak<Self> {
                slint::Weak::new(&self.0)
            }

            fn clone_strong(&self) -> Self {
                Self(self.0.clone())
            }

            fn from_inner(inner: vtable::VRc<slint::private_unstable_api::re_exports::ComponentVTable, #inner_component_id>) -> Self {
                Self(inner)
            }

            fn run(&self) -> core::result::Result<(), slint::PlatformError> {
                self.show()?;
                slint::run_event_loop()?;
                self.hide()?;
                core::result::Result::Ok(())
            }

            fn show(&self) -> core::result::Result<(), slint::PlatformError> {
                self.window().show()
            }

            fn hide(&self) -> core::result::Result<(), slint::PlatformError> {
                self.window().hide()
            }

            fn window(&self) -> &slint::Window {
                vtable::VRc::as_pin_ref(&self.0).get_ref().window_adapter.get().unwrap().window()
            }

            fn global<'a, T: slint::Global<'a, Self>>(&'a self) -> T {
                T::get(&self)
            }
        }

        #[allow(dead_code)] // FIXME: some global are unused because of optimization, we should then remove them completely
        struct #global_container_id {
            #(#global_names : ::core::pin::Pin<slint::private_unstable_api::re_exports::Rc<#global_types>>,)*
        }
        impl Default for #global_container_id {
            fn default() -> Self {
                Self {
                    #(#global_names : #global_types::new(),)*
                }
            }
        }
    )
}

fn generate_struct(name: &str, fields: &BTreeMap<String, Type>) -> TokenStream {
    let component_id = struct_name_to_tokens(name);
    let (declared_property_vars, declared_property_types): (Vec<_>, Vec<_>) =
        fields.iter().map(|(name, ty)| (ident(name), rust_primitive_type(ty).unwrap())).unzip();

    quote! {
        #[derive(Default, PartialEq, Debug, Clone)]
        pub struct #component_id {
            #(pub #declared_property_vars : #declared_property_types),*
        }
    }
}

fn handle_property_init(
    prop: &llr::PropertyReference,
    binding_expression: &llr::BindingExpression,
    init: &mut Vec<TokenStream>,
    ctx: &EvaluationContext,
) {
    let rust_property = access_member(prop, ctx);
    let prop_type = ctx.property_ty(prop);

    let init_self_pin_ref = if ctx.current_global.is_some() {
        quote!(let _self = self_rc.as_ref();)
    } else {
        quote!(let _self = self_rc.as_pin_ref();)
    };

    if let Type::Callback { args, return_type } = &prop_type {
        let mut ctx2 = ctx.clone();
        ctx2.argument_types = args;
        let tokens_for_expression =
            compile_expression(&binding_expression.expression.borrow(), &ctx2);
        let as_ = if return_type.as_deref().map_or(true, |t| matches!(t, Type::Void)) {
            quote!(;)
        } else {
            quote!(as _)
        };
        init.push(quote!({
            #[allow(unreachable_code, unused)]
            slint::private_unstable_api::set_callback_handler(#rust_property, &self_rc, {
                move |self_rc, args| {
                    #init_self_pin_ref
                    (#tokens_for_expression) #as_
                }
            });
        }));
    } else {
        let tokens_for_expression =
            compile_expression(&binding_expression.expression.borrow(), ctx);

        let tokens_for_expression = set_primitive_property_value(prop_type, tokens_for_expression);

        init.push(if binding_expression.is_constant && !binding_expression.is_state_info {
            let t = rust_property_type(prop_type).unwrap_or(quote!(_));

            // When there is a `return` statement, we must use a lambda expression in the generated code so that the
            // generated code can have an actual return in it. We only want to do that if necessary because otherwise
            // this would slow down the rust compilation
            let mut uses_return = false;
            binding_expression.expression.visit_recursive(&mut |e| {
                if matches!(e, Expression::ReturnStatement(..)) {
                    uses_return = true;
                }
            });

            if uses_return {
                quote! { #[allow(unreachable_code)] #rust_property.set((||-> #t { (#tokens_for_expression) as #t })()); }
            } else {
                quote! { #rust_property.set({ (#tokens_for_expression) as #t }); }
            }
        } else {
            let maybe_cast_to_property_type = if binding_expression.expression.borrow().ty(ctx) == Type::Invalid {
                // Don't cast if the Rust code is the never type, as with return statements inside a block, the
                // type of the return expression is `()` instead of `!`.
                None
            } else {
                Some(quote!(as _))
            };

            let binding_tokens = quote!(move |self_rc| {
                #init_self_pin_ref
                (#tokens_for_expression) #maybe_cast_to_property_type
            });

            if binding_expression.is_state_info {
                quote! { {
                    slint::private_unstable_api::set_property_state_binding(#rust_property, &self_rc, #binding_tokens);
                } }
            } else {
                match &binding_expression.animation {
                    Some(llr::Animation::Static(anim)) => {
                        let anim = compile_expression(anim, ctx);
                        quote! { {
                            #init_self_pin_ref
                            slint::private_unstable_api::set_animated_property_binding(#rust_property, &self_rc, #binding_tokens, #anim);
                        } }
                    }
                    Some(llr::Animation::Transition(anim)) => {
                        let anim = compile_expression(anim, ctx);
                        quote! {
                            slint::private_unstable_api::set_animated_property_binding_for_transition(
                                #rust_property, &self_rc, #binding_tokens, move |self_rc| {
                                    #init_self_pin_ref
                                    #anim
                                }
                            );
                        }
                    }
                    None => {
                        quote! { {
                            slint::private_unstable_api::set_property_binding(#rust_property, &self_rc, #binding_tokens);
                        } }
                    }
                }
            }
        });
    }
}

/// Public API for Global and root component
fn public_api(
    public_properties: &llr::PublicProperties,
    private_properties: &llr::PrivateProperties,
    self_init: TokenStream,
    ctx: &EvaluationContext,
) -> TokenStream {
    let mut property_and_callback_accessors: Vec<TokenStream> = vec![];
    for p in public_properties {
        let prop_ident = ident(&p.name);
        let prop = access_member(&p.prop, ctx);

        if let Type::Callback { args, return_type } = &p.ty {
            let callback_args =
                args.iter().map(|a| rust_primitive_type(a).unwrap()).collect::<Vec<_>>();
            let return_type =
                return_type.as_ref().map_or(quote!(()), |a| rust_primitive_type(a).unwrap());
            let args_name = (0..args.len()).map(|i| format_ident!("arg_{}", i)).collect::<Vec<_>>();
            let caller_ident = format_ident!("invoke_{}", prop_ident);
            property_and_callback_accessors.push(quote!(
                #[allow(dead_code)]
                pub fn #caller_ident(&self, #(#args_name : #callback_args,)*) -> #return_type {
                    let _self = #self_init;
                    #prop.call(&(#(#args_name,)*))
                }
            ));
            let on_ident = format_ident!("on_{}", prop_ident);
            let args_index = (0..callback_args.len()).map(proc_macro2::Literal::usize_unsuffixed);
            property_and_callback_accessors.push(quote!(
                #[allow(dead_code)]
                pub fn #on_ident(&self, mut f: impl FnMut(#(#callback_args),*) -> #return_type + 'static) {
                    let _self = #self_init;
                    #[allow(unused)]
                    #prop.set_handler(
                        // FIXME: why do i need to clone here?
                        move |args| f(#(args.#args_index.clone()),*)
                    )
                }
            ));
        } else if let Type::Function { return_type, args } = &p.ty {
            let callback_args =
                args.iter().map(|a| rust_primitive_type(a).unwrap()).collect::<Vec<_>>();
            let return_type = rust_primitive_type(return_type).unwrap();
            let args_name = (0..args.len()).map(|i| format_ident!("arg_{}", i)).collect::<Vec<_>>();
            let caller_ident = format_ident!("invoke_{}", prop_ident);
            property_and_callback_accessors.push(quote!(
                #[allow(dead_code)]
                pub fn #caller_ident(&self, #(#args_name : #callback_args,)*) -> #return_type {
                    let _self = #self_init;
                    #prop(#(#args_name,)*)
                }
            ));
        } else {
            let rust_property_type = rust_primitive_type(&p.ty).unwrap();

            let getter_ident = format_ident!("get_{}", prop_ident);

            let prop_expression = primitive_property_value(&p.ty, prop);

            property_and_callback_accessors.push(quote!(
                #[allow(dead_code)]
                pub fn #getter_ident(&self) -> #rust_property_type {
                    #[allow(unused_imports)]
                    let _self = #self_init;
                    #prop_expression
                }
            ));

            let setter_ident = format_ident!("set_{}", prop_ident);
            if !p.read_only {
                let set_value = property_set_value_tokens(&p.prop, quote!(value), ctx);
                property_and_callback_accessors.push(quote!(
                    #[allow(dead_code)]
                    pub fn #setter_ident(&self, value: #rust_property_type) {
                        #[allow(unused_imports)]
                        let _self = #self_init;
                        #set_value
                    }
                ));
            } else {
                property_and_callback_accessors.push(quote!(
                    #[allow(dead_code)] fn #setter_ident(&self, _read_only_property : ()) { }
                ));
            }
        }
    }

    for (name, ty) in private_properties {
        let prop_ident = ident(name);
        if let Type::Function { .. } = ty {
            let caller_ident = format_ident!("invoke_{}", prop_ident);
            property_and_callback_accessors.push(
                quote!( #[allow(dead_code)] fn #caller_ident(&self, _private_function: ()) {} ),
            );
        } else {
            let getter_ident = format_ident!("get_{}", prop_ident);
            let setter_ident = format_ident!("set_{}", prop_ident);
            property_and_callback_accessors.push(quote!(
                #[allow(dead_code)] fn #getter_ident(&self, _private_property: ()) {}
                #[allow(dead_code)] fn #setter_ident(&self, _private_property: ()) {}
            ));
        }
    }

    quote!(#(#property_and_callback_accessors)*)
}

/// Generate the rust code for the given component.
fn generate_sub_component(
    component: &llr::SubComponent,
    root: &llr::PublicComponent,
    parent_ctx: Option<ParentCtx>,
    extra_fields: TokenStream,
    index_property: Option<llr::PropertyIndex>,
    pinned_drop: bool,
) -> TokenStream {
    let inner_component_id = inner_component_id(component);

    let ctx = EvaluationContext::new_sub_component(
        root,
        component,
        quote!(_self.root.get().unwrap().upgrade().unwrap()),
        parent_ctx.clone(),
    );
    let mut extra_components = component
        .popup_windows
        .iter()
        .map(|c| generate_item_tree(c, root, Some(ParentCtx::new(&ctx, None)), quote!(), None))
        .collect::<Vec<_>>();

    let mut declared_property_vars = vec![];
    let mut declared_property_types = vec![];
    let mut declared_callbacks = vec![];
    let mut declared_callbacks_types = vec![];
    let mut declared_callbacks_ret = vec![];

    for property in component.properties.iter().filter(|p| p.use_count.get() > 0) {
        let prop_ident = ident(&property.name);
        if let Type::Callback { args, return_type } = &property.ty {
            let callback_args =
                args.iter().map(|a| rust_primitive_type(a).unwrap()).collect::<Vec<_>>();
            let return_type =
                return_type.as_ref().map_or(quote!(()), |a| rust_primitive_type(a).unwrap());
            declared_callbacks.push(prop_ident.clone());
            declared_callbacks_types.push(callback_args);
            declared_callbacks_ret.push(return_type);
        } else {
            let rust_property_type = rust_property_type(&property.ty).unwrap();
            declared_property_vars.push(prop_ident.clone());
            declared_property_types.push(rust_property_type.clone());
        }
    }

    let declared_functions = generate_functions(&component.functions, &ctx);

    let mut init = vec![];
    let mut item_names = vec![];
    let mut item_types = vec![];

    #[cfg(slint_debug_property)]
    init.push(quote!(
        #(self_rc.#declared_property_vars.debug_name.replace(
            concat!(stringify!(#inner_component_id), ".", stringify!(#declared_property_vars)).into());)*
    ));

    for item in &component.items {
        if item.is_flickable_viewport {
            continue;
        }
        item_names.push(ident(&item.name));
        item_types.push(ident(&item.ty.class_name));
        #[cfg(slint_debug_property)]
        {
            let mut it = Some(&item.ty);
            let elem_name = ident(&item.name);
            while let Some(ty) = it {
                for (prop, info) in &ty.properties {
                    if info.ty.is_property_type()
                        && !prop.starts_with("viewport")
                        && prop != "commands"
                    {
                        let name = format!("{}::{}.{}", component.name, item.name, prop);
                        let prop = ident(&prop);
                        init.push(
                            quote!(self_rc.#elem_name.#prop.debug_name.replace(#name.into());),
                        );
                    }
                }
                it = ty.parent.as_ref();
            }
        }
    }

    let mut repeated_element_names: Vec<Ident> = vec![];
    let mut repeated_visit_branch: Vec<TokenStream> = vec![];
    let mut repeated_element_components: Vec<Ident> = vec![];
    let mut repeated_subtree_ranges: Vec<TokenStream> = vec![];
    let mut repeated_subtree_components: Vec<TokenStream> = vec![];

    for (idx, repeated) in component.repeated.iter().enumerate() {
        extra_components.push(generate_repeated_component(
            repeated,
            root,
            ParentCtx::new(&ctx, Some(idx)),
        ));
        let repeater_id = format_ident!("repeater{}", idx);
        let rep_inner_component_id = self::inner_component_id(&repeated.sub_tree.root);

        let mut model = compile_expression(&repeated.model.borrow(), &ctx);
        if repeated.model.ty(&ctx) == Type::Bool {
            model = quote!(slint::private_unstable_api::re_exports::ModelRc::new(#model as bool))
        }

        init.push(quote! {
            _self.#repeater_id.set_model_binding({
                let self_weak = slint::private_unstable_api::re_exports::VRcMapped::downgrade(&self_rc);
                move || {
                    let self_rc = self_weak.upgrade().unwrap();
                    let _self = self_rc.as_pin_ref();
                    (#model) as _
                }
            });
        });
        let ensure_updated = if let Some(listview) = &repeated.listview {
            let vp_y = access_member(&listview.viewport_y, &ctx);
            let vp_h = access_member(&listview.viewport_height, &ctx);
            let lv_h = access_member(&listview.listview_height, &ctx);
            let vp_w = access_member(&listview.viewport_width, &ctx);
            let lv_w = access_member(&listview.listview_width, &ctx);

            quote! {
                #inner_component_id::FIELD_OFFSETS.#repeater_id.apply_pin(_self).ensure_updated_listview(
                    || { #rep_inner_component_id::new(_self.self_weak.get().unwrap().clone()).into() },
                    #vp_w, #vp_h, #vp_y, #lv_w.get(), #lv_h
                );
            }
        } else {
            quote! {
                #inner_component_id::FIELD_OFFSETS.#repeater_id.apply_pin(_self).ensure_updated(
                    || #rep_inner_component_id::new(_self.self_weak.get().unwrap().clone()).into()
                );
            }
        };
        repeated_visit_branch.push(quote!(
            #idx => {
                #ensure_updated
                _self.#repeater_id.visit(order, visitor)
            }
        ));
        repeated_subtree_ranges.push(quote!(
            #idx => {
                #ensure_updated
                slint::private_unstable_api::re_exports::IndexRange::from(_self.#repeater_id.range())
            }
        ));
        repeated_subtree_components.push(quote!(
            #idx => {
                #ensure_updated
                *result = vtable::VRc::downgrade(&vtable::VRc::into_dyn(_self.#repeater_id.component_at(subtree_index).unwrap()))
            }
        ));
        repeated_element_names.push(repeater_id);
        repeated_element_components.push(rep_inner_component_id);
    }

    let mut accessible_role_branch = vec![];
    let mut accessible_string_property_branch = vec![];
    for ((index, what), expr) in &component.accessible_prop {
        let expr = compile_expression(&expr.borrow(), &ctx);
        if what == "Role" {
            accessible_role_branch.push(quote!(#index => #expr,));
        } else {
            let what = ident(what);
            accessible_string_property_branch
                .push(quote!((#index, AccessibleStringProperty::#what) => #expr,));
        }
    }

    let mut user_init_code: Vec<TokenStream> = Vec::new();

    let mut sub_component_names: Vec<Ident> = vec![];
    let mut sub_component_types: Vec<Ident> = vec![];

    for sub in &component.sub_components {
        let field_name = ident(&sub.name);
        let sub_component_id = self::inner_component_id(&sub.ty);
        let local_tree_index: u32 = sub.index_in_tree as _;
        let local_index_of_first_child: u32 = sub.index_of_first_child_in_tree as _;
        let root_ref_tokens = &ctx.generator_state;

        // For children of sub-components, the item index generated by the generate_item_indices pass
        // starts at 1 (0 is the root element).
        let global_index = if local_tree_index == 0 {
            quote!(tree_index)
        } else {
            quote!(tree_index_of_first_child + #local_tree_index - 1)
        };
        let global_children = if local_index_of_first_child == 0 {
            quote!(0)
        } else {
            quote!(tree_index_of_first_child + #local_index_of_first_child - 1)
        };

        let sub_compo_field = access_component_field_offset(&format_ident!("Self"), &field_name);

        init.push(quote!(#sub_component_id::init(
            VRcMapped::map(self_rc.clone(), |x| #sub_compo_field.apply_pin(x)),
            &#root_ref_tokens,
            #global_index, #global_children
        );));
        user_init_code.push(quote!(#sub_component_id::user_init(
            VRcMapped::map(self_rc.clone(), |x| #sub_compo_field.apply_pin(x)),
        );));

        let sub_component_repeater_count = sub.ty.repeater_count();
        if sub_component_repeater_count > 0 {
            let repeater_offset = sub.repeater_offset;
            let last_repeater: usize = repeater_offset + sub_component_repeater_count - 1;
            repeated_visit_branch.push(quote!(
                #repeater_offset..=#last_repeater => {
                    #sub_compo_field.apply_pin(_self).visit_dynamic_children(dyn_index - #repeater_offset, order, visitor)
                }
            ));
            repeated_subtree_ranges.push(quote!(
                #repeater_offset..=#last_repeater => {
                    #sub_compo_field.apply_pin(_self).subtree_range(dyn_index - #repeater_offset)
                }
            ));
            repeated_subtree_components.push(quote!(
                #repeater_offset..=#last_repeater => {
                    #sub_compo_field.apply_pin(_self).subtree_component(dyn_index - #repeater_offset, subtree_index, result)
                }
            ));
        }

        let sub_items_count = sub.ty.child_item_count();
        let local_tree_index = local_tree_index as usize;
        accessible_role_branch.push(quote!(
            #local_tree_index => #sub_compo_field.apply_pin(_self).accessible_role(0),
        ));
        accessible_string_property_branch.push(quote!(
            (#local_tree_index, _) => #sub_compo_field.apply_pin(_self).accessible_string_property(0, what),
        ));
        if sub_items_count > 1 {
            let range_begin = local_index_of_first_child as usize;
            let range_end = range_begin + sub_items_count - 2;
            accessible_role_branch.push(quote!(
                #range_begin..=#range_end => #sub_compo_field.apply_pin(_self).accessible_role(index - #range_begin + 1),
            ));
            accessible_string_property_branch.push(quote!(
                (#range_begin..=#range_end, _) => #sub_compo_field.apply_pin(_self).accessible_string_property(index - #range_begin + 1, what),
            ));
        }

        sub_component_names.push(field_name);
        sub_component_types.push(sub_component_id);
    }

    for (prop1, prop2) in &component.two_way_bindings {
        let p1 = access_member(prop1, &ctx);
        let p2 = access_member(prop2, &ctx);
        init.push(quote!(
            Property::link_two_way(#p1, #p2);
        ));
    }

    for (prop, expression) in &component.property_init {
        if expression.use_count.get() > 0 {
            handle_property_init(prop, expression, &mut init, &ctx)
        }
    }
    for prop in &component.const_properties {
        if let llr::PropertyReference::Local { property_index, sub_component_path } = prop {
            let mut sc = component;
            for i in sub_component_path {
                sc = &sc.sub_components[*i].ty;
            }
            if sc.properties[*property_index].use_count.get() == 0 {
                continue;
            }
        }
        let rust_property = access_member(prop, &ctx);
        init.push(quote!(#rust_property.set_constant();))
    }

    let root_component_id = self::inner_component_id(&root.item_tree.root);

    let parent_component_type = parent_ctx.iter().map(|parent| {
        let parent_component_id = self::inner_component_id(parent.ctx.current_sub_component.unwrap());
        quote!(slint::private_unstable_api::re_exports::VWeakMapped::<slint::private_unstable_api::re_exports::ComponentVTable, #parent_component_id>)
    });

    user_init_code.extend(component.init_code.iter().map(|e| {
        let code = compile_expression(&e.borrow(), &ctx);
        quote!(#code;)
    }));

    let layout_info_h = compile_expression(&component.layout_info_h.borrow(), &ctx);
    let layout_info_v = compile_expression(&component.layout_info_v.borrow(), &ctx);

    // FIXME! this is only public because of the ComponentHandle::Inner. we should find another way
    let visibility =
        core::ptr::eq(&root.item_tree.root as *const _, component as *const _).then(|| quote!(pub));

    let subtree_index_function = if let Some(property_index) = index_property {
        let prop = access_member(
            &llr::PropertyReference::Local { sub_component_path: vec![], property_index },
            &ctx,
        );
        quote!(#prop.get() as usize)
    } else {
        quote!(core::usize::MAX)
    };

    let pin_macro = if pinned_drop { quote!(#[pin_drop]) } else { quote!(#[pin]) };

    quote!(
        #[derive(slint::private_unstable_api::re_exports::FieldOffsets, Default)]
        #[const_field_offset(slint::private_unstable_api::re_exports::const_field_offset)]
        #[repr(C)]
        #pin_macro
        #visibility
        struct #inner_component_id {
            #(#item_names : slint::private_unstable_api::re_exports::#item_types,)*
            #(#sub_component_names : #sub_component_types,)*
            #(#declared_property_vars : slint::private_unstable_api::re_exports::Property<#declared_property_types>,)*
            #(#declared_callbacks : slint::private_unstable_api::re_exports::Callback<(#(#declared_callbacks_types,)*), #declared_callbacks_ret>,)*
            #(#repeated_element_names : slint::private_unstable_api::re_exports::Repeater<#repeated_element_components>,)*
            self_weak : slint::private_unstable_api::re_exports::OnceCell<slint::private_unstable_api::re_exports::VWeakMapped<slint::private_unstable_api::re_exports::ComponentVTable, #inner_component_id>>,
            #(parent : #parent_component_type,)*
            // FIXME: Do we really need a window all the time?
            window_adapter: slint::private_unstable_api::re_exports::OnceCell<slint::private_unstable_api::re_exports::Rc<dyn slint::private_unstable_api::re_exports::WindowAdapter>>,
            root : slint::private_unstable_api::re_exports::OnceCell<slint::private_unstable_api::re_exports::VWeak<slint::private_unstable_api::re_exports::ComponentVTable, #root_component_id>>,
            tree_index: ::core::cell::Cell<u32>,
            tree_index_of_first_child: ::core::cell::Cell<u32>,
            #extra_fields
        }

        impl #inner_component_id {
            pub fn init(self_rc: slint::private_unstable_api::re_exports::VRcMapped<slint::private_unstable_api::re_exports::ComponentVTable, Self>,
                    root : &slint::private_unstable_api::re_exports::VRc<slint::private_unstable_api::re_exports::ComponentVTable, #root_component_id>,
                    tree_index: u32, tree_index_of_first_child: u32) {
                #![allow(unused)]
                #![allow(unused)]
                let _self = self_rc.as_pin_ref();
                _self.self_weak.set(VRcMapped::downgrade(&self_rc));
                _self.root.set(VRc::downgrade(root));
                _self.window_adapter.set(root.window_adapter.get().unwrap().clone());
                _self.tree_index.set(tree_index);
                _self.tree_index_of_first_child.set(tree_index_of_first_child);
                #(#init)*
            }

            pub fn user_init(self_rc: slint::private_unstable_api::re_exports::VRcMapped<slint::private_unstable_api::re_exports::ComponentVTable, Self>) {
                let _self = self_rc.as_pin_ref();
                #(#user_init_code)*
            }

            fn visit_dynamic_children(
                self: ::core::pin::Pin<&Self>,
                dyn_index: usize,
                order: slint::private_unstable_api::re_exports::TraversalOrder,
                visitor: slint::private_unstable_api::re_exports::ItemVisitorRefMut
            ) -> slint::private_unstable_api::re_exports::VisitChildrenResult {
                #![allow(unused)]
                let _self = self;
                match dyn_index {
                    #(#repeated_visit_branch)*
                    _ => panic!("invalid dyn_index {}", dyn_index),
                }
            }

            fn layout_info(self: ::core::pin::Pin<&Self>, orientation: slint::private_unstable_api::re_exports::Orientation) -> slint::private_unstable_api::re_exports::LayoutInfo {
                #![allow(unused)]
                let _self = self;
                match orientation {
                    slint::private_unstable_api::re_exports::Orientation::Horizontal => #layout_info_h,
                    slint::private_unstable_api::re_exports::Orientation::Vertical => #layout_info_v,
                }
            }

            fn subtree_range(self: ::core::pin::Pin<&Self>, dyn_index: usize) -> slint::private_unstable_api::re_exports::IndexRange {
                #![allow(unused)]
                let _self = self;
                match dyn_index {
                    #(#repeated_subtree_ranges)*
                    _ => panic!("invalid dyn_index {}", dyn_index),
                }
            }

            fn subtree_component(self: ::core::pin::Pin<&Self>, dyn_index: usize, subtree_index: usize, result: &mut slint::private_unstable_api::re_exports::ComponentWeak) {
                #![allow(unused)]
                let _self = self;
                match dyn_index {
                    #(#repeated_subtree_components)*
                    _ => panic!("invalid dyn_index {}", dyn_index),
                };
            }

            fn index_property(self: ::core::pin::Pin<&Self>) -> usize {
                #![allow(unused)]
                let _self = self;
                #subtree_index_function
            }

            fn accessible_role(self: ::core::pin::Pin<&Self>, index: usize) -> slint::private_unstable_api::re_exports::AccessibleRole {
                #![allow(unused)]
                let _self = self;
                match index {
                    #(#accessible_role_branch)*
                    //#(#forward_sub_ranges => #forward_sub_field.apply_pin(_self).accessible_role())*
                    _ => AccessibleRole::default(),
                }
            }

            fn accessible_string_property(
                self: ::core::pin::Pin<&Self>,
                index: usize,
                what: slint::private_unstable_api::re_exports::AccessibleStringProperty,
            ) -> slint::private_unstable_api::re_exports::SharedString {
                #![allow(unused)]
                let _self = self;
                match (index, what) {
                    #(#accessible_string_property_branch)*
                    _ => Default::default(),
                }
            }

            #(#declared_functions)*
        }

        #(#extra_components)*
    )
}

fn generate_functions(functions: &[llr::Function], ctx: &EvaluationContext) -> Vec<TokenStream> {
    functions
        .iter()
        .map(|f| {
            let mut ctx2 = ctx.clone();
            ctx2.argument_types = &f.args;
            let tokens_for_expression = compile_expression(&f.code, &ctx2);
            let as_ = if f.ret_ty == Type::Void {
                Some(quote!(;))
            } else if f.code.ty(&ctx2) == Type::Invalid {
                // Don't cast if the Rust code is the never type, as with return statements inside a block, the
                // type of the return expression is `()` instead of `!`.
                None
            } else {
                Some(quote!(as _))
            };
            let fn_id = ident(&format!("fn_{}", f.name));
            let args_ty =
                f.args.iter().map(|a| rust_primitive_type(a).unwrap()).collect::<Vec<_>>();
            let return_type = rust_primitive_type(&f.ret_ty).unwrap();
            let args_name =
                (0..f.args.len()).map(|i| format_ident!("arg_{}", i)).collect::<Vec<_>>();

            quote! {
                #[allow(dead_code, unused)]
                pub fn #fn_id(self: ::core::pin::Pin<&Self>, #(#args_name : #args_ty,)*) -> #return_type {
                    let _self = self;
                    let args = (#(#args_name,)*);
                    (#tokens_for_expression) #as_
                }
            }
        })
        .collect()
}

fn generate_global(global: &llr::GlobalComponent, root: &llr::PublicComponent) -> TokenStream {
    let mut declared_property_vars = vec![];
    let mut declared_property_types = vec![];
    let mut declared_callbacks = vec![];
    let mut declared_callbacks_types = vec![];
    let mut declared_callbacks_ret = vec![];

    for property in global.properties.iter().filter(|p| p.use_count.get() > 0) {
        let prop_ident = ident(&property.name);
        if let Type::Callback { args, return_type } = &property.ty {
            let callback_args =
                args.iter().map(|a| rust_primitive_type(a).unwrap()).collect::<Vec<_>>();
            let return_type =
                return_type.as_ref().map_or(quote!(()), |a| rust_primitive_type(a).unwrap());
            declared_callbacks.push(prop_ident.clone());
            declared_callbacks_types.push(callback_args);
            declared_callbacks_ret.push(return_type);
        } else {
            let rust_property_type = rust_property_type(&property.ty).unwrap();
            declared_property_vars.push(prop_ident.clone());
            declared_property_types.push(rust_property_type.clone());
        }
    }

    let mut init = vec![];
    let inner_component_id = format_ident!("Inner{}", ident(&global.name));

    #[cfg(slint_debug_property)]
    init.push(quote!(
        #(self_rc.#declared_property_vars.debug_name.replace(
            concat!(stringify!(#inner_component_id), ".", stringify!(#declared_property_vars)).into());)*
    ));

    let ctx = EvaluationContext::new_global(
        root,
        global,
        quote!(_self.root.get().unwrap().upgrade().unwrap()),
    );

    let declared_functions = generate_functions(&global.functions, &ctx);

    for (property_index, expression) in global.init_values.iter().enumerate() {
        if global.properties[property_index].use_count.get() == 0 {
            continue;
        }
        if let Some(expression) = expression.as_ref() {
            handle_property_init(
                &llr::PropertyReference::Local { sub_component_path: vec![], property_index },
                expression,
                &mut init,
                &ctx,
            )
        }
    }
    for (property_index, cst) in global.const_properties.iter().enumerate() {
        if global.properties[property_index].use_count.get() == 0 {
            continue;
        }
        if *cst {
            let rust_property = access_member(
                &llr::PropertyReference::Local { sub_component_path: vec![], property_index },
                &ctx,
            );
            init.push(quote!(#rust_property.set_constant();))
        }
    }

    let public_interface = global.exported.then(|| {
        let property_and_callback_accessors = public_api(&global.public_properties, &global.private_properties, quote!(self.0.as_ref()), &ctx);
        let public_component_id = ident(&global.name);
        let root_component_id = self::public_component_id(&root.item_tree.root);
        let global_id = format_ident!("global_{}", public_component_id);

        let aliases = global.aliases.iter().map(|name| ident(name));
        quote!(
            pub struct #public_component_id<'a>(&'a ::core::pin::Pin<slint::private_unstable_api::re_exports::Rc<#inner_component_id>>);

            impl<'a> #public_component_id<'a> {
                #property_and_callback_accessors
            }

            #(pub type #aliases<'a> = #public_component_id<'a>;)*

            impl<'a> slint::Global<'a, #root_component_id> for #public_component_id<'a> {
                fn get(component: &'a #root_component_id) -> Self {
                    Self(&component.0 .globals.#global_id)
                }
            }
        )
    });

    let root_component_id = self::inner_component_id(&root.item_tree.root);
    quote!(
        #[derive(slint::private_unstable_api::re_exports::FieldOffsets, Default)]
        #[const_field_offset(slint::private_unstable_api::re_exports::const_field_offset)]
        #[repr(C)]
        #[pin]
        struct #inner_component_id {
            #(#declared_property_vars: slint::private_unstable_api::re_exports::Property<#declared_property_types>,)*
            #(#declared_callbacks: slint::private_unstable_api::re_exports::Callback<(#(#declared_callbacks_types,)*), #declared_callbacks_ret>,)*
            root : slint::private_unstable_api::re_exports::OnceCell<slint::private_unstable_api::re_exports::VWeak<slint::private_unstable_api::re_exports::ComponentVTable, #root_component_id>>,
        }

        impl #inner_component_id {
            fn new() -> ::core::pin::Pin<slint::private_unstable_api::re_exports::Rc<Self>> {
                slint::private_unstable_api::re_exports::Rc::pin(Self::default())
            }
            fn init(self: ::core::pin::Pin<slint::private_unstable_api::re_exports::Rc<Self>>, root: &slint::private_unstable_api::re_exports::VRc<slint::private_unstable_api::re_exports::ComponentVTable, #root_component_id>) {
                #![allow(unused)]
                self.root.set(VRc::downgrade(root));
                let self_rc = self;
                let _self = self_rc.as_ref();
                #(#init)*
            }

            #(#declared_functions)*
        }

        #public_interface
    )
}

fn generate_item_tree(
    sub_tree: &llr::ItemTree,
    root: &llr::PublicComponent,
    parent_ctx: Option<ParentCtx>,
    extra_fields: TokenStream,
    index_property: Option<llr::PropertyIndex>,
) -> TokenStream {
    let sub_comp = generate_sub_component(
        &sub_tree.root,
        root,
        parent_ctx.clone(),
        extra_fields,
        index_property,
        true,
    );
    let inner_component_id = self::inner_component_id(&sub_tree.root);
    let parent_component_type = parent_ctx.iter().map(|parent| {
        let parent_component_id = self::inner_component_id(parent.ctx.current_sub_component.unwrap());
        quote!(slint::private_unstable_api::re_exports::VWeakMapped::<slint::private_unstable_api::re_exports::ComponentVTable, #parent_component_id>)
    }).collect::<Vec<_>>();
    let root_token = if parent_ctx.is_some() {
        quote!(&parent.upgrade().unwrap().root.get().unwrap().upgrade().unwrap())
    } else {
        quote!(&self_rc)
    };

    let (create_window_adapter, init_window, new_result, new_end) = if let Some(parent_ctx) =
        parent_ctx
    {
        (
            None,
            None,
            quote!(vtable::VRc<slint::private_unstable_api::re_exports::ComponentVTable, Self>),
            if parent_ctx.repeater_index.is_some() {
                // Repeaters run their user_init() code from RepeatedComponent::init() after update() initialized model_data/index.
                quote!(self_rc)
            } else {
                quote! {
                    Self::user_init(slint::private_unstable_api::re_exports::VRc::map(self_rc.clone(), |x| x));
                    self_rc
                }
            },
        )
    } else {
        (
            Some(
                quote!(let window_adapter = slint::private_unstable_api::create_window_adapter()?;),
            ),
            Some(quote! {
                _self.window_adapter.set(window_adapter);
                slint::private_unstable_api::re_exports::WindowInner::from_pub(_self.window_adapter.get().unwrap().window()).set_component(&VRc::into_dyn(self_rc.clone()));
            }),
            quote!(
                core::result::Result<
                    vtable::VRc<slint::private_unstable_api::re_exports::ComponentVTable, Self>,
                    slint::PlatformError,
                >
            ),
            quote!(core::result::Result::Ok(self_rc)),
        )
    };

    let parent_item_expression = parent_ctx.and_then(|parent| {
        parent.repeater_index.map(|idx| {
            let sub_component_offset = parent.ctx.current_sub_component.unwrap().repeated[idx].index_in_tree;

            quote!(if let Some((parent_component, parent_index)) = self
                .parent
                .clone()
                .upgrade()
                .map(|sc| (VRcMapped::origin(&sc), sc.tree_index_of_first_child.get()))
            {
                *_result = slint::private_unstable_api::re_exports::ItemRc::new(parent_component, parent_index as usize + #sub_component_offset - 1)
                    .downgrade();
            })
        })
    });
    let mut item_tree_array = vec![];
    let mut item_array = vec![];
    sub_tree.tree.visit_in_array(&mut |node, children_offset, parent_index| {
        let parent_index = parent_index as u32;
        let (path, component) = follow_sub_component_path(&sub_tree.root, &node.sub_component_path);
        if node.repeated {
            assert_eq!(node.children.len(), 0);
            let mut repeater_index = node.item_index;
            let mut sub_component = &sub_tree.root;
            for i in &node.sub_component_path {
                repeater_index += sub_component.sub_components[*i].repeater_offset;
                sub_component = &sub_component.sub_components[*i].ty;
            }
            item_tree_array.push(quote!(
                slint::private_unstable_api::re_exports::ItemTreeNode::DynamicTree {
                    index: #repeater_index,
                    parent_index: #parent_index,
                }
            ));
        } else {
            let item = &component.items[node.item_index];
            let flick = item
                .is_flickable_viewport
                .then(|| quote!(+ slint::private_unstable_api::re_exports::Flickable::FIELD_OFFSETS.viewport));

            let field = access_component_field_offset(
                &self::inner_component_id(component),
                &ident(&item.name),
            );

            let children_count = node.children.len() as u32;
            let children_index = children_offset as u32;
            let item_array_len = item_array.len() as u32;
            let is_accessible = node.is_accessible;
            item_tree_array.push(quote!(
                slint::private_unstable_api::re_exports::ItemTreeNode::Item {
                    is_accessible: #is_accessible,
                    children_count: #children_count,
                    children_index: #children_index,
                    parent_index: #parent_index,
                    item_array_index: #item_array_len,
                }
            ));
            item_array.push(quote!(VOffset::new(#path #field #flick)));
        }
    });

    let item_tree_array_len = item_tree_array.len();
    let item_array_len = item_array.len();

    quote!(
        #sub_comp

        impl #inner_component_id {
            pub fn new(#(parent: #parent_component_type)*) -> #new_result {
                #![allow(unused)]
                #create_window_adapter // We must create the window first to initialize the backend before using the style
                let mut _self = Self::default();
                #(_self.parent = parent.clone() as #parent_component_type;)*
                let self_rc = VRc::new(_self);
                let _self = self_rc.as_pin_ref();
                #init_window
                slint::private_unstable_api::re_exports::register_component(_self, Self::item_array(), #root_token.window_adapter.get().unwrap());
                Self::init(slint::private_unstable_api::re_exports::VRc::map(self_rc.clone(), |x| x), #root_token, 0, 1);
                #new_end
            }

            fn item_tree() -> &'static [slint::private_unstable_api::re_exports::ItemTreeNode] {
                const ITEM_TREE : [slint::private_unstable_api::re_exports::ItemTreeNode; #item_tree_array_len] = [#(#item_tree_array),*];
                &ITEM_TREE
            }

            fn item_array() -> &'static [vtable::VOffset<Self, ItemVTable, vtable::AllowPin>] {
                // FIXME: ideally this should be a const, but we can't because of the pointer to the vtable
                static ITEM_ARRAY : slint::private_unstable_api::re_exports::OnceBox<
                    [vtable::VOffset<#inner_component_id, ItemVTable, vtable::AllowPin>; #item_array_len]
                > = slint::private_unstable_api::re_exports::OnceBox::new();
                &*ITEM_ARRAY.get_or_init(|| Box::new([#(#item_array),*]))
            }
        }

        impl slint::private_unstable_api::re_exports::PinnedDrop for #inner_component_id {
            fn drop(self: core::pin::Pin<&mut #inner_component_id>) {
                use slint::private_unstable_api::re_exports::*;
                ComponentVTable_static!(static VT for self::#inner_component_id);
                new_vref!(let vref : VRef<ComponentVTable> for Component = self.as_ref().get_ref());
                slint::private_unstable_api::re_exports::unregister_component(self.as_ref(), vref, Self::item_array(), self.window_adapter.get().unwrap());
            }
        }

        impl slint::private_unstable_api::re_exports::Component for #inner_component_id {
            fn visit_children_item(self: ::core::pin::Pin<&Self>, index: isize, order: slint::private_unstable_api::re_exports::TraversalOrder, visitor: slint::private_unstable_api::re_exports::ItemVisitorRefMut)
                -> slint::private_unstable_api::re_exports::VisitChildrenResult
            {
                return slint::private_unstable_api::re_exports::visit_item_tree(self, &VRcMapped::origin(&self.as_ref().self_weak.get().unwrap().upgrade().unwrap()), self.get_item_tree().as_slice(), index, order, visitor, visit_dynamic);
                #[allow(unused)]
                fn visit_dynamic(_self: ::core::pin::Pin<&#inner_component_id>, order: slint::private_unstable_api::re_exports::TraversalOrder, visitor: ItemVisitorRefMut, dyn_index: usize) -> VisitChildrenResult  {
                    _self.visit_dynamic_children(dyn_index, order, visitor)
                }
            }

            fn get_item_ref(self: ::core::pin::Pin<&Self>, index: usize) -> ::core::pin::Pin<ItemRef> {
                match &self.get_item_tree().as_slice()[index] {
                    ItemTreeNode::Item { item_array_index, .. } => {
                        Self::item_array()[*item_array_index as usize].apply_pin(self)
                    }
                    ItemTreeNode::DynamicTree { .. } => panic!("get_item_ref called on dynamic tree"),

                }
            }

            fn get_item_tree(
                self: ::core::pin::Pin<&Self>) -> slint::private_unstable_api::re_exports::Slice<slint::private_unstable_api::re_exports::ItemTreeNode>
            {
                Self::item_tree().into()
            }

            fn get_subtree_range(
                self: ::core::pin::Pin<&Self>, index: usize) -> slint::private_unstable_api::re_exports::IndexRange
            {
                self.subtree_range(index)
            }

            fn get_subtree_component(
                self: ::core::pin::Pin<&Self>, index: usize, subtree_index: usize, result: &mut slint::private_unstable_api::re_exports::ComponentWeak)
            {
                self.subtree_component(index, subtree_index, result);
            }

            fn subtree_index(
                self: ::core::pin::Pin<&Self>) -> usize
            {
                self.index_property()
            }

            fn parent_node(self: ::core::pin::Pin<&Self>, _result: &mut slint::private_unstable_api::re_exports::ItemWeak) {
                #parent_item_expression
            }

            fn layout_info(self: ::core::pin::Pin<&Self>, orientation: slint::private_unstable_api::re_exports::Orientation) -> slint::private_unstable_api::re_exports::LayoutInfo {
                self.layout_info(orientation)
            }

            fn accessible_role(self: ::core::pin::Pin<&Self>, index: usize) -> slint::private_unstable_api::re_exports::AccessibleRole {
                self.accessible_role(index)
            }

            fn accessible_string_property(
                self: ::core::pin::Pin<&Self>,
                index: usize,
                what: slint::private_unstable_api::re_exports::AccessibleStringProperty,
                result: &mut slint::private_unstable_api::re_exports::SharedString,
            ) {
                *result = self.accessible_string_property(index, what);
            }
        }


    )
}

fn generate_repeated_component(
    repeated: &llr::RepeatedElement,
    root: &llr::PublicComponent,
    parent_ctx: ParentCtx,
) -> TokenStream {
    let component = generate_item_tree(
        &repeated.sub_tree,
        root,
        Some(parent_ctx.clone()),
        quote!(),
        repeated.index_prop,
    );

    let ctx = EvaluationContext {
        public_component: root,
        current_sub_component: Some(&repeated.sub_tree.root),
        current_global: None,
        generator_state: quote!(_self),
        parent: Some(parent_ctx),
        argument_types: &[],
    };

    let inner_component_id = self::inner_component_id(&repeated.sub_tree.root);

    // let rep_inner_component_id = self::inner_component_id(&repeated.sub_tree.root.name);
    // let inner_component_id = self::inner_component_id(&parent_compo);

    let extra_fn = if let Some(listview) = &repeated.listview {
        let p_y = access_member(&listview.prop_y, &ctx);
        let p_height = access_member(&listview.prop_height, &ctx);
        let p_width = access_member(&listview.prop_width, &ctx);
        quote! {
            fn listview_layout(
                self: core::pin::Pin<&Self>,
                offset_y: &mut slint::private_unstable_api::re_exports::LogicalLength,
                viewport_width: core::pin::Pin<&slint::private_unstable_api::re_exports::Property<slint::private_unstable_api::re_exports::LogicalLength>>,
            ) {
                let _self = self;
                let vp_w = viewport_width.get();
                #p_y.set(*offset_y);
                *offset_y += #p_height.get();
                let w = #p_width.get();
                if vp_w < w {
                    viewport_width.set(w);
                }
            }
        }
    } else {
        // TODO: we could generate this code only if we know that this component is in a box layout
        quote! {
            fn box_layout_data(self: ::core::pin::Pin<&Self>, o: slint::private_unstable_api::re_exports::Orientation)
                -> slint::private_unstable_api::re_exports::BoxLayoutCellData
            {
                BoxLayoutCellData { constraint: self.as_ref().layout_info(o) }
            }
        }
    };

    let data_type = if let Some(data_prop) = repeated.data_prop {
        rust_primitive_type(&repeated.sub_tree.root.properties[data_prop].ty).unwrap()
    } else {
        quote!(())
    };

    let access_prop = |property_index| {
        access_member(
            &llr::PropertyReference::Local { sub_component_path: vec![], property_index },
            &ctx,
        )
    };
    let index_prop = repeated.index_prop.into_iter().map(access_prop);
    let set_data_expr = repeated.data_prop.into_iter().map(|property_index| {
        let prop_type = ctx.property_ty(&llr::PropertyReference::Local {
            sub_component_path: vec![],
            property_index,
        });
        let data_prop = access_prop(property_index);
        let value_tokens = set_primitive_property_value(prop_type, quote!(_data));
        quote!(#data_prop.set(#value_tokens);)
    });

    quote!(
        #component

        impl slint::private_unstable_api::re_exports::RepeatedComponent for #inner_component_id {
            type Data = #data_type;
            fn update(&self, _index: usize, _data: Self::Data) {
                let self_rc = self.self_weak.get().unwrap().upgrade().unwrap();
                let _self = self_rc.as_pin_ref();
                #(#index_prop.set(_index as _);)*
                #(#set_data_expr)*
            }
            fn init(&self) {
                let self_rc = self.self_weak.get().unwrap().upgrade().unwrap();
                #inner_component_id::user_init(
                    VRcMapped::map(self_rc, |x| x),
                );
            }
            #extra_fn
        }
    )
}

/// Return an identifier suitable for this component for internal use
fn inner_component_id(component: &llr::SubComponent) -> proc_macro2::Ident {
    format_ident!("Inner{}", ident(&component.name))
}

fn global_inner_name(g: &llr::GlobalComponent) -> proc_macro2::Ident {
    if g.is_builtin {
        ident(&g.name)
    } else {
        format_ident!("Inner{}", ident(&g.name))
    }
}

/// Return an identifier suitable for this component for the developer facing API
fn public_component_id(component: &llr::SubComponent) -> proc_macro2::Ident {
    ident(&component.name)
}

fn property_set_value_tokens(
    property: &llr::PropertyReference,
    value_tokens: TokenStream,
    ctx: &EvaluationContext,
) -> TokenStream {
    let prop = access_member(property, ctx);
    let prop_type = ctx.property_ty(property);
    let value_tokens = set_primitive_property_value(prop_type, value_tokens);
    if let Some(animation) = ctx.current_sub_component.and_then(|c| c.animations.get(property)) {
        let animation_tokens = compile_expression(animation, ctx);
        return quote!(#prop.set_animated_value(#value_tokens as _, #animation_tokens));
    }
    quote!(#prop.set(#value_tokens as _))
}

/// Returns the code that can access the given property or callback (but without the set or get)
///
/// to be used like:
/// ```ignore
/// let access = access_member(...)
/// quote!(#access.get())
/// ```
///
/// Or for functions:
///
/// ```ignore
/// let access = access_member(...)
/// quote!(#access(arg1, arg2))
/// ```

fn access_member(reference: &llr::PropertyReference, ctx: &EvaluationContext) -> TokenStream {
    fn in_native_item(
        ctx: &EvaluationContext,
        sub_component_path: &[usize],
        item_index: usize,
        prop_name: &str,
        path: TokenStream,
    ) -> TokenStream {
        let (compo_path, sub_component) =
            follow_sub_component_path(ctx.current_sub_component.unwrap(), sub_component_path);
        let component_id = inner_component_id(sub_component);
        let item_name = ident(&sub_component.items[item_index].name);
        let item_field = access_component_field_offset(&component_id, &item_name);
        if prop_name.is_empty() {
            // then this is actually a reference to the element itself
            quote!((#compo_path #item_field).apply_pin(_self))
        } else {
            let property_name = ident(prop_name);
            let item_ty = ident(&sub_component.items[item_index].ty.class_name);
            let flick = sub_component.items[item_index]
                .is_flickable_viewport
                .then(|| quote!(+ slint::private_unstable_api::re_exports::Flickable::FIELD_OFFSETS.viewport));
            quote!((#compo_path #item_field #flick + #item_ty::FIELD_OFFSETS.#property_name).apply_pin(#path))
        }
    }
    match reference {
        llr::PropertyReference::Local { sub_component_path, property_index } => {
            if let Some(sub_component) = ctx.current_sub_component {
                let (compo_path, sub_component) =
                    follow_sub_component_path(sub_component, sub_component_path);
                let component_id = inner_component_id(sub_component);
                let property_name = ident(&sub_component.properties[*property_index].name);
                let property_field = access_component_field_offset(&component_id, &property_name);
                quote!((#compo_path #property_field).apply_pin(_self))
            } else if let Some(current_global) = ctx.current_global {
                let global_name = global_inner_name(current_global);
                let property_name = ident(&current_global.properties[*property_index].name);
                let property_field = access_component_field_offset(&global_name, &property_name);
                quote!(#property_field.apply_pin(_self))
            } else {
                unreachable!()
            }
        }
        llr::PropertyReference::InNativeItem { sub_component_path, item_index, prop_name } => {
            in_native_item(ctx, sub_component_path, *item_index, prop_name, quote!(_self))
        }
        llr::PropertyReference::InParent { level, parent_reference } => {
            let mut ctx = ctx;
            let mut path = quote!(_self);
            for _ in 0..level.get() {
                path = quote!(#path.parent.upgrade().unwrap().as_pin_ref());
                ctx = ctx.parent.as_ref().unwrap().ctx;
            }

            match &**parent_reference {
                llr::PropertyReference::Local { sub_component_path, property_index } => {
                    let sub_component = ctx.current_sub_component.unwrap();
                    let (compo_path, sub_component) =
                        follow_sub_component_path(sub_component, sub_component_path);
                    let component_id = inner_component_id(sub_component);
                    let property_name = ident(&sub_component.properties[*property_index].name);
                    quote!((#compo_path #component_id::FIELD_OFFSETS.#property_name).apply_pin(#path))
                }
                llr::PropertyReference::InNativeItem {
                    sub_component_path,
                    item_index,
                    prop_name,
                } => in_native_item(ctx, sub_component_path, *item_index, prop_name, path),
                llr::PropertyReference::Function { sub_component_path, function_index } => {
                    let mut sub_component = ctx.current_sub_component.unwrap();

                    let mut compo_path = path;
                    for i in sub_component_path {
                        let component_id = inner_component_id(sub_component);
                        let sub_component_name = ident(&sub_component.sub_components[*i].name);
                        compo_path = quote!( #component_id::FIELD_OFFSETS.#sub_component_name.apply_pin(#compo_path));
                        sub_component = &sub_component.sub_components[*i].ty;
                    }
                    let fn_id =
                        ident(&format!("fn_{}", sub_component.functions[*function_index].name));
                    quote!(#compo_path.#fn_id)
                }
                llr::PropertyReference::InParent { .. }
                | llr::PropertyReference::Global { .. }
                | llr::PropertyReference::GlobalFunction { .. } => {
                    unreachable!()
                }
            }
        }
        llr::PropertyReference::Global { global_index, property_index } => {
            let root_access = &ctx.generator_state;
            let global = &ctx.public_component.globals[*global_index];
            let global_id = format_ident!("global_{}", ident(&global.name));
            let global_name = global_inner_name(global);
            let property_name = ident(
                &ctx.public_component.globals[*global_index].properties[*property_index].name,
            );
            quote!(#global_name::FIELD_OFFSETS.#property_name.apply_pin(#root_access.globals.#global_id.as_ref()))
        }
        llr::PropertyReference::Function { sub_component_path, function_index } => {
            if let Some(mut sub_component) = ctx.current_sub_component {
                let mut compo_path = quote!(_self);
                for i in sub_component_path {
                    let component_id = inner_component_id(sub_component);
                    let sub_component_name = ident(&sub_component.sub_components[*i].name);
                    compo_path = quote!( #component_id::FIELD_OFFSETS.#sub_component_name.apply_pin(#compo_path));
                    sub_component = &sub_component.sub_components[*i].ty;
                }
                let fn_id = ident(&format!("fn_{}", sub_component.functions[*function_index].name));
                quote!(#compo_path.#fn_id)
            } else if let Some(current_global) = ctx.current_global {
                let fn_id =
                    ident(&format!("fn_{}", current_global.functions[*function_index].name));
                quote!(_self.#fn_id)
            } else {
                unreachable!()
            }
        }
        llr::PropertyReference::GlobalFunction { global_index, function_index } => {
            let root_access = &ctx.generator_state;
            let global = &ctx.public_component.globals[*global_index];
            let global_id = format_ident!("global_{}", ident(&global.name));
            let fn_id = ident(&format!(
                "fn_{}",
                ctx.public_component.globals[*global_index].functions[*function_index].name
            ));
            quote!(#root_access.globals.#global_id.as_ref().#fn_id)
        }
    }
}

fn follow_sub_component_path<'a>(
    root: &'a llr::SubComponent,
    sub_component_path: &[usize],
) -> (TokenStream, &'a llr::SubComponent) {
    let mut compo_path = quote!();
    let mut sub_component = root;
    for i in sub_component_path {
        let component_id = inner_component_id(sub_component);
        let sub_component_name = ident(&sub_component.sub_components[*i].name);
        compo_path = quote!(#compo_path {#component_id::FIELD_OFFSETS.#sub_component_name} +);
        sub_component = &sub_component.sub_components[*i].ty;
    }
    (compo_path, sub_component)
}

fn access_window_adapter_field(ctx: &EvaluationContext) -> TokenStream {
    let root = &ctx.generator_state;
    quote!(#root.window_adapter.get().unwrap())
}

/// Given a property reference to a native item (eg, the property name is empty)
/// return tokens to the `ItemRc`
fn access_item_rc(pr: &llr::PropertyReference, ctx: &EvaluationContext) -> TokenStream {
    let mut ctx = ctx;
    let mut component_access_tokens = quote!(_self);

    let pr = match pr {
        llr::PropertyReference::InParent { level, parent_reference } => {
            for _ in 0..level.get() {
                component_access_tokens =
                    quote!(#component_access_tokens.parent.upgrade().unwrap().as_pin_ref());
                ctx = ctx.parent.as_ref().unwrap().ctx;
            }
            parent_reference
        }
        other => other,
    };

    match pr {
        llr::PropertyReference::InNativeItem { sub_component_path, item_index, prop_name } => {
            assert!(prop_name.is_empty());
            let (sub_compo_path, sub_component) =
                follow_sub_component_path(ctx.current_sub_component.unwrap(), sub_component_path);
            component_access_tokens = quote!(#component_access_tokens #sub_compo_path);
            let component_rc_tokens = quote!(VRcMapped::origin(&#component_access_tokens.self_weak.get().unwrap().upgrade().unwrap()));
            let item_index_in_tree = sub_component.items[*item_index].index_in_tree;
            let item_index_tokens = if item_index_in_tree == 0 {
                quote!(#component_access_tokens.tree_index.get() as usize)
            } else {
                quote!(#component_access_tokens.tree_index_of_first_child.get() as usize + #item_index_in_tree - 1)
            };

            quote!(&ItemRc::new(#component_rc_tokens, #item_index_tokens))
        }
        _ => unreachable!(),
    }
}

fn compile_expression(expr: &Expression, ctx: &EvaluationContext) -> TokenStream {
    match expr {
        Expression::StringLiteral(s) => {
            quote!(slint::private_unstable_api::re_exports::SharedString::from(#s))
        }
        Expression::NumberLiteral(n) => quote!(#n),
        Expression::BoolLiteral(b) => quote!(#b),
        Expression::Cast { from, to } => {
            let f = compile_expression(&*from, ctx);
            match (from.ty(ctx), to) {
                (from, Type::String) if from.as_unit_product().is_some() => {
                    quote!(slint::private_unstable_api::re_exports::SharedString::from(
                        slint::private_unstable_api::re_exports::format!("{}", #f).as_str()
                    ))
                }
                (Type::Float32, Type::Model) | (Type::Int32, Type::Model) => {
                    quote!(slint::private_unstable_api::re_exports::ModelRc::new(#f.max(Default::default()) as usize))
                }
                (Type::Float32, Type::Color) => {
                    quote!(slint::private_unstable_api::re_exports::Color::from_argb_encoded(#f as u32))
                }
                (Type::Color, Type::Brush) => {
                    quote!(slint::Brush::SolidColor(#f))
                }
                (Type::Brush, Type::Color) => {
                    quote!(#f.color())
                }
                (Type::Struct { ref fields, .. }, Type::Struct { name: Some(n), .. }) => {
                    let fields = fields.iter().enumerate().map(|(index, (name, _))| {
                        let index = proc_macro2::Literal::usize_unsuffixed(index);
                        let name = ident(name);
                        quote!(the_struct.#name =  obj.#index as _;)
                    });
                    let id = struct_name_to_tokens(n);
                    quote!({ let obj = #f; let mut the_struct = #id::default(); #(#fields)* the_struct })
                }
                (Type::Array(..), Type::PathData)
                    if matches!(
                        from.as_ref(),
                        Expression::Array { element_ty: Type::Struct { .. }, .. }
                    ) =>
                {
                    let path_elements = match from.as_ref() {
                        Expression::Array { element_ty: _, values, as_model: _ } => values
                            .iter()
                            .map(|path_elem_expr|
                                // Close{} is a struct with no fields in markup, and PathElement::Close has no fields, so map to an empty token stream
                                // and thus later just unit type, which can convert into PathElement::Close.
                                if matches!(path_elem_expr, Expression::Struct { ty: Type::Struct { fields, .. }, .. } if fields.is_empty()) {
                                    Default::default()
                                } else {
                                    compile_expression(path_elem_expr, ctx)
                                }
                            ),
                        _ => {
                            unreachable!()
                        }
                    };
                    quote!(slint::private_unstable_api::re_exports::PathData::Elements(slint::private_unstable_api::re_exports::SharedVector::<_>::from_slice(&[#((#path_elements).into()),*])))
                }
                (Type::Struct { .. }, Type::PathData)
                    if matches!(
                        from.as_ref(),
                        Expression::Struct { ty: Type::Struct { .. }, .. }
                    ) =>
                {
                    let (events, points) = match from.as_ref() {
                        Expression::Struct { ty: _, values } => (
                            compile_expression(&values["events"], ctx),
                            compile_expression(&values["points"], ctx),
                        ),
                        _ => {
                            unreachable!()
                        }
                    };
                    quote!(slint::private_unstable_api::re_exports::PathData::Events(slint::private_unstable_api::re_exports::SharedVector::<_>::from_slice(&#events), slint::private_unstable_api::re_exports::SharedVector::<_>::from_slice(&#points)))
                }
                (Type::String, Type::PathData) => {
                    quote!(slint::private_unstable_api::re_exports::PathData::Commands(#f))
                }
                _ => f,
            }
        }
        Expression::PropertyReference(nr) => {
            let access = access_member(nr, ctx);
            let prop_type = ctx.property_ty(nr);
            primitive_property_value(prop_type, access)
        }
        Expression::BuiltinFunctionCall { function, arguments } => {
            compile_builtin_function_call(function.clone(), arguments, ctx)
        }
        Expression::CallBackCall { callback, arguments } => {
            let f = access_member(callback, ctx);
            let a = arguments.iter().map(|a| compile_expression(a, ctx));
            quote! { #f.call(&(#(#a as _,)*).into())}
        }
        Expression::FunctionCall { function, arguments } => {
            let a = arguments.iter().map(|a| compile_expression(a, ctx));
            let access_function = access_member(function, ctx);
            quote! { #access_function( #(#a as _),*) }
        }

        Expression::ExtraBuiltinFunctionCall { function, arguments, return_ty: _ } => {
            let f = ident(function);
            let a = arguments.iter().map(|a| {
                let arg = compile_expression(a, ctx);
                if matches!(a.ty(ctx), Type::Struct { .. }) {
                    quote!(&#arg)
                } else {
                    arg
                }
            });
            quote! { #f(#(#a as _),*) }
        }
        Expression::FunctionParameterReference { index } => {
            let i = proc_macro2::Literal::usize_unsuffixed(*index);
            quote! {args.#i.clone()}
        }
        Expression::StructFieldAccess { base, name } => match base.ty(ctx) {
            Type::Struct { fields, name: None, .. } => {
                let index = fields
                    .keys()
                    .position(|k| k == name)
                    .expect("Expression::StructFieldAccess: Cannot find a key in an object");
                let index = proc_macro2::Literal::usize_unsuffixed(index);
                let base_e = compile_expression(base, ctx);
                quote!((#base_e).#index )
            }
            Type::Struct { .. } => {
                let name = ident(name);
                let base_e = compile_expression(base, ctx);
                quote!((#base_e).#name)
            }
            _ => panic!("Expression::StructFieldAccess's base expression is not an Object type"),
        },
        Expression::ArrayIndex { array, index } => {
            debug_assert!(matches!(array.ty(ctx), Type::Array(_)));
            let base_e = compile_expression(array, ctx);
            let index_e = compile_expression(index, ctx);
            quote!(match &#base_e { x => {
                let index = (#index_e) as usize;
                x.row_data_tracked(index).unwrap_or_default()
            }})
        }
        Expression::CodeBlock(sub) => {
            let map = sub.iter().map(|e| compile_expression(e, ctx));
            quote!({ #(#map);* })
        }
        Expression::PropertyAssignment { property, value } => {
            let value = compile_expression(value, ctx);
            property_set_value_tokens(property, value, ctx)
        }
        Expression::ModelDataAssignment { level, value } => {
            let value = compile_expression(value, ctx);
            let mut path = quote!(_self);
            let mut ctx2 = ctx;
            let mut repeater_index = None;
            for _ in 0..=*level {
                let x = ctx2.parent.clone().unwrap();
                ctx2 = x.ctx;
                repeater_index = x.repeater_index;
                path = quote!(#path.parent.upgrade().unwrap());
            }
            let repeater_index = repeater_index.unwrap();
            let mut index_prop = llr::PropertyReference::Local {
                sub_component_path: vec![],
                property_index: ctx2.current_sub_component.unwrap().repeated[repeater_index]
                    .index_prop
                    .unwrap(),
            };
            if let Some(level) = NonZeroUsize::new(*level) {
                index_prop =
                    llr::PropertyReference::InParent { level, parent_reference: index_prop.into() };
            }
            let index_access = access_member(&index_prop, ctx);
            let repeater = access_component_field_offset(
                &inner_component_id(ctx2.current_sub_component.unwrap()),
                &format_ident!("repeater{}", repeater_index),
            );
            quote!(#repeater.apply_pin(#path.as_pin_ref()).model_set_row_data(#index_access.get() as _, #value as _))
        }
        Expression::ArrayIndexAssignment { array, index, value } => {
            debug_assert!(matches!(array.ty(ctx), Type::Array(_)));
            let base_e = compile_expression(array, ctx);
            let index_e = compile_expression(index, ctx);
            let value_e = compile_expression(value, ctx);
            quote!((#base_e).set_row_data(#index_e as usize, #value_e as _))
        }
        Expression::BinaryExpression { lhs, rhs, op } => {
            let (conv1, conv2) = match crate::expression_tree::operator_class(*op) {
                OperatorClass::ArithmeticOp => match lhs.ty(ctx) {
                    Type::String => (None, Some(quote!(.as_str()))),
                    Type::Struct { .. } => (None, None),
                    _ => (Some(quote!(as f64)), Some(quote!(as f64))),
                },
                OperatorClass::ComparisonOp
                    if matches!(
                        lhs.ty(ctx),
                        Type::Int32
                            | Type::Float32
                            | Type::Duration
                            | Type::PhysicalLength
                            | Type::LogicalLength
                            | Type::Angle
                            | Type::Percent
                            | Type::Rem
                    ) =>
                {
                    (Some(quote!(as f64)), Some(quote!(as f64)))
                }
                _ => (None, None),
            };
            let lhs = compile_expression(&*lhs, ctx);
            let rhs = compile_expression(&*rhs, ctx);

            let op = match op {
                '=' => quote!(==),
                '!' => quote!(!=),
                '≤' => quote!(<=),
                '≥' => quote!(>=),
                '&' => quote!(&&),
                '|' => quote!(||),
                _ => proc_macro2::TokenTree::Punct(proc_macro2::Punct::new(
                    *op,
                    proc_macro2::Spacing::Alone,
                ))
                .into(),
            };
            quote!( ((#lhs #conv1 ) #op (#rhs #conv2)) )
        }
        Expression::UnaryOp { sub, op } => {
            let sub = compile_expression(&*sub, ctx);
            if *op == '+' {
                // there is no unary '+' in rust
                return sub;
            }
            let op = proc_macro2::Punct::new(*op, proc_macro2::Spacing::Alone);
            quote!( #op #sub )
        }
        Expression::ImageReference { resource_ref, .. } => match resource_ref {
            crate::expression_tree::ImageReference::None => {
                quote!(slint::private_unstable_api::re_exports::Image::default())
            }
            crate::expression_tree::ImageReference::AbsolutePath(path) => {
                quote!(slint::private_unstable_api::re_exports::Image::load_from_path(::std::path::Path::new(#path)).unwrap())
            }
            crate::expression_tree::ImageReference::EmbeddedData { resource_id, extension } => {
                let symbol = format_ident!("SLINT_EMBEDDED_RESOURCE_{}", resource_id);
                let format = proc_macro2::Literal::byte_string(extension.as_bytes());
                quote!(slint::private_unstable_api::re_exports::load_image_from_embedded_data(#symbol.into(), Slice::from_slice(#format)))
            }
            crate::expression_tree::ImageReference::EmbeddedTexture { resource_id } => {
                let symbol = format_ident!("SLINT_EMBEDDED_RESOURCE_{}", resource_id);
                quote!(
                    slint::private_unstable_api::re_exports::Image::from(slint::private_unstable_api::re_exports::ImageInner::StaticTextures(&#symbol))
                )
            }
        },
        Expression::Condition { condition, true_expr, false_expr } => {
            let condition_code = compile_expression(&*condition, ctx);
            let true_code = compile_expression(&*true_expr, ctx);
            let false_code = compile_expression(false_expr, ctx);
            quote!(
                if #condition_code {
                    #true_code
                } else {
                    (#false_code) as _
                }
            )
        }
        Expression::Array { values, element_ty, as_model } => {
            let val = values.iter().map(|e| compile_expression(e, ctx));
            if *as_model {
                let rust_element_ty = rust_primitive_type(element_ty).unwrap();
                quote!(slint::private_unstable_api::re_exports::ModelRc::new(
                    slint::private_unstable_api::re_exports::VecModel::<#rust_element_ty>::from(
                        slint::private_unstable_api::re_exports::vec![#(#val as _),*]
                    )
                ))
            } else {
                quote!(Slice::from_slice(&[#(#val),*]))
            }
        }
        Expression::Struct { ty, values } => {
            if let Type::Struct { fields, name, .. } = ty {
                let elem = fields.keys().map(|k| values.get(k).map(|e| compile_expression(e, ctx)));
                if let Some(name) = name {
                    let name_tokens: TokenStream = struct_name_to_tokens(name.as_str());
                    let keys = fields.keys().map(|k| ident(k));
                    if name.contains("LayoutData") {
                        quote!(#name_tokens{#(#keys: #elem as _,)*})
                    } else {
                        quote!({ let mut the_struct = #name_tokens::default(); #(the_struct.#keys =  #elem as _;)* the_struct})
                    }
                } else {
                    let as_ = fields.values().map(|t| {
                        if t.as_unit_product().is_some() {
                            // number needs to be converted to the right things because intermediate
                            // result might be f64 and that's usually not what the type of the tuple is in the end
                            let t = rust_primitive_type(t).unwrap();
                            quote!(as #t)
                        } else {
                            quote!()
                        }
                    });
                    // This will produce a tuple
                    quote!((#(#elem #as_,)*))
                }
            } else {
                panic!("Expression::Struct is not a Type::Struct")
            }
        }

        Expression::StoreLocalVariable { name, value } => {
            let value = compile_expression(value, ctx);
            let name = ident(name);
            quote!(let #name = #value;)
        }
        Expression::ReadLocalVariable { name, .. } => {
            let name = ident(name);
            quote!(#name.clone())
        }
        Expression::EasingCurve(EasingCurve::Linear) => {
            quote!(slint::private_unstable_api::re_exports::EasingCurve::Linear)
        }
        Expression::EasingCurve(EasingCurve::CubicBezier(a, b, c, d)) => {
            quote!(slint::private_unstable_api::re_exports::EasingCurve::CubicBezier([#a, #b, #c, #d]))
        }
        Expression::LinearGradient { angle, stops } => {
            let angle = compile_expression(angle, ctx);
            let stops = stops.iter().map(|(color, stop)| {
                let color = compile_expression(color, ctx);
                let position = compile_expression(stop, ctx);
                quote!(slint::private_unstable_api::re_exports::GradientStop{ color: #color, position: #position as _ })
            });
            quote!(slint::Brush::LinearGradient(
                slint::private_unstable_api::re_exports::LinearGradientBrush::new(#angle as _, [#(#stops),*])
            ))
        }
        Expression::RadialGradient { stops } => {
            let stops = stops.iter().map(|(color, stop)| {
                let color = compile_expression(color, ctx);
                let position = compile_expression(stop, ctx);
                quote!(slint::private_unstable_api::re_exports::GradientStop{ color: #color, position: #position as _ })
            });
            quote!(slint::Brush::RadialGradient(
                slint::private_unstable_api::re_exports::RadialGradientBrush::new_circle([#(#stops),*])
            ))
        }
        Expression::EnumerationValue(value) => {
            let base_ident = ident(&value.enumeration.name);
            let value_ident = ident(&value.to_pascal_case());
            quote!(slint::private_unstable_api::re_exports::#base_ident::#value_ident)
        }
        Expression::ReturnStatement(expr) => {
            let return_expr = expr.as_ref().map(|expr| compile_expression(expr, ctx));
            quote!(return (#return_expr) as _;)
        }
        Expression::LayoutCacheAccess { layout_cache_prop, index, repeater_index } => {
            let cache = access_member(layout_cache_prop, ctx);
            if let Some(ri) = repeater_index {
                let offset = compile_expression(ri, ctx);
                quote!({
                    let cache = #cache.get();
                    *cache.get((cache[#index] as usize) + #offset as usize * 2).unwrap_or(&(0 as slint::private_unstable_api::re_exports::Coord))
                })
            } else {
                quote!(#cache.get()[#index])
            }
        }
        Expression::BoxLayoutFunction {
            cells_variable,
            repeater_indices,
            elements,
            orientation,
            sub_expression,
        } => box_layout_function(
            cells_variable,
            repeater_indices.as_ref().map(String::as_str),
            elements,
            *orientation,
            sub_expression,
            ctx,
        ),
        Expression::ComputeDialogLayoutCells { cells_variable, roles, unsorted_cells } => {
            let cells_variable = ident(cells_variable);
            let roles = compile_expression(roles, ctx);
            let cells = match &**unsorted_cells {
                Expression::Array { values, .. } => {
                    values.iter().map(|v| compile_expression(v, ctx))
                }
                _ => panic!("dialog layout unsorted cells not an array"),
            };
            quote! {
                let mut #cells_variable = [#(#cells),*];
                slint::private_unstable_api::re_exports::reorder_dialog_button_layout(&mut #cells_variable, &#roles);
                let #cells_variable = slint::private_unstable_api::re_exports::Slice::from_slice(&#cells_variable);
            }
        }
    }
}

fn compile_builtin_function_call(
    function: BuiltinFunction,
    arguments: &[Expression],
    ctx: &EvaluationContext,
) -> TokenStream {
    let mut a = arguments.iter().map(|a| compile_expression(a, ctx));
    match function {
        BuiltinFunction::SetFocusItem => {
            if let [Expression::PropertyReference(pr)] = arguments {
                let window_tokens = access_window_adapter_field(ctx);
                let focus_item = access_item_rc(pr, ctx);
                quote!(
                    slint::private_unstable_api::re_exports::WindowInner::from_pub(#window_tokens.window()).set_focus_item(#focus_item)
                )
            } else {
                panic!("internal error: invalid args to SetFocusItem {:?}", arguments)
            }
        }
        BuiltinFunction::ShowPopupWindow => {
            if let [Expression::NumberLiteral(popup_index), x, y, Expression::PropertyReference(parent_ref)] =
                arguments
            {
                let mut parent_ctx = ctx;
                let mut component_access_tokens = quote!(_self);
                if let llr::PropertyReference::InParent { level, .. } = parent_ref {
                    for _ in 0..level.get() {
                        component_access_tokens =
                            quote!(#component_access_tokens.parent.upgrade().unwrap().as_pin_ref());
                        parent_ctx = parent_ctx.parent.as_ref().unwrap().ctx;
                    }
                }
                let current_sub_component = parent_ctx.current_sub_component.unwrap();
                let popup_window_id = inner_component_id(
                    &current_sub_component.popup_windows[*popup_index as usize].root,
                );
                let parent_component = access_item_rc(parent_ref, ctx);
                let x = compile_expression(x, ctx);
                let y = compile_expression(y, ctx);
                let window_adapter_tokens = access_window_adapter_field(ctx);
                quote!(
                    slint::private_unstable_api::re_exports::WindowInner::from_pub(#window_adapter_tokens.window()).show_popup(
                        &VRc::into_dyn(#popup_window_id::new(#component_access_tokens.self_weak.get().unwrap().clone()).into()),
                        Point::new(#x as slint::private_unstable_api::re_exports::Coord, #y as slint::private_unstable_api::re_exports::Coord),
                        #parent_component
                    )
                )
            } else {
                panic!("internal error: invalid args to ShowPopupWindow {:?}", arguments)
            }
        }
        BuiltinFunction::ItemMemberFunction(name) => {
            if let [Expression::PropertyReference(pr)] = arguments {
                let item = access_member(pr, ctx);
                let item_rc = access_item_rc(pr, ctx);
                let window_adapter_tokens = access_window_adapter_field(ctx);
                let name = ident(&name);
                quote!(
                    #item.#name(#window_adapter_tokens, #item_rc)
                )
            } else {
                panic!("internal error: invalid args to ItemMemberFunction {:?}", arguments)
            }
        }
        BuiltinFunction::ImplicitLayoutInfo(orient) => {
            if let [Expression::PropertyReference(pr)] = arguments {
                let item = access_member(pr, ctx);
                let window_adapter_tokens = access_window_adapter_field(ctx);
                quote!(
                    slint::private_unstable_api::re_exports::Item::layout_info(#item, #orient, #window_adapter_tokens)
                )
            } else {
                panic!("internal error: invalid args to ImplicitLayoutInfo {:?}", arguments)
            }
        }
        BuiltinFunction::RegisterCustomFontByPath => {
            if let [Expression::StringLiteral(path)] = arguments {
                let window_adapter_tokens = access_window_adapter_field(ctx);
                quote!(#window_adapter_tokens.renderer().register_font_from_path(&std::path::PathBuf::from(#path)).unwrap())
            } else {
                panic!("internal error: invalid args to RegisterCustomFontByPath {:?}", arguments)
            }
        }
        BuiltinFunction::RegisterCustomFontByMemory => {
            if let [Expression::NumberLiteral(resource_id)] = &arguments {
                let resource_id: usize = *resource_id as _;
                let symbol = format_ident!("SLINT_EMBEDDED_RESOURCE_{}", resource_id);
                let window_adapter_tokens = access_window_adapter_field(ctx);
                quote!(#window_adapter_tokens.renderer().register_font_from_memory(#symbol.into()).unwrap())
            } else {
                panic!("internal error: invalid args to RegisterCustomFontByMemory {:?}", arguments)
            }
        }
        BuiltinFunction::RegisterBitmapFont => {
            if let [Expression::NumberLiteral(resource_id)] = &arguments {
                let resource_id: usize = *resource_id as _;
                let symbol = format_ident!("SLINT_EMBEDDED_RESOURCE_{}", resource_id);
                let window_adapter_tokens = access_window_adapter_field(ctx);
                quote!(#window_adapter_tokens.renderer().register_bitmap_font(&#symbol))
            } else {
                panic!("internal error: invalid args to RegisterBitmapFont must be a number")
            }
        }
        BuiltinFunction::GetWindowScaleFactor => {
            let window_adapter_tokens = access_window_adapter_field(ctx);
            quote!(slint::private_unstable_api::re_exports::WindowInner::from_pub(#window_adapter_tokens.window()).scale_factor())
        }
        BuiltinFunction::GetWindowDefaultFontSize => {
            let window_item_name = ident(&ctx.public_component.item_tree.root.items[0].name);
            let root_access = &ctx.generator_state;
            let root_component_id = inner_component_id(&ctx.public_component.item_tree.root);
            let item_field = access_component_field_offset(&root_component_id, &window_item_name);
            quote!((#item_field + slint::private_unstable_api::re_exports::WindowItem::FIELD_OFFSETS.default_font_size).apply_pin(#root_access.as_pin_ref()).get().get())
        }
        BuiltinFunction::AnimationTick => {
            quote!(slint::private_unstable_api::re_exports::animation_tick())
        }
        BuiltinFunction::Debug => quote!(slint::private_unstable_api::debug(#(#a)*)),
        BuiltinFunction::Mod => quote!((#(#a as f64)%*)),
        BuiltinFunction::Round => quote!((#(#a)* as f64).round()),
        BuiltinFunction::Ceil => quote!((#(#a)* as f64).ceil()),
        BuiltinFunction::Floor => quote!((#(#a)* as f64).floor()),
        BuiltinFunction::Sqrt => quote!((#(#a)* as f64).sqrt()),
        BuiltinFunction::Abs => quote!((#(#a)* as f64).abs()),
        BuiltinFunction::Sin => quote!((#(#a)* as f64).to_radians().sin()),
        BuiltinFunction::Cos => quote!((#(#a)* as f64).to_radians().cos()),
        BuiltinFunction::Tan => quote!((#(#a)* as f64).to_radians().tan()),
        BuiltinFunction::ASin => quote!((#(#a)* as f64).asin().to_degrees()),
        BuiltinFunction::ACos => quote!((#(#a)* as f64).acos().to_degrees()),
        BuiltinFunction::ATan => quote!((#(#a)* as f64).atan().to_degrees()),
        BuiltinFunction::Log => {
            let (a1, a2) = (a.next().unwrap(), a.next().unwrap());
            quote!((#a1 as f64).log(#a2 as f64))
        }
        BuiltinFunction::Pow => {
            let (a1, a2) = (a.next().unwrap(), a.next().unwrap());
            quote!((#a1 as f64).powf(#a2 as f64))
        }
        BuiltinFunction::StringToFloat => {
            quote!(#(#a)*.as_str().parse::<f64>().unwrap_or_default())
        }
        BuiltinFunction::StringIsFloat => quote!(#(#a)*.as_str().parse::<f64>().is_ok()),
        BuiltinFunction::ColorBrighter => {
            let x = a.next().unwrap();
            let factor = a.next().unwrap();
            quote!(#x.brighter(#factor as f32))
        }
        BuiltinFunction::ColorDarker => {
            let x = a.next().unwrap();
            let factor = a.next().unwrap();
            quote!(#x.darker(#factor as f32))
        }
        BuiltinFunction::ImageSize => quote!( #(#a)*.size()),
        BuiltinFunction::ArrayLength => {
            quote!(match &#(#a)* { x => {
                x.model_tracker().track_row_count_changes();
                x.row_count() as i32
            }})
        }

        BuiltinFunction::Rgb => {
            let (r, g, b, a) =
                (a.next().unwrap(), a.next().unwrap(), a.next().unwrap(), a.next().unwrap());
            quote!({
                let r: u8 = (#r as u32).max(0).min(255) as u8;
                let g: u8 = (#g as u32).max(0).min(255) as u8;
                let b: u8 = (#b as u32).max(0).min(255) as u8;
                let a: u8 = (255. * (#a as f32)).max(0.).min(255.) as u8;
                slint::private_unstable_api::re_exports::Color::from_argb_u8(a, r, g, b)
            })
        }
        BuiltinFunction::DarkColorScheme => {
            let window_adapter_tokens = access_window_adapter_field(ctx);
            quote!(#window_adapter_tokens.dark_color_scheme())
        }
        BuiltinFunction::TextInputFocused => {
            let window_adapter_tokens = access_window_adapter_field(ctx);
            quote!(slint::private_unstable_api::re_exports::WindowInner::from_pub(#window_adapter_tokens.window()).text_input_focused())
        }
        BuiltinFunction::SetTextInputFocused => {
            let window_adapter_tokens = access_window_adapter_field(ctx);
            quote!(slint::private_unstable_api::re_exports::WindowInner::from_pub(#window_adapter_tokens.window()).set_text_input_focused(#(#a)*))
        }
        BuiltinFunction::Translate => {
            todo!("BuiltinFunction::Translate in rust")
        }
    }
}

/// Return a TokenStream for a name (as in [`Type::Struct::name`])
fn struct_name_to_tokens(name: &str) -> TokenStream {
    // the name match the C++ signature so we need to change that to the rust namespace
    let mut name = name
        .replace("slint::private_api::", "slint::private_unstable_api::re_exports::")
        .replace('-', "_");
    if !name.contains("::") {
        name.insert_str(0, "r#")
    }
    name.parse().unwrap()
}

fn box_layout_function(
    cells_variable: &str,
    repeated_indices: Option<&str>,
    elements: &[Either<Expression, usize>],
    orientation: Orientation,
    sub_expression: &Expression,
    ctx: &EvaluationContext,
) -> TokenStream {
    let repeated_indices = repeated_indices.map(ident);
    let inner_component_id = self::inner_component_id(ctx.current_sub_component.unwrap());
    let mut fixed_count = 0usize;
    let mut repeated_count = quote!();
    let mut push_code = vec![];
    let mut repeater_idx = 0usize;
    for item in elements {
        match item {
            Either::Left(value) => {
                let value = compile_expression(value, ctx);
                fixed_count += 1;
                push_code.push(quote!(items_vec.push(#value);))
            }
            Either::Right(repeater) => {
                let repeater_id = format_ident!("repeater{}", repeater);
                let rep_inner_component_id = self::inner_component_id(
                    &ctx.current_sub_component.unwrap().repeated[*repeater].sub_tree.root,
                );
                repeated_count = quote!(#repeated_count + _self.#repeater_id.len());
                let ri = repeated_indices.as_ref().map(|ri| {
                    quote!(
                        #ri[#repeater_idx * 2] = items_vec.len() as u32;
                        #ri[#repeater_idx * 2 + 1] = internal_vec.len() as u32;
                    )
                });
                repeater_idx += 1;
                push_code.push(quote!(
                        #inner_component_id::FIELD_OFFSETS.#repeater_id.apply_pin(_self).ensure_updated(
                            || { #rep_inner_component_id::new(_self.self_weak.get().unwrap().clone()).into() }
                        );
                        let internal_vec = _self.#repeater_id.components_vec();
                        #ri
                        for sub_comp in &internal_vec {
                            items_vec.push(sub_comp.as_pin_ref().box_layout_data(#orientation))
                        }
                    ));
            }
        }
    }

    let ri = repeated_indices.as_ref().map(|ri| quote!(let mut #ri = [0u32; 2 * #repeater_idx];));
    let ri2 = repeated_indices.map(
        |ri| quote!(let #ri = slint::private_unstable_api::re_exports::Slice::from_slice(&#ri);),
    );
    let cells_variable = ident(cells_variable);
    let sub_expression = compile_expression(sub_expression, ctx);

    quote! { {
        #ri
        let mut items_vec = slint::private_unstable_api::re_exports::Vec::with_capacity(#fixed_count #repeated_count);
        #(#push_code)*
        let #cells_variable = slint::private_unstable_api::re_exports::Slice::from_slice(&items_vec);
        #ri2
        #sub_expression
    } }
}

// In Rust debug builds, accessing the member of the FIELD_OFFSETS ends up copying the
// entire FIELD_OFFSETS into a new stack allocation, which with large property
// binding initialization functions isn't re-used and with large generated inner
// components ends up large amounts of stack space (see issue #133)
fn access_component_field_offset(component_id: &Ident, field: &Ident) -> TokenStream {
    quote!({ *&#component_id::FIELD_OFFSETS.#field })
}

fn embedded_file_tokens(path: &str) -> TokenStream {
    let file = crate::fileaccess::load_file(std::path::Path::new(path)).unwrap(); // embedding pass ensured that the file exists
    match file.builtin_contents {
        Some(static_data) => {
            let literal = proc_macro2::Literal::byte_string(static_data);
            quote!(#literal)
        }
        None => quote!(::core::include_bytes!(#path)),
    }
}
