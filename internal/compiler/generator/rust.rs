// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore conv gdata powf punct vref

/*! module for the Rust code generator

Some convention used in the generated code:
 - `_self` is of type `Pin<&ComponentType>`  where ComponentType is the type of the generated sub component,
    this is existing for any evaluation of a binding
 - `self_rc` is of type `VRc<ItemTreeVTable, ComponentType>` or `Rc<ComponentType>` for globals
    this is usually a local variable to the init code that shouldn't rbe relied upon by the binding code.
*/

use crate::expression_tree::{BuiltinFunction, EasingCurve, MinMaxOp, OperatorClass};
use crate::langtype::{Enumeration, EnumerationValue, Struct, Type};
use crate::layout::Orientation;
use crate::llr::{
    self, EvaluationContext as llr_EvaluationContext, Expression, ParentCtx as llr_ParentCtx,
    TypeResolutionContext as _,
};
use crate::object_tree::Document;
use crate::CompilerConfiguration;
use itertools::Either;
use lyon_path::geom::euclid::approxeq::ApproxEq;
use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use smol_str::SmolStr;
use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroUsize;
use std::str::FromStr;

#[derive(Clone)]
struct RustGeneratorContext {
    /// Path to the SharedGlobals structure that contains the global and the WindowAdaptor
    global_access: TokenStream,
}

type EvaluationContext<'a> = llr_EvaluationContext<'a, RustGeneratorContext>;
type ParentCtx<'a> = llr_ParentCtx<'a, RustGeneratorContext>;

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
                quote!(sp::Orientation::Horizontal)
            }
            Orientation::Vertical => {
                quote!(sp::Orientation::Vertical)
            }
        };
        tokens.extend(tks);
    }
}

impl quote::ToTokens for crate::embedded_resources::PixelFormat {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        use crate::embedded_resources::PixelFormat::*;
        let tks = match self {
            Rgb => quote!(sp::TexturePixelFormat::Rgb),
            Rgba => quote!(sp::TexturePixelFormat::Rgba),
            RgbaPremultiplied => {
                quote!(sp::TexturePixelFormat::RgbaPremultiplied)
            }
            AlphaMap(_) => quote!(sp::TexturePixelFormat::AlphaMap),
        };
        tokens.extend(tks);
    }
}

fn rust_primitive_type(ty: &Type) -> Option<proc_macro2::TokenStream> {
    match ty {
        Type::Void => Some(quote!(())),
        Type::Int32 => Some(quote!(i32)),
        Type::Float32 => Some(quote!(f32)),
        Type::String => Some(quote!(sp::SharedString)),
        Type::Color => Some(quote!(sp::Color)),
        Type::ComponentFactory => Some(quote!(slint::ComponentFactory)),
        Type::Duration => Some(quote!(i64)),
        Type::Angle => Some(quote!(f32)),
        Type::PhysicalLength => Some(quote!(sp::Coord)),
        Type::LogicalLength => Some(quote!(sp::Coord)),
        Type::Rem => Some(quote!(f32)),
        Type::Percent => Some(quote!(f32)),
        Type::Bool => Some(quote!(bool)),
        Type::Image => Some(quote!(sp::Image)),
        Type::Struct(s) => {
            if let Some(name) = &s.name {
                Some(struct_name_to_tokens(name))
            } else {
                let elem =
                    s.fields.values().map(rust_primitive_type).collect::<Option<Vec<_>>>()?;
                // This will produce a tuple
                Some(quote!((#(#elem,)*)))
            }
        }
        Type::Array(o) => {
            let inner = rust_primitive_type(o)?;
            Some(quote!(sp::ModelRc<#inner>))
        }
        Type::Enumeration(e) => {
            let i = ident(&e.name);
            if e.node.is_some() {
                Some(quote!(#i))
            } else {
                Some(quote!(sp::#i))
            }
        }
        Type::Brush => Some(quote!(slint::Brush)),
        Type::LayoutCache => Some(quote!(
            sp::SharedVector<
                sp::Coord,
            >
        )),
        _ => None,
    }
}

fn rust_property_type(ty: &Type) -> Option<proc_macro2::TokenStream> {
    match ty {
        Type::LogicalLength => Some(quote!(sp::LogicalLength)),
        Type::Easing => Some(quote!(sp::EasingCurve)),
        _ => rust_primitive_type(ty),
    }
}

fn primitive_property_value(ty: &Type, property_accessor: MemberAccess) -> TokenStream {
    let value = property_accessor.get_property();
    match ty {
        Type::LogicalLength => quote!(#value.get()),
        _ => value,
    }
}

fn set_primitive_property_value(ty: &Type, value_expression: TokenStream) -> TokenStream {
    match ty {
        Type::LogicalLength => {
            let rust_ty = rust_primitive_type(ty).unwrap_or(quote!(_));
            quote!(sp::LogicalLength::new(#value_expression as #rust_ty))
        }
        _ => value_expression,
    }
}

/// Generate the rust code for the given component.
pub fn generate(
    doc: &Document,
    compiler_config: &CompilerConfiguration,
) -> std::io::Result<TokenStream> {
    let (structs_and_enums_ids, structs_and_enum_def): (Vec<_>, Vec<_>) = doc
        .used_types
        .borrow()
        .structs_and_enums
        .iter()
        .filter_map(|ty| match ty {
            Type::Struct(s) => match s.as_ref() {
                Struct { fields, name: Some(name), node: Some(_), rust_attributes } => {
                    Some((ident(name), generate_struct(name, fields, rust_attributes)))
                }
                _ => None,
            },
            Type::Enumeration(en) => Some((ident(&en.name), generate_enum(en))),
            _ => None,
        })
        .unzip();

    let llr = crate::llr::lower_to_item_tree::lower_to_item_tree(doc, compiler_config)?;

    if llr.public_components.is_empty() {
        return Ok(Default::default());
    }

    let sub_compos = llr
        .used_sub_components
        .iter()
        .map(|sub_compo| generate_sub_component(*sub_compo, &llr, None, None, false))
        .collect::<Vec<_>>();
    let public_components =
        llr.public_components.iter().map(|p| generate_public_component(p, &llr));

    let popup_menu =
        llr.popup_menu.as_ref().map(|p| generate_item_tree(&p.item_tree, &llr, None, None, true));

    let version_check = format_ident!(
        "VersionCheck_{}_{}_{}",
        env!("CARGO_PKG_VERSION_MAJOR"),
        env!("CARGO_PKG_VERSION_MINOR"),
        env!("CARGO_PKG_VERSION_PATCH"),
    );

    let globals = llr
        .globals
        .iter_enumerated()
        .filter(|(_, glob)| glob.must_generate())
        .map(|(idx, glob)| generate_global(idx, glob, &llr));
    let shared_globals = generate_shared_globals(&llr, compiler_config);
    let globals_ids = llr.globals.iter().filter(|glob| glob.exported).flat_map(|glob| {
        std::iter::once(ident(&glob.name)).chain(glob.aliases.iter().map(|x| ident(x)))
    });
    let compo_ids = llr.public_components.iter().map(|c| ident(&c.name));

    let resource_symbols = generate_resources(doc);
    let named_exports = generate_named_exports(doc);
    // The inner module was meant to be internal private, but projects have been reaching into it
    // so we can't change the name of this module
    let generated_mod = doc
        .last_exported_component()
        .map(|c| format_ident!("slint_generated{}", ident(&c.id)))
        .unwrap_or_else(|| format_ident!("slint_generated"));

    #[cfg(not(feature = "bundle-translations"))]
    let translations = quote!();
    #[cfg(feature = "bundle-translations")]
    let translations = llr.translations.as_ref().map(|t| (generate_translations(t, &llr)));

    Ok(quote! {
        #[allow(non_snake_case, non_camel_case_types)]
        #[allow(unused_braces, unused_parens)]
        #[allow(clippy::all, clippy::pedantic, clippy::nursery)]
        #[allow(unknown_lints, if_let_rescope, tail_expr_drop_order)] // We don't have fancy Drop
        mod #generated_mod {
            use slint::private_unstable_api::re_exports as sp;
            #[allow(unused_imports)]
            use sp::{RepeatedItemTree as _, ModelExt as _, Model as _, Float as _};
            #(#structs_and_enum_def)*
            #(#globals)*
            #(#sub_compos)*
            #popup_menu
            #(#public_components)*
            #shared_globals
            #(#resource_symbols)*
            #translations
            const _THE_SAME_VERSION_MUST_BE_USED_FOR_THE_COMPILER_AND_THE_RUNTIME : slint::#version_check = slint::#version_check;
        }
        #[allow(unused_imports)]
        pub use #generated_mod::{#(#compo_ids,)* #(#structs_and_enums_ids,)* #(#globals_ids,)* #(#named_exports,)*};
        #[allow(unused_imports)]
        pub use slint::{ComponentHandle as _, Global as _, ModelExt as _};
    })
}

fn generate_public_component(
    llr: &llr::PublicComponent,
    unit: &llr::CompilationUnit,
) -> TokenStream {
    let public_component_id = ident(&llr.name);
    let inner_component_id = inner_component_id(&unit.sub_components[llr.item_tree.root]);

    let component = generate_item_tree(&llr.item_tree, unit, None, None, false);

    let ctx = EvaluationContext {
        compilation_unit: unit,
        current_sub_component: Some(llr.item_tree.root),
        current_global: None,
        generator_state: RustGeneratorContext {
            global_access: quote!(_self.globals.get().unwrap()),
        },
        parent: None,
        argument_types: &[],
    };

    let property_and_callback_accessors = public_api(
        &llr.public_properties,
        &llr.private_properties,
        quote!(sp::VRc::as_pin_ref(&self.0)),
        &ctx,
    );

    #[cfg(feature = "bundle-translations")]
    let init_bundle_translations = unit
        .translations
        .as_ref()
        .map(|_| quote!(sp::set_bundled_languages(_SLINT_BUNDLED_LANGUAGES);));
    #[cfg(not(feature = "bundle-translations"))]
    let init_bundle_translations = quote!();

    quote!(
        #component
        pub struct #public_component_id(sp::VRc<sp::ItemTreeVTable, #inner_component_id>);

        impl #public_component_id {
            pub fn new() -> core::result::Result<Self, slint::PlatformError> {
                let inner = #inner_component_id::new()?;
                #init_bundle_translations
                inner.globals.get().unwrap().init();
                #inner_component_id::user_init(sp::VRc::map(inner.clone(), |x| x));
                // ensure that the window exist as this point so further call to window() don't panic
                inner.globals.get().unwrap().window_adapter_ref()?;
                core::result::Result::Ok(Self(inner))
            }

            #property_and_callback_accessors
        }

        impl From<#public_component_id> for sp::VRc<sp::ItemTreeVTable, #inner_component_id> {
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

            fn from_inner(inner: sp::VRc<sp::ItemTreeVTable, #inner_component_id>) -> Self {
                Self(inner)
            }

            fn run(&self) -> core::result::Result<(), slint::PlatformError> {
                self.show()?;
                slint::run_event_loop()?;
                self.hide()?;
                core::result::Result::Ok(())
            }

            fn show(&self) -> core::result::Result<(), slint::PlatformError> {
                self.0.globals.get().unwrap().window_adapter_ref()?.window().show()
            }

            fn hide(&self) -> core::result::Result<(), slint::PlatformError> {
                self.0.globals.get().unwrap().window_adapter_ref()?.window().hide()
            }

            fn window(&self) -> &slint::Window {
                self.0.globals.get().unwrap().window_adapter_ref().unwrap().window()
            }

            fn global<'a, T: slint::Global<'a, Self>>(&'a self) -> T {
                T::get(&self)
            }
        }
    )
}

fn generate_shared_globals(
    llr: &llr::CompilationUnit,
    compiler_config: &CompilerConfiguration,
) -> TokenStream {
    let global_names = llr
        .globals
        .iter()
        .filter(|g| g.is_builtin || g.must_generate())
        .map(|g| format_ident!("global_{}", ident(&g.name)))
        .collect::<Vec<_>>();
    let global_types = llr
        .globals
        .iter()
        .filter(|g| g.is_builtin || g.must_generate())
        .map(global_inner_name)
        .collect::<Vec<_>>();

    let apply_constant_scale_factor = if !compiler_config.const_scale_factor.approx_eq(&1.0) {
        let factor = compiler_config.const_scale_factor as f32;
        Some(
            quote!(adapter.window().try_dispatch_event(slint::platform::WindowEvent::ScaleFactorChanged{ scale_factor: #factor })?;),
        )
    } else {
        None
    };

    quote! {
        struct SharedGlobals {
            #(#global_names : ::core::pin::Pin<sp::Rc<#global_types>>,)*
            window_adapter : sp::OnceCell<sp::WindowAdapterRc>,
            root_item_tree_weak : sp::VWeak<sp::ItemTreeVTable>,
        }
        impl SharedGlobals {
            fn new(root_item_tree_weak : sp::VWeak<sp::ItemTreeVTable>) -> Self {
                Self {
                    #(#global_names : #global_types::new(),)*
                    window_adapter : ::core::default::Default::default(),
                    root_item_tree_weak,
                }
            }

            fn init(self: &sp::Rc<Self>) {
                #(self.#global_names.clone().init(self);)*
            }

            fn window_adapter_impl(&self) -> sp::Rc<dyn sp::WindowAdapter> {
                sp::Rc::clone(self.window_adapter_ref().unwrap())
            }

            fn window_adapter_ref(&self) -> sp::Result<&sp::Rc<dyn sp::WindowAdapter>, slint::PlatformError>
            {
                self.window_adapter.get_or_try_init(|| {
                    let adapter = slint::private_unstable_api::create_window_adapter()?;
                    let root_rc = self.root_item_tree_weak.upgrade().unwrap();
                    sp::WindowInner::from_pub(adapter.window()).set_component(&root_rc);
                    #apply_constant_scale_factor
                    core::result::Result::Ok(adapter)
                })
            }

            fn maybe_window_adapter_impl(&self) -> sp::Option<sp::Rc<dyn sp::WindowAdapter>> {
                self.window_adapter.get().cloned()
            }
        }
    }
}

fn generate_struct(
    name: &str,
    fields: &BTreeMap<SmolStr, Type>,
    rust_attributes: &Option<Vec<SmolStr>>,
) -> TokenStream {
    let component_id = struct_name_to_tokens(name);
    let (declared_property_vars, declared_property_types): (Vec<_>, Vec<_>) =
        fields.iter().map(|(name, ty)| (ident(name), rust_primitive_type(ty).unwrap())).unzip();

    let attributes = if let Some(feature) = rust_attributes {
        let attr =
            feature.iter().map(|f| match TokenStream::from_str(format!(r#"#[{f}]"#).as_str()) {
                Ok(eval) => eval,
                Err(_) => quote! {},
            });
        quote! { #(#attr)* }
    } else {
        quote! {}
    };

    quote! {
        #attributes
        #[derive(Default, PartialEq, Debug, Clone)]
        pub struct #component_id {
            #(pub #declared_property_vars : #declared_property_types),*
        }
    }
}

fn generate_enum(en: &std::rc::Rc<Enumeration>) -> TokenStream {
    let enum_name = ident(&en.name);

    let enum_values = (0..en.values.len()).map(|value| {
        let i = ident(&EnumerationValue { value, enumeration: en.clone() }.to_pascal_case());
        if value == en.default_value {
            quote!(#[default] #i)
        } else {
            quote!(#i)
        }
    });
    let rust_attr = en.node.as_ref().and_then(|node| {
        node.AtRustAttr().map(|attr| {
            match TokenStream::from_str(format!(r#"#[{}]"#, attr.text()).as_str()) {
                Ok(eval) => eval,
                Err(_) => quote! {},
            }
        })
    });
    quote! {
        #[allow(dead_code)]
        #[derive(Default, Copy, Clone, PartialEq, Debug)]
        #rust_attr
        pub enum #enum_name {
            #(#enum_values,)*
        }
    }
}

fn handle_property_init(
    prop: &llr::PropertyReference,
    binding_expression: &llr::BindingExpression,
    init: &mut Vec<TokenStream>,
    ctx: &EvaluationContext,
) {
    let rust_property = access_member(prop, ctx).unwrap();
    let prop_type = ctx.property_ty(prop);

    let init_self_pin_ref = if ctx.current_global.is_some() {
        quote!(let _self = self_rc.as_ref();)
    } else {
        quote!(let _self = self_rc.as_pin_ref();)
    };

    if let Type::Callback(callback) = &prop_type {
        let mut ctx2 = ctx.clone();
        ctx2.argument_types = &callback.args;
        let tokens_for_expression =
            compile_expression(&binding_expression.expression.borrow(), &ctx2);
        let as_ = if matches!(callback.return_type, Type::Void) { quote!(;) } else { quote!(as _) };
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
            quote! { #rust_property.set({ (#tokens_for_expression) as #t }); }
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
        let prop = access_member(&p.prop, ctx).unwrap();

        if let Type::Callback(callback) = &p.ty {
            let callback_args =
                callback.args.iter().map(|a| rust_primitive_type(a).unwrap()).collect::<Vec<_>>();
            let return_type = rust_primitive_type(&callback.return_type).unwrap();
            let args_name =
                (0..callback.args.len()).map(|i| format_ident!("arg_{}", i)).collect::<Vec<_>>();
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
        } else if let Type::Function(function) = &p.ty {
            let callback_args =
                function.args.iter().map(|a| rust_primitive_type(a).unwrap()).collect::<Vec<_>>();
            let return_type = rust_primitive_type(&function.return_type).unwrap();
            let args_name =
                (0..function.args.len()).map(|i| format_ident!("arg_{}", i)).collect::<Vec<_>>();
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

            let prop_expression = primitive_property_value(&p.ty, MemberAccess::Direct(prop));

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
    component_idx: llr::SubComponentIdx,
    root: &llr::CompilationUnit,
    parent_ctx: Option<ParentCtx>,
    index_property: Option<llr::PropertyIdx>,
    pinned_drop: bool,
) -> TokenStream {
    let component = &root.sub_components[component_idx];
    let inner_component_id = inner_component_id(component);

    let ctx = EvaluationContext::new_sub_component(
        root,
        component_idx,
        RustGeneratorContext { global_access: quote!(_self.globals.get().unwrap()) },
        parent_ctx,
    );
    let mut extra_components = component
        .popup_windows
        .iter()
        .map(|popup| {
            generate_item_tree(
                &popup.item_tree,
                root,
                Some(ParentCtx::new(&ctx, None)),
                None,
                false,
            )
        })
        .chain(component.menu_item_trees.iter().map(|tree| {
            generate_item_tree(tree, root, Some(ParentCtx::new(&ctx, None)), None, false)
        }))
        .collect::<Vec<_>>();

    let mut declared_property_vars = vec![];
    let mut declared_property_types = vec![];
    let mut declared_callbacks = vec![];
    let mut declared_callbacks_types = vec![];
    let mut declared_callbacks_ret = vec![];

    for property in component.properties.iter().filter(|p| p.use_count.get() > 0) {
        let prop_ident = ident(&property.name);
        if let Type::Callback(callback) = &property.ty {
            let callback_args =
                callback.args.iter().map(|a| rust_primitive_type(a).unwrap()).collect::<Vec<_>>();
            let return_type = rust_primitive_type(&callback.return_type).unwrap();
            declared_callbacks.push(prop_ident.clone());
            declared_callbacks_types.push(callback_args);
            declared_callbacks_ret.push(return_type);
        } else {
            let rust_property_type = rust_property_type(&property.ty).unwrap();
            declared_property_vars.push(prop_ident.clone());
            declared_property_types.push(rust_property_type.clone());
        }
    }

    let change_tracker_names = component
        .change_callbacks
        .iter()
        .enumerate()
        .map(|(idx, _)| format_ident!("change_tracker{idx}"));

    let declared_functions = generate_functions(component.functions.as_ref(), &ctx);

    let mut init = vec![];
    let mut item_names = vec![];
    let mut item_types = vec![];

    #[cfg(slint_debug_property)]
    init.push(quote!(
        #(self_rc.#declared_property_vars.debug_name.replace(
            concat!(stringify!(#inner_component_id), ".", stringify!(#declared_property_vars)).into());)*
    ));

    for item in &component.items {
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

    let mut repeated_visit_branch: Vec<TokenStream> = vec![];
    let mut repeated_element_components: Vec<TokenStream> = vec![];
    let mut repeated_subtree_ranges: Vec<TokenStream> = vec![];
    let mut repeated_subtree_components: Vec<TokenStream> = vec![];

    for (idx, repeated) in component.repeated.iter_enumerated() {
        extra_components.push(generate_repeated_component(
            repeated,
            root,
            ParentCtx::new(&ctx, Some(idx)),
        ));
        let idx = usize::from(idx) as u32;
        let repeater_id = format_ident!("repeater{}", idx);
        let rep_inner_component_id =
            self::inner_component_id(&root.sub_components[repeated.sub_tree.root]);

        let model = compile_expression(&repeated.model.borrow(), &ctx);
        init.push(quote! {
            _self.#repeater_id.set_model_binding({
                let self_weak = sp::VRcMapped::downgrade(&self_rc);
                move || {
                    let self_rc = self_weak.upgrade().unwrap();
                    let _self = self_rc.as_pin_ref();
                    (#model) as _
                }
            });
        });
        let ensure_updated = if let Some(listview) = &repeated.listview {
            let vp_y = access_member(&listview.viewport_y, &ctx).unwrap();
            let vp_h = access_member(&listview.viewport_height, &ctx).unwrap();
            let lv_h = access_member(&listview.listview_height, &ctx).unwrap();
            let vp_w = access_member(&listview.viewport_width, &ctx).unwrap();
            let lv_w = access_member(&listview.listview_width, &ctx).unwrap();

            quote! {
                #inner_component_id::FIELD_OFFSETS.#repeater_id.apply_pin(_self).ensure_updated_listview(
                    || { #rep_inner_component_id::new(_self.self_weak.get().unwrap().clone()).unwrap().into() },
                    #vp_w, #vp_h, #vp_y, #lv_w.get(), #lv_h
                );
            }
        } else {
            quote! {
                #inner_component_id::FIELD_OFFSETS.#repeater_id.apply_pin(_self).ensure_updated(
                    || #rep_inner_component_id::new(_self.self_weak.get().unwrap().clone()).unwrap().into()
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
                sp::IndexRange::from(_self.#repeater_id.range())
            }
        ));
        repeated_subtree_components.push(quote!(
            #idx => {
                #ensure_updated
                if let Some(instance) = _self.#repeater_id.instance_at(subtree_index) {
                    *result = sp::VRc::downgrade(&sp::VRc::into_dyn(instance));
                }
            }
        ));
        repeated_element_components.push(if repeated.index_prop.is_some() {
            quote!(#repeater_id: sp::Repeater<#rep_inner_component_id>)
        } else {
            quote!(#repeater_id: sp::Conditional<#rep_inner_component_id>)
        });
    }

    // Use ids following the real repeaters to piggyback on their forwarding through sub-components!
    for (idx, container) in component.component_containers.iter().enumerate() {
        let idx = (component.repeated.len() + idx) as u32;
        let items_index = container.component_container_items_index;

        let embed_item = access_member(
            &llr::PropertyReference::InNativeItem {
                sub_component_path: vec![],
                item_index: items_index,
                prop_name: String::new(),
            },
            &ctx,
        )
        .unwrap();

        let ensure_updated = {
            quote! {
                #embed_item.ensure_updated();
            }
        };

        repeated_visit_branch.push(quote!(
            #idx => {
                #ensure_updated
                #embed_item.visit_children_item(-1, order, visitor)
            }
        ));
        repeated_subtree_ranges.push(quote!(
            #idx => {
                #ensure_updated
                #embed_item.subtree_range()
            }
        ));
        repeated_subtree_components.push(quote!(
            #idx => {
                #ensure_updated
                if subtree_index == 0 {
                    *result = #embed_item.subtree_component()
                }
            }
        ));
    }

    let mut accessible_role_branch = vec![];
    let mut accessible_string_property_branch = vec![];
    let mut accessibility_action_branch = vec![];
    let mut supported_accessibility_actions = BTreeMap::<u32, BTreeSet<_>>::new();
    for ((index, what), expr) in &component.accessible_prop {
        let e = compile_expression(&expr.borrow(), &ctx);
        if what == "Role" {
            accessible_role_branch.push(quote!(#index => #e,));
        } else if let Some(what) = what.strip_prefix("Action") {
            let what = ident(what);
            let has_args = matches!(&*expr.borrow(), Expression::CallBackCall { arguments, .. } if !arguments.is_empty());
            accessibility_action_branch.push(if has_args {
                quote!((#index, sp::AccessibilityAction::#what(args)) => { let args = (args,); #e })
            } else {
                quote!((#index, sp::AccessibilityAction::#what) => { #e })
            });
            supported_accessibility_actions.entry(*index).or_default().insert(what);
        } else {
            let what = ident(what);
            accessible_string_property_branch
                .push(quote!((#index, sp::AccessibleStringProperty::#what) => sp::Some(#e),));
        }
    }
    let mut supported_accessibility_actions_branch = supported_accessibility_actions
        .into_iter()
        .map(|(index, values)| quote!(#index => #(sp::SupportedAccessibilityAction::#values)|*,))
        .collect::<Vec<_>>();

    let mut item_geometry_branch = component
        .geometries
        .iter()
        .enumerate()
        .filter_map(|(i, x)| x.as_ref().map(|x| (i, x)))
        .map(|(index, expr)| {
            let expr = compile_expression(&expr.borrow(), &ctx);
            let index = index as u32;
            quote!(#index => #expr,)
        })
        .collect::<Vec<_>>();

    let mut item_element_infos_branch = component
        .element_infos
        .iter()
        .map(|(item_index, ids)| quote!(#item_index => { return sp::Some(#ids.into()); }))
        .collect::<Vec<_>>();

    let mut user_init_code: Vec<TokenStream> = Vec::new();

    let mut sub_component_names: Vec<Ident> = vec![];
    let mut sub_component_types: Vec<Ident> = vec![];

    for sub in &component.sub_components {
        let field_name = ident(&sub.name);
        let sc = &root.sub_components[sub.ty];
        let sub_component_id = self::inner_component_id(sc);
        let local_tree_index: u32 = sub.index_in_tree as _;
        let local_index_of_first_child: u32 = sub.index_of_first_child_in_tree as _;
        let global_access = &ctx.generator_state.global_access;

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
            sp::VRcMapped::map(self_rc.clone(), |x| #sub_compo_field.apply_pin(x)),
            #global_access.clone(), #global_index, #global_children
        );));
        user_init_code.push(quote!(#sub_component_id::user_init(
            sp::VRcMapped::map(self_rc.clone(), |x| #sub_compo_field.apply_pin(x)),
        );));

        let sub_component_repeater_count = sc.repeater_count(root);
        if sub_component_repeater_count > 0 {
            let repeater_offset = sub.repeater_offset;
            let last_repeater = repeater_offset + sub_component_repeater_count - 1;
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

        let sub_items_count = sc.child_item_count(root);
        accessible_role_branch.push(quote!(
            #local_tree_index => #sub_compo_field.apply_pin(_self).accessible_role(0),
        ));
        accessible_string_property_branch.push(quote!(
            (#local_tree_index, _) => #sub_compo_field.apply_pin(_self).accessible_string_property(0, what),
        ));
        accessibility_action_branch.push(quote!(
            (#local_tree_index, _) => #sub_compo_field.apply_pin(_self).accessibility_action(0, action),
        ));
        supported_accessibility_actions_branch.push(quote!(
            #local_tree_index => #sub_compo_field.apply_pin(_self).supported_accessibility_actions(0),
        ));
        if sub_items_count > 1 {
            let range_begin = local_index_of_first_child;
            let range_end = range_begin + sub_items_count - 2 + sc.repeater_count(root);
            accessible_role_branch.push(quote!(
                #range_begin..=#range_end => #sub_compo_field.apply_pin(_self).accessible_role(index - #range_begin + 1),
            ));
            accessible_string_property_branch.push(quote!(
                (#range_begin..=#range_end, _) => #sub_compo_field.apply_pin(_self).accessible_string_property(index - #range_begin + 1, what),
            ));
            item_geometry_branch.push(quote!(
                #range_begin..=#range_end => return #sub_compo_field.apply_pin(_self).item_geometry(index - #range_begin + 1),
            ));
            accessibility_action_branch.push(quote!(
                (#range_begin..=#range_end, _) => #sub_compo_field.apply_pin(_self).accessibility_action(index - #range_begin + 1, action),
            ));
            supported_accessibility_actions_branch.push(quote!(
                #range_begin..=#range_end => #sub_compo_field.apply_pin(_self).supported_accessibility_actions(index - #range_begin + 1),
            ));
            item_element_infos_branch.push(quote!(
                #range_begin..=#range_end => #sub_compo_field.apply_pin(_self).item_element_infos(index - #range_begin + 1),
            ));
        }

        sub_component_names.push(field_name);
        sub_component_types.push(sub_component_id);
    }

    let popup_id_names =
        component.popup_windows.iter().enumerate().map(|(i, _)| internal_popup_id(i));

    for (prop1, prop2) in &component.two_way_bindings {
        let p1 = access_member(prop1, &ctx);
        let p2 = access_member(prop2, &ctx);
        let r = p1.then(|p1| p2.then(|p2| quote!(sp::Property::link_two_way(#p1, #p2))));
        init.push(quote!(#r;))
    }

    for (prop, expression) in &component.property_init {
        if expression.use_count.get() > 0 && component.prop_used(prop, root) {
            handle_property_init(prop, expression, &mut init, &ctx)
        }
    }
    for prop in &component.const_properties {
        if component.prop_used(prop, root) {
            let rust_property = access_member(prop, &ctx).unwrap();
            init.push(quote!(#rust_property.set_constant();))
        }
    }

    let parent_component_type = parent_ctx.iter().map(|parent| {
        let parent_component_id =
            self::inner_component_id(parent.ctx.current_sub_component().unwrap());
        quote!(sp::VWeakMapped::<sp::ItemTreeVTable, #parent_component_id>)
    });

    user_init_code.extend(component.init_code.iter().map(|e| {
        let code = compile_expression(&e.borrow(), &ctx);
        quote!(#code;)
    }));

    user_init_code.extend(component.change_callbacks.iter().enumerate().map(|(idx, (p, e))| {
        let code = compile_expression(&e.borrow(), &ctx);
        let prop = compile_expression(&Expression::PropertyReference(p.clone()), &ctx);
        let change_tracker = format_ident!("change_tracker{idx}");
        quote! {
            let self_weak = sp::VRcMapped::downgrade(&self_rc);
            #[allow(dead_code, unused)]
            _self.#change_tracker.init(
                self_weak,
                move |self_weak| {
                    let self_rc = self_weak.upgrade().unwrap();
                    let _self = self_rc.as_pin_ref();
                    #prop
                },
                move |self_weak, _| {
                    let self_rc = self_weak.upgrade().unwrap();
                    let _self = self_rc.as_pin_ref();
                    #code;
                }
            );
        }
    }));

    let layout_info_h = compile_expression(&component.layout_info_h.borrow(), &ctx);
    let layout_info_v = compile_expression(&component.layout_info_v.borrow(), &ctx);

    // FIXME! this is only public because of the ComponentHandle::Inner. we should find another way
    let visibility = parent_ctx.is_none().then(|| quote!(pub));

    let subtree_index_function = if let Some(property_index) = index_property {
        let prop = access_member(
            &llr::PropertyReference::Local { sub_component_path: vec![], property_index },
            &ctx,
        )
        .unwrap();
        quote!(#prop.get() as usize)
    } else {
        quote!(usize::MAX)
    };

    let timer_names =
        component.timers.iter().enumerate().map(|(idx, _)| format_ident!("timer{idx}"));
    let update_timers = (!component.timers.is_empty()).then(|| {
        let updt = component.timers.iter().enumerate().map(|(idx, tmr)| {
            let ident = format_ident!("timer{idx}");
            let interval = compile_expression(&tmr.interval.borrow(), &ctx);
            let running = compile_expression(&tmr.running.borrow(), &ctx);
            let callback = compile_expression(&tmr.triggered.borrow(), &ctx);
            quote!(
                if #running {
                    let interval = core::time::Duration::from_millis(#interval as u64);
                    if !self.#ident.running() || interval != self.#ident.interval() {
                        let self_weak = self.self_weak.get().unwrap().clone();
                        self.#ident.start(sp::TimerMode::Repeated, interval, move || {
                            if let Some(self_rc) = self_weak.upgrade() {
                                let _self = self_rc.as_pin_ref();
                                #callback
                            }
                        });
                    }
                } else {
                    self.#ident.stop();
                }
            )
        });
        user_init_code.push(quote!(_self.update_timers();));
        quote!(
            fn update_timers(self: ::core::pin::Pin<&Self>) {
                let _self = self;
                #(#updt)*
            }
        )
    });

    let pin_macro = if pinned_drop { quote!(#[pin_drop]) } else { quote!(#[pin]) };

    quote!(
        #[derive(sp::FieldOffsets, Default)]
        #[const_field_offset(sp::const_field_offset)]
        #[repr(C)]
        #pin_macro
        #visibility
        struct #inner_component_id {
            #(#item_names : sp::#item_types,)*
            #(#sub_component_names : #sub_component_types,)*
            #(#popup_id_names : ::core::cell::Cell<sp::Option<::core::num::NonZeroU32>>,)*
            #(#declared_property_vars : sp::Property<#declared_property_types>,)*
            #(#declared_callbacks : sp::Callback<(#(#declared_callbacks_types,)*), #declared_callbacks_ret>,)*
            #(#repeated_element_components,)*
            #(#change_tracker_names : sp::ChangeTracker,)*
            #(#timer_names : sp::Timer,)*
            self_weak : sp::OnceCell<sp::VWeakMapped<sp::ItemTreeVTable, #inner_component_id>>,
            #(parent : #parent_component_type,)*
            globals: sp::OnceCell<sp::Rc<SharedGlobals>>,
            tree_index: ::core::cell::Cell<u32>,
            tree_index_of_first_child: ::core::cell::Cell<u32>,
        }

        impl #inner_component_id {
            fn init(self_rc: sp::VRcMapped<sp::ItemTreeVTable, Self>,
                    globals : sp::Rc<SharedGlobals>,
                    tree_index: u32, tree_index_of_first_child: u32) {
                #![allow(unused)]
                let _self = self_rc.as_pin_ref();
                _self.self_weak.set(sp::VRcMapped::downgrade(&self_rc));
                _self.globals.set(globals);
                _self.tree_index.set(tree_index);
                _self.tree_index_of_first_child.set(tree_index_of_first_child);
                #(#init)*
            }

            fn user_init(self_rc: sp::VRcMapped<sp::ItemTreeVTable, Self>) {
                #![allow(unused)]
                let _self = self_rc.as_pin_ref();
                #(#user_init_code)*
            }

            fn visit_dynamic_children(
                self: ::core::pin::Pin<&Self>,
                dyn_index: u32,
                order: sp::TraversalOrder,
                visitor: sp::ItemVisitorRefMut<'_>
            ) -> sp::VisitChildrenResult {
                #![allow(unused)]
                let _self = self;
                match dyn_index {
                    #(#repeated_visit_branch)*
                    _ => panic!("invalid dyn_index {}", dyn_index),
                }
            }

            fn layout_info(self: ::core::pin::Pin<&Self>, orientation: sp::Orientation) -> sp::LayoutInfo {
                #![allow(unused)]
                let _self = self;
                match orientation {
                    sp::Orientation::Horizontal => #layout_info_h,
                    sp::Orientation::Vertical => #layout_info_v,
                }
            }

            fn subtree_range(self: ::core::pin::Pin<&Self>, dyn_index: u32) -> sp::IndexRange {
                #![allow(unused)]
                let _self = self;
                match dyn_index {
                    #(#repeated_subtree_ranges)*
                    _ => panic!("invalid dyn_index {}", dyn_index),
                }
            }

            fn subtree_component(self: ::core::pin::Pin<&Self>, dyn_index: u32, subtree_index: usize, result: &mut sp::ItemTreeWeak) {
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

            fn item_geometry(self: ::core::pin::Pin<&Self>, index: u32) -> sp::LogicalRect {
                #![allow(unused)]
                let _self = self;
                // The result of the expression is an anonymous struct, `{height: length, width: length, x: length, y: length}`
                // fields are in alphabetical order
                let (h, w, x, y) = match index {
                    #(#item_geometry_branch)*
                    _ => return ::core::default::Default::default()
                };
                sp::euclid::rect(x, y, w, h)
            }

            fn accessible_role(self: ::core::pin::Pin<&Self>, index: u32) -> sp::AccessibleRole {
                #![allow(unused)]
                let _self = self;
                match index {
                    #(#accessible_role_branch)*
                    //#(#forward_sub_ranges => #forward_sub_field.apply_pin(_self).accessible_role())*
                    _ => sp::AccessibleRole::default(),
                }
            }

            fn accessible_string_property(
                self: ::core::pin::Pin<&Self>,
                index: u32,
                what: sp::AccessibleStringProperty,
            ) -> sp::Option<sp::SharedString> {
                #![allow(unused)]
                let _self = self;
                match (index, what) {
                    #(#accessible_string_property_branch)*
                    _ => sp::None,
                }
            }

            fn accessibility_action(self: ::core::pin::Pin<&Self>, index: u32, action: &sp::AccessibilityAction) {
                #![allow(unused)]
                let _self = self;
                match (index, action) {
                    #(#accessibility_action_branch)*
                    _ => (),
                }
            }

            fn supported_accessibility_actions(self: ::core::pin::Pin<&Self>, index: u32) -> sp::SupportedAccessibilityAction {
                #![allow(unused)]
                let _self = self;
                match index {
                    #(#supported_accessibility_actions_branch)*
                    _ => ::core::default::Default::default(),
                }
            }

            fn item_element_infos(self: ::core::pin::Pin<&Self>, index: u32) -> sp::Option<sp::SharedString> {
                #![allow(unused)]
                let _self = self;
                match index {
                    #(#item_element_infos_branch)*
                    _ => { ::core::default::Default::default() }
                }
            }

            #update_timers

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

fn generate_global(
    global_idx: llr::GlobalIdx,
    global: &llr::GlobalComponent,
    root: &llr::CompilationUnit,
) -> TokenStream {
    let mut declared_property_vars = vec![];
    let mut declared_property_types = vec![];
    let mut declared_callbacks = vec![];
    let mut declared_callbacks_types = vec![];
    let mut declared_callbacks_ret = vec![];

    for property in global.properties.iter().filter(|p| p.use_count.get() > 0) {
        let prop_ident = ident(&property.name);
        if let Type::Callback(callback) = &property.ty {
            let callback_args =
                callback.args.iter().map(|a| rust_primitive_type(a).unwrap()).collect::<Vec<_>>();
            let return_type = rust_primitive_type(&callback.return_type);
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
        global_idx,
        RustGeneratorContext {
            global_access: quote!(_self.globals.get().unwrap().upgrade().unwrap()),
        },
    );

    let declared_functions = generate_functions(global.functions.as_ref(), &ctx);

    for (property_index, expression) in global.init_values.iter_enumerated() {
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
    for (property_index, cst) in global.const_properties.iter_enumerated() {
        if global.properties[property_index].use_count.get() == 0 {
            continue;
        }
        if *cst {
            let rust_property = access_member(
                &llr::PropertyReference::Local { sub_component_path: vec![], property_index },
                &ctx,
            )
            .unwrap();
            init.push(quote!(#rust_property.set_constant();))
        }
    }

    let public_component_id = ident(&global.name);
    let global_id = format_ident!("global_{}", public_component_id);

    let change_tracker_names = global
        .change_callbacks
        .iter()
        .map(|(idx, _)| format_ident!("change_tracker{}", usize::from(*idx)));
    init.extend(global.change_callbacks.iter().map(|(p, e)| {
        let code = compile_expression(&e.borrow(), &ctx);
        let prop = access_member(
            &llr::PropertyReference::Local { sub_component_path: vec![], property_index: *p },
            &ctx,
        )
        .unwrap();
        let change_tracker = format_ident!("change_tracker{}", usize::from(*p));
        quote! {
            #[allow(dead_code, unused)]
            _self.#change_tracker.init(
                self_rc.globals.get().unwrap().clone(),
                move |global_weak| {
                    let self_rc = global_weak.upgrade().unwrap().#global_id.clone();
                    let _self = self_rc.as_ref();
                    #prop.get()
                },
                move |global_weak, _| {
                    let self_rc = global_weak.upgrade().unwrap().#global_id.clone();
                    let _self = self_rc.as_ref();
                    #code;
                }
            );
        }
    }));

    let public_interface = global.exported.then(|| {
        let property_and_callback_accessors = public_api(
            &global.public_properties,
            &global.private_properties,
            quote!(self.0.as_ref()),
            &ctx,
        );
        let aliases = global.aliases.iter().map(|name| ident(name));
        let getters = root.public_components.iter().map(|c| {
            let root_component_id = ident(&c.name);
            quote! {
                impl<'a> slint::Global<'a, #root_component_id> for #public_component_id<'a> {
                    fn get(component: &'a #root_component_id) -> Self {
                        Self(&component.0.globals.get().unwrap().#global_id)
                    }
                }
            }
        });

        quote!(
            #[allow(unused)]
            pub struct #public_component_id<'a>(&'a ::core::pin::Pin<sp::Rc<#inner_component_id>>);

            impl<'a> #public_component_id<'a> {
                #property_and_callback_accessors
            }
            #(pub type #aliases<'a> = #public_component_id<'a>;)*
            #(#getters)*
        )
    });

    quote!(
        #[derive(sp::FieldOffsets, Default)]
        #[const_field_offset(sp::const_field_offset)]
        #[repr(C)]
        #[pin]
        struct #inner_component_id {
            #(#declared_property_vars: sp::Property<#declared_property_types>,)*
            #(#declared_callbacks: sp::Callback<(#(#declared_callbacks_types,)*), #declared_callbacks_ret>,)*
            #(#change_tracker_names : sp::ChangeTracker,)*
            globals : sp::OnceCell<sp::Weak<SharedGlobals>>,
        }

        impl #inner_component_id {
            fn new() -> ::core::pin::Pin<sp::Rc<Self>> {
                sp::Rc::pin(Self::default())
            }
            fn init(self: ::core::pin::Pin<sp::Rc<Self>>, globals: &sp::Rc<SharedGlobals>) {
                #![allow(unused)]
                self.globals.set(sp::Rc::downgrade(globals));
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
    root: &llr::CompilationUnit,
    parent_ctx: Option<ParentCtx>,
    index_property: Option<llr::PropertyIdx>,
    is_popup_menu: bool,
) -> TokenStream {
    let sub_comp = generate_sub_component(sub_tree.root, root, parent_ctx, index_property, true);
    let inner_component_id = self::inner_component_id(&root.sub_components[sub_tree.root]);
    let parent_component_type = parent_ctx
        .iter()
        .map(|parent| {
            let parent_component_id =
                self::inner_component_id(parent.ctx.current_sub_component().unwrap());
            quote!(sp::VWeakMapped::<sp::ItemTreeVTable, #parent_component_id>)
        })
        .collect::<Vec<_>>();

    let globals = if is_popup_menu {
        quote!(globals)
    } else if parent_ctx.is_some() {
        quote!(parent.upgrade().unwrap().globals.get().unwrap().clone())
    } else {
        quote!(sp::Rc::new(SharedGlobals::new(sp::VRc::downgrade(&self_dyn_rc))))
    };
    let globals_arg = is_popup_menu.then(|| quote!(globals: sp::Rc<SharedGlobals>));

    let embedding_function = if parent_ctx.is_some() {
        quote!(todo!("Components written in Rust can not get embedded yet."))
    } else {
        quote!(false)
    };

    let parent_item_expression = parent_ctx.and_then(|parent| {
        parent.repeater_index.map(|idx| {
            let sub_component_offset = parent.ctx.current_sub_component().unwrap().repeated[idx].index_in_tree;

            quote!(if let Some((parent_component, parent_index)) = self
                .parent
                .clone()
                .upgrade()
                .map(|sc| (sp::VRcMapped::origin(&sc), sc.tree_index_of_first_child.get()))
            {
                *_result = sp::ItemRc::new(parent_component, parent_index + #sub_component_offset - 1)
                    .downgrade();
            })
        })
    });
    let mut item_tree_array = vec![];
    let mut item_array = vec![];
    sub_tree.tree.visit_in_array(&mut |node, children_offset, parent_index| {
        let parent_index = parent_index as u32;
        let (path, component) =
            follow_sub_component_path(root, sub_tree.root, &node.sub_component_path);
        match node.item_index {
            Either::Right(mut repeater_index) => {
                assert_eq!(node.children.len(), 0);
                let mut sub_component = &root.sub_components[sub_tree.root];
                for i in &node.sub_component_path {
                    repeater_index += sub_component.sub_components[*i].repeater_offset;
                    sub_component = &root.sub_components[sub_component.sub_components[*i].ty];
                }
                item_tree_array.push(quote!(
                    sp::ItemTreeNode::DynamicTree {
                        index: #repeater_index,
                        parent_index: #parent_index,
                    }
                ));
            }
            Either::Left(item_index) => {
                let item = &component.items[item_index];
                let field = access_component_field_offset(
                    &self::inner_component_id(component),
                    &ident(&item.name),
                );

                let children_count = node.children.len() as u32;
                let children_index = children_offset as u32;
                let item_array_len = item_array.len() as u32;
                let is_accessible = node.is_accessible;
                item_tree_array.push(quote!(
                    sp::ItemTreeNode::Item {
                        is_accessible: #is_accessible,
                        children_count: #children_count,
                        children_index: #children_index,
                        parent_index: #parent_index,
                        item_array_index: #item_array_len,
                    }
                ));
                item_array.push(quote!(sp::VOffset::new(#path #field)));
            }
        }
    });

    let item_tree_array_len = item_tree_array.len();
    let item_array_len = item_array.len();

    let element_info_body = if root.has_debug_info {
        quote!(
            *_result = self.item_element_infos(_index).unwrap_or_default();
            true
        )
    } else {
        quote!(false)
    };

    quote!(
        #sub_comp

        impl #inner_component_id {
            fn new(#(parent: #parent_component_type,)* #globals_arg) -> core::result::Result<sp::VRc<sp::ItemTreeVTable, Self>, slint::PlatformError> {
                #![allow(unused)]
                slint::private_unstable_api::ensure_backend()?;
                let mut _self = Self::default();
                #(_self.parent = parent.clone() as #parent_component_type;)*
                let self_rc = sp::VRc::new(_self);
                let self_dyn_rc = sp::VRc::into_dyn(self_rc.clone());
                let globals = #globals;
                sp::register_item_tree(&self_dyn_rc, globals.maybe_window_adapter_impl());
                Self::init(sp::VRc::map(self_rc.clone(), |x| x), globals, 0, 1);
                core::result::Result::Ok(self_rc)
            }

            fn item_tree() -> &'static [sp::ItemTreeNode] {
                const ITEM_TREE : [sp::ItemTreeNode; #item_tree_array_len] = [#(#item_tree_array),*];
                &ITEM_TREE
            }

            fn item_array() -> &'static [sp::VOffset<Self, sp::ItemVTable, sp::AllowPin>] {
                // FIXME: ideally this should be a const, but we can't because of the pointer to the vtable
                static ITEM_ARRAY : sp::OnceBox<
                    [sp::VOffset<#inner_component_id, sp::ItemVTable, sp::AllowPin>; #item_array_len]
                > = sp::OnceBox::new();
                &*ITEM_ARRAY.get_or_init(|| sp::vec![#(#item_array),*].into_boxed_slice().try_into().unwrap())
            }
        }

        const _ : () = {
            use slint::private_unstable_api::re_exports::*;
            ItemTreeVTable_static!(static VT for self::#inner_component_id);
        };

        impl sp::PinnedDrop for #inner_component_id {
            fn drop(self: core::pin::Pin<&mut #inner_component_id>) {
                sp::vtable::new_vref!(let vref : VRef<sp::ItemTreeVTable> for sp::ItemTree = self.as_ref().get_ref());
                if let Some(wa) = self.globals.get().unwrap().maybe_window_adapter_impl() {
                    sp::unregister_item_tree(self.as_ref(), vref, Self::item_array(), &wa);
                }
            }
        }

        impl sp::ItemTree for #inner_component_id {
            fn visit_children_item(self: ::core::pin::Pin<&Self>, index: isize, order: sp::TraversalOrder, visitor: sp::ItemVisitorRefMut<'_>)
                -> sp::VisitChildrenResult
            {
                return sp::visit_item_tree(self, &sp::VRcMapped::origin(&self.as_ref().self_weak.get().unwrap().upgrade().unwrap()), self.get_item_tree().as_slice(), index, order, visitor, visit_dynamic);
                #[allow(unused)]
                fn visit_dynamic(_self: ::core::pin::Pin<&#inner_component_id>, order: sp::TraversalOrder, visitor: sp::ItemVisitorRefMut<'_>, dyn_index: u32) -> sp::VisitChildrenResult  {
                    _self.visit_dynamic_children(dyn_index, order, visitor)
                }
            }

            fn get_item_ref(self: ::core::pin::Pin<&Self>, index: u32) -> ::core::pin::Pin<sp::ItemRef<'_>> {
                match &self.get_item_tree().as_slice()[index as usize] {
                    sp::ItemTreeNode::Item { item_array_index, .. } => {
                        Self::item_array()[*item_array_index as usize].apply_pin(self)
                    }
                    sp::ItemTreeNode::DynamicTree { .. } => panic!("get_item_ref called on dynamic tree"),

                }
            }

            fn get_item_tree(
                self: ::core::pin::Pin<&Self>) -> sp::Slice<'_, sp::ItemTreeNode>
            {
                Self::item_tree().into()
            }

            fn get_subtree_range(
                self: ::core::pin::Pin<&Self>, index: u32) -> sp::IndexRange
            {
                self.subtree_range(index)
            }

            fn get_subtree(
                self: ::core::pin::Pin<&Self>, index: u32, subtree_index: usize, result: &mut sp::ItemTreeWeak)
            {
                self.subtree_component(index, subtree_index, result);
            }

            fn subtree_index(
                self: ::core::pin::Pin<&Self>) -> usize
            {
                self.index_property()
            }

            fn parent_node(self: ::core::pin::Pin<&Self>, _result: &mut sp::ItemWeak) {
                #parent_item_expression
            }

            fn embed_component(self: ::core::pin::Pin<&Self>, _parent_component: &sp::ItemTreeWeak, _item_tree_index: u32) -> bool {
                #embedding_function
            }

            fn layout_info(self: ::core::pin::Pin<&Self>, orientation: sp::Orientation) -> sp::LayoutInfo {
                self.layout_info(orientation)
            }

            fn item_geometry(self: ::core::pin::Pin<&Self>, index: u32) -> sp::LogicalRect {
                self.item_geometry(index)
            }

            fn accessible_role(self: ::core::pin::Pin<&Self>, index: u32) -> sp::AccessibleRole {
                self.accessible_role(index)
            }

            fn accessible_string_property(
                self: ::core::pin::Pin<&Self>,
                index: u32,
                what: sp::AccessibleStringProperty,
                result: &mut sp::SharedString,
            ) -> bool {
                if let Some(r) = self.accessible_string_property(index, what) {
                    *result = r;
                    true
                } else {
                    false
                }
            }

            fn accessibility_action(self: ::core::pin::Pin<&Self>, index: u32, action: &sp::AccessibilityAction) {
                self.accessibility_action(index, action);
            }

            fn supported_accessibility_actions(self: ::core::pin::Pin<&Self>, index: u32) -> sp::SupportedAccessibilityAction {
                self.supported_accessibility_actions(index)
            }

            fn item_element_infos(
                self: ::core::pin::Pin<&Self>,
                _index: u32,
                _result: &mut sp::SharedString,
            ) -> bool {
                #element_info_body
            }

            fn window_adapter(
                self: ::core::pin::Pin<&Self>,
                do_create: bool,
                result: &mut sp::Option<sp::Rc<dyn sp::WindowAdapter>>,
            ) {
                if do_create {
                    *result = sp::Some(self.globals.get().unwrap().window_adapter_impl());
                } else {
                    *result = self.globals.get().unwrap().maybe_window_adapter_impl();
                }
            }
        }


    )
}

fn generate_repeated_component(
    repeated: &llr::RepeatedElement,
    unit: &llr::CompilationUnit,
    parent_ctx: ParentCtx,
) -> TokenStream {
    let component =
        generate_item_tree(&repeated.sub_tree, unit, Some(parent_ctx), repeated.index_prop, false);

    let ctx = EvaluationContext {
        compilation_unit: unit,
        current_sub_component: Some(repeated.sub_tree.root),
        current_global: None,
        generator_state: RustGeneratorContext { global_access: quote!(_self) },
        parent: Some(parent_ctx),
        argument_types: &[],
    };

    let root_sc = &unit.sub_components[repeated.sub_tree.root];
    let inner_component_id = self::inner_component_id(root_sc);

    let extra_fn = if let Some(listview) = &repeated.listview {
        let p_y = access_member(&listview.prop_y, &ctx).unwrap();
        let p_height = access_member(&listview.prop_height, &ctx).unwrap();
        quote! {
            fn listview_layout(
                self: ::core::pin::Pin<&Self>,
                offset_y: &mut sp::LogicalLength,
            ) -> sp::LogicalLength {
                let _self = self;
                #p_y.set(*offset_y);
                *offset_y += #p_height.get();
                sp::LogicalLength::new(self.as_ref().layout_info(sp::Orientation::Horizontal).min)
            }
        }
    } else {
        // TODO: we could generate this code only if we know that this component is in a box layout
        quote! {
            fn box_layout_data(self: ::core::pin::Pin<&Self>, o: sp::Orientation)
                -> sp::BoxLayoutCellData
            {
                sp::BoxLayoutCellData { constraint: self.as_ref().layout_info(o) }
            }
        }
    };

    let data_type = if let Some(data_prop) = repeated.data_prop {
        rust_primitive_type(&root_sc.properties[data_prop].ty).unwrap()
    } else {
        quote!(())
    };

    let access_prop = |property_index| {
        access_member(
            &llr::PropertyReference::Local { sub_component_path: vec![], property_index },
            &ctx,
        )
        .unwrap()
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

        impl sp::RepeatedItemTree for #inner_component_id {
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
                    sp::VRcMapped::map(self_rc, |x| x),
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

fn internal_popup_id(index: usize) -> proc_macro2::Ident {
    let mut name = index.to_string();
    name.insert_str(0, "popup_id_");
    ident(&name)
}

fn global_inner_name(g: &llr::GlobalComponent) -> TokenStream {
    if g.is_builtin {
        let i = ident(&g.name);
        quote!(sp::#i)
    } else {
        let i = format_ident!("Inner{}", ident(&g.name));
        quote!(#i)
    }
}

fn property_set_value_tokens(
    property: &llr::PropertyReference,
    value_tokens: TokenStream,
    ctx: &EvaluationContext,
) -> TokenStream {
    let prop = access_member(property, ctx);
    let prop_type = ctx.property_ty(property);
    let value_tokens = set_primitive_property_value(prop_type, value_tokens);
    if let Some((animation, map)) = &ctx.property_info(property).animation {
        let mut animation = (*animation).clone();
        map.map_expression(&mut animation);
        let animation_tokens = compile_expression(&animation, ctx);
        return prop
            .then(|prop| quote!(#prop.set_animated_value(#value_tokens as _, #animation_tokens)));
    }
    prop.then(|prop| quote!(#prop.set(#value_tokens as _)))
}

/// Returns the code that can access the given property or callback
fn access_member(reference: &llr::PropertyReference, ctx: &EvaluationContext) -> MemberAccess {
    fn in_native_item(
        ctx: &EvaluationContext,
        sub_component_path: &[llr::SubComponentInstanceIdx],
        item_index: llr::ItemInstanceIdx,
        prop_name: &str,
        path: TokenStream,
    ) -> (TokenStream, Option<TokenStream>) {
        let (compo_path, sub_component) = follow_sub_component_path(
            ctx.compilation_unit,
            ctx.current_sub_component.unwrap(),
            sub_component_path,
        );
        let component_id = inner_component_id(sub_component);
        let item_name = ident(&sub_component.items[item_index].name);
        let item_field = access_component_field_offset(&component_id, &item_name);
        if prop_name.is_empty() {
            // then this is actually a reference to the element itself
            (quote!((#compo_path #item_field).apply_pin(#path)), None)
        } else if matches!(
            sub_component.items[item_index].ty.lookup_property(prop_name),
            Some(&Type::Function(..))
        ) {
            let property_name = ident(prop_name);
            (quote!((#compo_path #item_field).apply_pin(#path)), Some(quote!(.#property_name)))
        } else {
            let property_name = ident(prop_name);
            let item_ty = ident(&sub_component.items[item_index].ty.class_name);
            (
                quote!((#compo_path #item_field + sp::#item_ty::FIELD_OFFSETS.#property_name).apply_pin(#path)),
                None,
            )
        }
    }
    match reference {
        llr::PropertyReference::Local { sub_component_path, property_index } => {
            if let Some(sub_component) = ctx.current_sub_component {
                let (compo_path, sub_component) = follow_sub_component_path(
                    ctx.compilation_unit,
                    sub_component,
                    sub_component_path,
                );
                let component_id = inner_component_id(sub_component);
                let property_name = ident(&sub_component.properties[*property_index].name);
                let property_field = access_component_field_offset(&component_id, &property_name);
                MemberAccess::Direct(quote!((#compo_path #property_field).apply_pin(_self)))
            } else if let Some(current_global) = ctx.current_global() {
                let global_name = global_inner_name(current_global);
                let property_name = ident(&current_global.properties[*property_index].name);
                let property_field = quote!({ *&#global_name::FIELD_OFFSETS.#property_name });
                MemberAccess::Direct(quote!(#property_field.apply_pin(_self)))
            } else {
                unreachable!()
            }
        }
        llr::PropertyReference::InNativeItem { sub_component_path, item_index, prop_name } => {
            let (a, b) =
                in_native_item(ctx, sub_component_path, *item_index, prop_name, quote!(_self));
            MemberAccess::Direct(quote!(#a #b))
        }
        llr::PropertyReference::InParent { level, parent_reference } => {
            let mut path = quote!(_self.parent.upgrade());
            let mut ctx = ctx.parent.as_ref().unwrap().ctx;
            for _ in 1..level.get() {
                path = quote!(#path.and_then(|x| x.parent.upgrade()));
                ctx = ctx.parent.as_ref().unwrap().ctx;
            }

            match &**parent_reference {
                llr::PropertyReference::Local { sub_component_path, property_index } => {
                    let sub_component = ctx.current_sub_component.unwrap();
                    let (compo_path, sub_component) = follow_sub_component_path(
                        ctx.compilation_unit,
                        sub_component,
                        sub_component_path,
                    );
                    let component_id = inner_component_id(sub_component);
                    let property_name = ident(&sub_component.properties[*property_index].name);
                    MemberAccess::Option(
                        quote!((#path.as_ref().map(|x| (#compo_path #component_id::FIELD_OFFSETS.#property_name).apply_pin(x.as_pin_ref())))),
                    )
                }
                llr::PropertyReference::InNativeItem {
                    sub_component_path,
                    item_index,
                    prop_name,
                } => {
                    let (a, b) = in_native_item(
                        ctx,
                        sub_component_path,
                        *item_index,
                        prop_name,
                        quote!(x.as_pin_ref()),
                    );
                    let opt = quote!(#path.as_ref().map(|x| #a));
                    match b {
                        None => MemberAccess::Option(opt),
                        Some(b) => MemberAccess::OptionFn(opt, quote!(|x| x #b)),
                    }
                }
                llr::PropertyReference::Function { sub_component_path, function_index } => {
                    let mut sub_component = ctx.current_sub_component().unwrap();

                    let mut compo_path = quote!(x.as_pin_ref());
                    for i in sub_component_path {
                        let component_id = inner_component_id(sub_component);
                        let sub_component_name = ident(&sub_component.sub_components[*i].name);
                        compo_path = quote!( #component_id::FIELD_OFFSETS.#sub_component_name.apply_pin(#compo_path));
                        sub_component = &ctx.compilation_unit.sub_components
                            [sub_component.sub_components[*i].ty];
                    }
                    let fn_id =
                        ident(&format!("fn_{}", sub_component.functions[*function_index].name));
                    MemberAccess::OptionFn(path, quote!(|x| #compo_path.#fn_id))
                }
                llr::PropertyReference::InParent { .. }
                | llr::PropertyReference::Global { .. }
                | llr::PropertyReference::GlobalFunction { .. } => {
                    unreachable!()
                }
            }
        }
        llr::PropertyReference::Global { global_index, property_index } => {
            let global_access = &ctx.generator_state.global_access;
            let global = &ctx.compilation_unit.globals[*global_index];
            let global_id = format_ident!("global_{}", ident(&global.name));
            let global_name = global_inner_name(global);
            let property_name = ident(
                &ctx.compilation_unit.globals[*global_index].properties[*property_index].name,
            );
            MemberAccess::Direct(
                quote!(#global_name::FIELD_OFFSETS.#property_name.apply_pin(#global_access.#global_id.as_ref())),
            )
        }
        llr::PropertyReference::Function { sub_component_path, function_index } => {
            if let Some(mut sub_component) = ctx.current_sub_component() {
                let mut compo_path = quote!(_self);
                for i in sub_component_path {
                    let component_id = inner_component_id(sub_component);
                    let sub_component_name = ident(&sub_component.sub_components[*i].name);
                    compo_path = quote!( #component_id::FIELD_OFFSETS.#sub_component_name.apply_pin(#compo_path));
                    sub_component =
                        &ctx.compilation_unit.sub_components[sub_component.sub_components[*i].ty];
                }
                let fn_id = ident(&format!("fn_{}", sub_component.functions[*function_index].name));
                MemberAccess::Direct(quote!(#compo_path.#fn_id))
            } else if let Some(current_global) = ctx.current_global() {
                let fn_id =
                    ident(&format!("fn_{}", current_global.functions[*function_index].name));
                MemberAccess::Direct(quote!(_self.#fn_id))
            } else {
                unreachable!()
            }
        }
        llr::PropertyReference::GlobalFunction { global_index, function_index } => {
            let global_access = &ctx.generator_state.global_access;
            let global = &ctx.compilation_unit.globals[*global_index];
            let global_id = format_ident!("global_{}", ident(&global.name));
            let fn_id = ident(&format!(
                "fn_{}",
                ctx.compilation_unit.globals[*global_index].functions[*function_index].name
            ));
            MemberAccess::Direct(quote!(#global_access.#global_id.as_ref().#fn_id))
        }
    }
}

/// Helper to access a member property/callback of a component.
///
/// Because the parent can be deleted (issue #3464), this might be an option when accessing the parent
#[derive(Clone)]
enum MemberAccess {
    /// The token stream is just an expression to the member
    Direct(TokenStream),
    /// The token stream is a an expression to an option of the member
    Option(TokenStream),
    /// the first token stream is an option, and the second is a path to the function in a `.map` and it must be called
    OptionFn(TokenStream, TokenStream),
}

impl MemberAccess {
    /// Used for code that is meant to return `()`
    fn then(self, f: impl FnOnce(TokenStream) -> TokenStream) -> TokenStream {
        match self {
            MemberAccess::Direct(t) => f(t),
            MemberAccess::Option(t) => {
                let r = f(quote!(x));
                quote!({ let _ = #t.map(|x| #r); })
            }
            MemberAccess::OptionFn(opt, inner) => {
                let r = f(inner);
                quote!({ let _ = #opt.as_ref().map(#r); })
            }
        }
    }

    fn map_or_default(self, f: impl FnOnce(TokenStream) -> TokenStream) -> TokenStream {
        match self {
            MemberAccess::Direct(t) => f(t),
            MemberAccess::Option(t) => {
                let r = f(quote!(x));
                quote!(#t.map(|x| #r).unwrap_or_default())
            }
            MemberAccess::OptionFn(opt, inner) => {
                let r = f(inner);
                quote!(#opt.as_ref().map(#r).unwrap_or_default())
            }
        }
    }

    fn get_property(self) -> TokenStream {
        match self {
            MemberAccess::Direct(t) => quote!(#t.get()),
            MemberAccess::Option(t) => {
                quote!(#t.map(|x| x.get()).unwrap_or_default())
            }
            MemberAccess::OptionFn(..) => panic!("function is not a property"),
        }
    }

    /// To be used when we know that the reference was local
    #[track_caller]
    fn unwrap(&self) -> TokenStream {
        match self {
            MemberAccess::Direct(t) => quote!(#t),
            _ => panic!("not a local property?"),
        }
    }
}

fn follow_sub_component_path<'a>(
    compilation_unit: &'a llr::CompilationUnit,
    root: llr::SubComponentIdx,
    sub_component_path: &[llr::SubComponentInstanceIdx],
) -> (TokenStream, &'a llr::SubComponent) {
    let mut compo_path = quote!();
    let mut sub_component = &compilation_unit.sub_components[root];
    for i in sub_component_path {
        let component_id = inner_component_id(sub_component);
        let sub_component_name = ident(&sub_component.sub_components[*i].name);
        compo_path = quote!(#compo_path {#component_id::FIELD_OFFSETS.#sub_component_name} +);
        sub_component = &compilation_unit.sub_components[sub_component.sub_components[*i].ty];
    }
    (compo_path, sub_component)
}

fn access_window_adapter_field(ctx: &EvaluationContext) -> TokenStream {
    let global_access = &ctx.generator_state.global_access;
    quote!((&#global_access.window_adapter_impl()))
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
        llr::PropertyReference::InNativeItem { sub_component_path, item_index, prop_name: _ } => {
            let root = ctx.current_sub_component().unwrap();
            let mut sub_component = root;
            for i in sub_component_path {
                let sub_component_name = ident(&sub_component.sub_components[*i].name);
                component_access_tokens = quote!(#component_access_tokens . #sub_component_name);
                sub_component =
                    &ctx.compilation_unit.sub_components[sub_component.sub_components[*i].ty];
            }
            let component_rc_tokens = quote!(sp::VRcMapped::origin(&#component_access_tokens.self_weak.get().unwrap().upgrade().unwrap()));
            let item_index_in_tree = sub_component.items[*item_index].index_in_tree;
            let item_index_tokens = if item_index_in_tree == 0 {
                quote!(#component_access_tokens.tree_index.get())
            } else {
                quote!(#component_access_tokens.tree_index_of_first_child.get() + #item_index_in_tree - 1)
            };

            quote!(&sp::ItemRc::new(#component_rc_tokens, #item_index_tokens))
        }
        _ => unreachable!(),
    }
}

fn compile_expression(expr: &Expression, ctx: &EvaluationContext) -> TokenStream {
    match expr {
        Expression::StringLiteral(s) => {
            let s = s.as_str();
            quote!(sp::SharedString::from(#s))
        }
        Expression::NumberLiteral(n) if n.is_finite() => quote!(#n),
        Expression::NumberLiteral(_) => quote!(0.),
        Expression::BoolLiteral(b) => quote!(#b),
        Expression::Cast { from, to } => {
            let f = compile_expression(from, ctx);
            match (from.ty(ctx), to) {
                (Type::Float32, Type::Int32) => {
                    quote!((#f as i32))
                }
                (from, Type::String) if from.as_unit_product().is_some() => {
                    quote!(sp::shared_string_from_number((#f) as f64))
                }
                (Type::Float32, Type::Model) | (Type::Int32, Type::Model) => {
                    quote!(sp::ModelRc::new(#f.max(::core::default::Default::default()) as usize))
                }
                (Type::Float32, Type::Color) => {
                    quote!(sp::Color::from_argb_encoded(#f as u32))
                }
                (Type::Color, Type::Brush) => {
                    quote!(slint::Brush::SolidColor(#f))
                }
                (Type::Brush, Type::Color) => {
                    quote!(#f.color())
                }
                (Type::Struct (lhs), Type::Struct (rhs)) if rhs.name.is_some() => {
                    let fields = lhs.fields.iter().enumerate().map(|(index, (name, _))| {
                        let index = proc_macro2::Literal::usize_unsuffixed(index);
                        let name = ident(name);
                        quote!(the_struct.#name =  obj.#index as _;)
                    });
                    let id = struct_name_to_tokens(rhs.name.as_ref().unwrap());
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
                                // Close{} is a struct with no fields in markup, and PathElement::Close has no fields
                                if matches!(path_elem_expr, Expression::Struct { ty, .. } if ty.fields.is_empty()) {
                                    quote!(sp::PathElement::Close)
                                } else {
                                    compile_expression(path_elem_expr, ctx)
                                }
                            ),
                        _ => {
                            unreachable!()
                        }
                    };
                    quote!(sp::PathData::Elements(sp::SharedVector::<_>::from_slice(&[#((#path_elements).into()),*])))
                }
                (Type::Struct { .. }, Type::PathData) if matches!(from.as_ref(), Expression::Struct { .. }) => {
                    let (events, points) = match from.as_ref() {
                        Expression::Struct { ty: _, values } => (
                            compile_expression(&values["events"], ctx),
                            compile_expression(&values["points"], ctx),
                        ),
                        _ => {
                            unreachable!()
                        }
                    };
                    quote!(sp::PathData::Events(sp::SharedVector::<_>::from_slice(&#events), sp::SharedVector::<_>::from_slice(&#points)))
                }
                (Type::String, Type::PathData) => {
                    quote!(sp::PathData::Commands(#f))
                }
                (_, Type::Void) => {
                    quote!(#f;)
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
            if expr.ty(ctx) == Type::Void {
                f.then(|f| quote!(#f.call(&(#(#a as _,)*))))
            } else {
                f.map_or_default(|f| quote!(#f.call(&(#(#a as _,)*))))
            }
        }
        Expression::FunctionCall { function, arguments } => {
            let a = arguments.iter().map(|a| compile_expression(a, ctx));
            let f = access_member(function, ctx);
            if expr.ty(ctx) == Type::Void {
                f.then(|f| quote!(#f( #(#a as _),*)))
            } else {
                f.map_or_default(|f| quote!(#f( #(#a as _),*)))
            }
        }
        Expression::ItemMemberFunctionCall { function } => {
            let fun = access_member(function, ctx);
            let item_rc = access_item_rc(function, ctx);
            let window_adapter_tokens = access_window_adapter_field(ctx);
            fun.map_or_default(|fun| quote!(#fun(#window_adapter_tokens, #item_rc)))
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
            quote! { sp::#f(#(#a as _),*) }
        }
        Expression::FunctionParameterReference { index } => {
            let i = proc_macro2::Literal::usize_unsuffixed(*index);
            quote! {args.#i.clone()}
        }
        Expression::StructFieldAccess { base, name } => match base.ty(ctx) {
            Type::Struct (s) if s.name.is_none() => {
                let index = s.fields
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
                let x = ctx2.parent.unwrap();
                ctx2 = x.ctx;
                repeater_index = x.repeater_index;
                path = quote!(#path.parent.upgrade().unwrap());
            }
            let repeater_index = repeater_index.unwrap();
            let mut index_prop = llr::PropertyReference::Local {
                sub_component_path: vec![],
                property_index: ctx2.current_sub_component().unwrap().repeated[repeater_index].index_prop.unwrap(),
            };
            if let Some(level) = NonZeroUsize::new(*level) {
                index_prop =
                    llr::PropertyReference::InParent { level, parent_reference: index_prop.into() };
            }
            let index_access = access_member(&index_prop, ctx).get_property();
            let repeater = access_component_field_offset(
                &inner_component_id(ctx2.current_sub_component().unwrap()),
                &format_ident!("repeater{}", usize::from(repeater_index)),
            );
            quote!(#repeater.apply_pin(#path.as_pin_ref()).model_set_row_data(#index_access as _, #value as _))
        }
        Expression::ArrayIndexAssignment { array, index, value } => {
            debug_assert!(matches!(array.ty(ctx), Type::Array(_)));
            let base_e = compile_expression(array, ctx);
            let index_e = compile_expression(index, ctx);
            let value_e = compile_expression(value, ctx);
            quote!((#base_e).set_row_data(#index_e as isize as usize, #value_e as _))
        }
        Expression::BinaryExpression { lhs, rhs, op } => {
            let lhs_ty = lhs.ty(ctx);
            let lhs = compile_expression(lhs, ctx);
            let rhs = compile_expression(rhs, ctx);

            if lhs_ty.as_unit_product().is_some() && (*op == '=' || *op == '!') {
                let maybe_negate = if *op == '!' { quote!(!) } else { quote!() };
                quote!(#maybe_negate sp::ApproxEq::<f64>::approx_eq(&(#lhs as f64), &(#rhs as f64)))
            } else {
                let (conv1, conv2) = match crate::expression_tree::operator_class(*op) {
                    OperatorClass::ArithmeticOp => match lhs_ty {
                        Type::String => (None, Some(quote!(.as_str()))),
                        Type::Struct { .. } => (None, None),
                        _ => (Some(quote!(as f64)), Some(quote!(as f64))),
                    },
                    OperatorClass::ComparisonOp
                        if matches!(
                            lhs_ty,
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
        }
        Expression::UnaryOp { sub, op } => {
            let sub = compile_expression(sub, ctx);
            if *op == '+' {
                // there is no unary '+' in rust
                return sub;
            }
            let op = proc_macro2::Punct::new(*op, proc_macro2::Spacing::Alone);
            quote!( (#op #sub) )
        }
        Expression::ImageReference { resource_ref, nine_slice } => {
            let image = match resource_ref {
                crate::expression_tree::ImageReference::None => {
                    quote!(sp::Image::default())
                }
                crate::expression_tree::ImageReference::AbsolutePath(path) => {
                    let path = path.as_str();
                    quote!(sp::Image::load_from_path(::std::path::Path::new(#path)).unwrap_or_default())
                }
                crate::expression_tree::ImageReference::EmbeddedData { resource_id, extension } => {
                    let symbol = format_ident!("SLINT_EMBEDDED_RESOURCE_{}", resource_id);
                    let format = proc_macro2::Literal::byte_string(extension.as_bytes());
                    quote!(sp::load_image_from_embedded_data(#symbol.into(), sp::Slice::from_slice(#format)))
                }
                crate::expression_tree::ImageReference::EmbeddedTexture { resource_id } => {
                    let symbol = format_ident!("SLINT_EMBEDDED_RESOURCE_{}", resource_id);
                    quote!(
                        sp::Image::from(sp::ImageInner::StaticTextures(&#symbol))
                    )
                }
            };
            match &nine_slice {
                Some([a, b, c, d]) => {
                    quote! {{ let mut image = #image; image.set_nine_slice_edges(#a, #b, #c, #d); image }}
                }
                None => image,
            }
        }
        Expression::Condition { condition, true_expr, false_expr } => {
            let condition_code = compile_expression(condition, ctx);
            let true_code = compile_expression(true_expr, ctx);
            let false_code = compile_expression(false_expr, ctx);
            let semi = if false_expr.ty(ctx) == Type::Void { quote!(;) } else { quote!(as _) };
            quote!(
                if #condition_code {
                    (#true_code) #semi
                } else {
                    #false_code
                }
            )
        }
        Expression::Array { values, element_ty, as_model } => {
            let val = values.iter().map(|e| compile_expression(e, ctx));
            if *as_model {
                let rust_element_ty = rust_primitive_type(element_ty).unwrap();
                quote!(sp::ModelRc::new(
                    sp::VecModel::<#rust_element_ty>::from(
                        sp::vec![#(#val as _),*]
                    )
                ))
            } else {
                quote!(sp::Slice::from_slice(&[#(#val),*]))
            }
        }
        Expression::Struct { ty, values } => {
            let elem = ty.fields.keys().map(|k| values.get(k).map(|e| compile_expression(e, ctx)));
            if let Some(name) = &ty.name {
                let name_tokens: TokenStream = struct_name_to_tokens(name.as_str());
                let keys = ty.fields.keys().map(|k| ident(k));
                if name.starts_with("slint::private_api::") && name.ends_with("LayoutData") {
                    quote!(#name_tokens{#(#keys: #elem as _,)*})
                } else {
                    quote!({ let mut the_struct = #name_tokens::default(); #(the_struct.#keys =  #elem as _;)* the_struct})
                }
            } else {
                let as_ = ty.fields.values().map(|t| {
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
            quote!(sp::EasingCurve::Linear)
        }
        Expression::EasingCurve(EasingCurve::CubicBezier(a, b, c, d)) => {
            quote!(sp::EasingCurve::CubicBezier([#a, #b, #c, #d]))
        }
        Expression::EasingCurve(EasingCurve::EaseInElastic) => {
            quote!(sp::EasingCurve::EaseInElastic)
        }
        Expression::EasingCurve(EasingCurve::EaseOutElastic) => {
            quote!(sp::EasingCurve::EaseOutElastic)
        }
        Expression::EasingCurve(EasingCurve::EaseInOutElastic) => {
            quote!(sp::EasingCurve::EaseInOutElastic)
        }
        Expression::EasingCurve(EasingCurve::EaseInBounce) => {
            quote!(sp::EasingCurve::EaseInBounce)
        }
        Expression::EasingCurve(EasingCurve::EaseOutBounce) => {
            quote!(sp::EasingCurve::EaseOutBounce)
        }
        Expression::EasingCurve(EasingCurve::EaseInOutBounce) => {
            quote!(sp::EasingCurve::EaseInOutBounce)
        }
        Expression::LinearGradient { angle, stops } => {
            let angle = compile_expression(angle, ctx);
            let stops = stops.iter().map(|(color, stop)| {
                let color = compile_expression(color, ctx);
                let position = compile_expression(stop, ctx);
                quote!(sp::GradientStop{ color: #color, position: #position as _ })
            });
            quote!(slint::Brush::LinearGradient(
                sp::LinearGradientBrush::new(#angle as _, [#(#stops),*])
            ))
        }
        Expression::RadialGradient { stops } => {
            let stops = stops.iter().map(|(color, stop)| {
                let color = compile_expression(color, ctx);
                let position = compile_expression(stop, ctx);
                quote!(sp::GradientStop{ color: #color, position: #position as _ })
            });
            quote!(slint::Brush::RadialGradient(
                sp::RadialGradientBrush::new_circle([#(#stops),*])
            ))
        }
        Expression::EnumerationValue(value) => {
            let base_ident = ident(&value.enumeration.name);
            let value_ident = ident(&value.to_pascal_case());
            if value.enumeration.node.is_some() {
                quote!(#base_ident::#value_ident)
            } else {
                quote!(sp::#base_ident::#value_ident)
            }
        }
        Expression::LayoutCacheAccess { layout_cache_prop, index, repeater_index } => {
            access_member(layout_cache_prop, ctx).map_or_default(|cache| {
                if let Some(ri) = repeater_index {
                    let offset = compile_expression(ri, ctx);
                    quote!({
                        let cache = #cache.get();
                        *cache.get((cache[#index] as usize) + #offset as usize * 2).unwrap_or(&(0 as sp::Coord))
                    })
                } else {
                    quote!(#cache.get()[#index])
                }
            })
        }
        Expression::BoxLayoutFunction {
            cells_variable,
            repeater_indices,
            elements,
            orientation,
            sub_expression,
        } => box_layout_function(
            cells_variable,
            repeater_indices.as_ref().map(SmolStr::as_str),
            elements.as_ref(),
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
                sp::reorder_dialog_button_layout(&mut #cells_variable, &#roles);
                let #cells_variable = sp::Slice::from_slice(&#cells_variable);
            }
        }
        Expression::MinMax { ty, op, lhs, rhs } => {
            let t = rust_primitive_type(ty);
            let wrap = |expr| match &t {
                Some(t) => quote!((#expr as #t)),
                None => expr,
            };
            let lhs = wrap(compile_expression(lhs, ctx));
            let rhs = wrap(compile_expression(rhs, ctx));
            match op {
                MinMaxOp::Min => {
                    quote!(#lhs.min(#rhs))
                }
                MinMaxOp::Max => {
                    quote!(#lhs.max(#rhs))
                }
            }
        }
        Expression::EmptyComponentFactory => quote!(slint::ComponentFactory::default()),
        Expression::TranslationReference { format_args, string_index, plural } => {
            let args = compile_expression(format_args, ctx);
            match plural {
                Some(plural) => {
                    let plural = compile_expression(plural, ctx);
                    quote!(sp::translate_from_bundle_with_plural(
                        &self::_SLINT_TRANSLATED_STRINGS_PLURALS[#string_index],
                        &self::_SLINT_TRANSLATED_PLURAL_RULES,
                        sp::Slice::<sp::SharedString>::from(#args).as_slice(),
                        #plural as _
                    ))
                }
                None => quote!(sp::translate_from_bundle(&self::_SLINT_TRANSLATED_STRINGS[#string_index], sp::Slice::<sp::SharedString>::from(#args).as_slice())),

            }
        },
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
                    sp::WindowInner::from_pub(#window_tokens.window()).set_focus_item(#focus_item, true)
                )
            } else {
                panic!("internal error: invalid args to SetFocusItem {arguments:?}")
            }
        }
        BuiltinFunction::ClearFocusItem => {
            if let [Expression::PropertyReference(pr)] = arguments {
                let window_tokens = access_window_adapter_field(ctx);
                let focus_item = access_item_rc(pr, ctx);
                quote!(
                    sp::WindowInner::from_pub(#window_tokens.window()).set_focus_item(#focus_item, false)
                )
            } else {
                panic!("internal error: invalid args to ClearFocusItem {arguments:?}")
            }
        }
        BuiltinFunction::ShowPopupWindow => {
            if let [Expression::NumberLiteral(popup_index), close_policy, Expression::PropertyReference(parent_ref)] =
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
                let current_sub_component = parent_ctx.current_sub_component().unwrap();
                let popup = &current_sub_component.popup_windows[*popup_index as usize];
                let popup_window_id =
                    inner_component_id(&ctx.compilation_unit.sub_components[popup.item_tree.root]);
                let parent_component = access_item_rc(parent_ref, ctx);

                let popup_ctx = EvaluationContext::new_sub_component(
                    ctx.compilation_unit,
                    popup.item_tree.root,
                    RustGeneratorContext { global_access: quote!(_self.globals.get().unwrap()) },
                    Some(ParentCtx::new(ctx, None)),
                );
                let position = compile_expression(&popup.position.borrow(), &popup_ctx);

                let close_policy = compile_expression(close_policy, ctx);
                let window_adapter_tokens = access_window_adapter_field(ctx);
                let popup_id_name = internal_popup_id(*popup_index as usize);
                quote!({
                    let popup_instance = #popup_window_id::new(#component_access_tokens.self_weak.get().unwrap().clone()).unwrap();
                    let popup_instance_vrc = sp::VRc::map(popup_instance.clone(), |x| x);
                    let position = { let _self = popup_instance_vrc.as_pin_ref(); #position };
                    if let Some(current_id) = #component_access_tokens.#popup_id_name.take() {
                        sp::WindowInner::from_pub(#window_adapter_tokens.window()).close_popup(current_id);
                    }
                    #component_access_tokens.#popup_id_name.set(Some(
                        sp::WindowInner::from_pub(#window_adapter_tokens.window()).show_popup(
                            &sp::VRc::into_dyn(popup_instance.into()),
                            position,
                            #close_policy,
                            #parent_component,
                            false, // is_menu
                        ))
                    );
                    #popup_window_id::user_init(popup_instance_vrc.clone());
                })
            } else {
                panic!("internal error: invalid args to ShowPopupWindow {arguments:?}")
            }
        }
        BuiltinFunction::ClosePopupWindow => {
            if let [Expression::NumberLiteral(popup_index), Expression::PropertyReference(parent_ref)] =
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
                let window_adapter_tokens = access_window_adapter_field(ctx);
                let popup_id_name = internal_popup_id(*popup_index as usize);
                quote!(
                    if let Some(current_id) = #component_access_tokens.#popup_id_name.take() {
                        sp::WindowInner::from_pub(#window_adapter_tokens.window()).close_popup(current_id);
                    }
                )
            } else {
                panic!("internal error: invalid args to ClosePopupWindow {arguments:?}")
            }
        }
        BuiltinFunction::ShowPopupMenu => {
            let [Expression::PropertyReference(context_menu_ref), entries, position] = arguments
            else {
                panic!("internal error: invalid args to ShowPopupMenu {arguments:?}")
            };

            let context_menu = access_member(context_menu_ref, ctx);
            let context_menu_rc = access_item_rc(context_menu_ref, ctx);
            let position = compile_expression(position, ctx);

            let popup = ctx
                .compilation_unit
                .popup_menu
                .as_ref()
                .expect("there should be a popup menu if we want to show it");
            let popup_id =
                inner_component_id(&ctx.compilation_unit.sub_components[popup.item_tree.root]);
            let window_adapter_tokens = access_window_adapter_field(ctx);

            let popup_ctx = EvaluationContext::new_sub_component(
                ctx.compilation_unit,
                popup.item_tree.root,
                RustGeneratorContext { global_access: quote!(_self.globals.get().unwrap()) },
                None,
            );
            let access_entries = access_member(&popup.entries, &popup_ctx).unwrap();
            let access_sub_menu = access_member(&popup.sub_menu, &popup_ctx).unwrap();
            let access_activated = access_member(&popup.activated, &popup_ctx).unwrap();
            let access_close = access_member(&popup.close, &popup_ctx).unwrap();

            let close_popup = context_menu.clone().then(|context_menu| quote!{
                if let Some(current_id) = #context_menu.popup_id.take() {
                    sp::WindowInner::from_pub(#window_adapter_tokens.window()).close_popup(current_id);
                }
            });

            let init_popup = if let Expression::NumberLiteral(tree_index) = entries {
                // We have an MenuItem tree
                let current_sub_component = ctx.current_sub_component().unwrap();
                let item_tree_id = inner_component_id(
                    &ctx.compilation_unit.sub_components
                        [current_sub_component.menu_item_trees[*tree_index as usize].root],
                );
                quote!({
                    let menu_item_tree_instance = #item_tree_id::new(_self.self_weak.get().unwrap().clone()).unwrap();
                    let context_menu_item_tree = sp::Rc::new(sp::MenuFromItemTree::new(sp::VRc::into_dyn(menu_item_tree_instance)));
                    let mut entries = sp::SharedVector::default();
                    sp::Menu::sub_menu(&*context_menu_item_tree, sp::Option::None, &mut entries);
                    let _self = popup_instance_vrc.as_pin_ref();
                    #access_entries.set(sp::ModelRc::new(sp::SharedVectorModel::from(entries)));
                    let context_menu_item_tree_ = context_menu_item_tree.clone();
                    #access_sub_menu.set_handler(move |entry| {
                        let mut entries = sp::SharedVector::default();
                        sp::Menu::sub_menu(&*context_menu_item_tree_, sp::Option::Some(&entry.0), &mut entries);
                        sp::ModelRc::new(sp::SharedVectorModel::from(entries))
                    });
                    #access_activated.set_handler(move |entry| {
                        sp::Menu::activate(&*context_menu_item_tree, &entry.0);
                    });
                    let self_weak = parent_weak.clone();
                    #access_close.set_handler(move |()| {
                        let Some(self_rc) = self_weak.upgrade() else { return };
                        let _self = self_rc.as_pin_ref();
                        #close_popup
                    });
                })
            } else {
                // entries should be an expression of type array of MenuEntry
                debug_assert!(
                    matches!(entries.ty(ctx), Type::Array(ty) if matches!(&*ty, Type::Struct{..}))
                );
                let entries = compile_expression(entries, ctx);
                let forward_callback = |access, cb| {
                    let call = context_menu
                        .clone()
                        .map_or_default(|context_menu| quote!(#context_menu.#cb.call(entry)));
                    quote!(
                        let self_weak = parent_weak.clone();
                        #access.set_handler(move |entry| {
                            if let Some(self_rc) = self_weak.upgrade() {
                                let _self = self_rc.as_pin_ref();
                                #call
                            } else { Default::default() }
                        });
                    )
                };
                let fw_sub_menu = forward_callback(access_sub_menu, quote!(sub_menu));
                let fw_activated = forward_callback(access_activated, quote!(activated));
                quote! {
                    let entries = #entries;
                    {
                        let _self = popup_instance_vrc.as_pin_ref();
                        #access_entries.set(entries);
                        #fw_sub_menu
                        #fw_activated
                        let self_weak = parent_weak.clone();
                        #access_close.set_handler(move |()| {
                            let Some(self_rc) = self_weak.upgrade() else { return };
                            let _self = self_rc.as_pin_ref();
                            #close_popup
                        });
                    }
                }
            };
            let context_menu = context_menu.unwrap();
            quote!({
                let position = #position;
                let popup_instance = #popup_id::new(_self.globals.get().unwrap().clone()).unwrap();
                let popup_instance_vrc = sp::VRc::map(popup_instance.clone(), |x| x);
                let parent_weak = _self.self_weak.get().unwrap().clone();
                #init_popup
                #close_popup
                let id = sp::WindowInner::from_pub(#window_adapter_tokens.window()).show_popup(
                    &sp::VRc::into_dyn(popup_instance.into()),
                    position,
                    sp::PopupClosePolicy::CloseOnClickOutside,
                    #context_menu_rc,
                    true, // is_menu
                );
                #context_menu.popup_id.set(Some(id));
                #popup_id::user_init(popup_instance_vrc);
            })
        }
        BuiltinFunction::SetSelectionOffsets => {
            if let [llr::Expression::PropertyReference(pr), from, to] = arguments {
                let item = access_member(pr, ctx);
                let item_rc = access_item_rc(pr, ctx);
                let window_adapter_tokens = access_window_adapter_field(ctx);
                let start = compile_expression(from, ctx);
                let end = compile_expression(to, ctx);

                item.then(|item| quote!(
                    #item.set_selection_offsets(#window_adapter_tokens, #item_rc, #start as i32, #end as i32)
                ))
            } else {
                panic!("internal error: invalid args to set-selection-offsets {arguments:?}")
            }
        }
        BuiltinFunction::ItemFontMetrics => {
            if let [Expression::PropertyReference(pr)] = arguments {
                let item = access_member(pr, ctx);
                let item_rc = access_item_rc(pr, ctx);
                let window_adapter_tokens = access_window_adapter_field(ctx);
                item.then(|item| {
                    quote!(
                        #item.font_metrics(#window_adapter_tokens, #item_rc)
                    )
                })
            } else {
                panic!("internal error: invalid args to ItemMemberFunction {arguments:?}")
            }
        }
        BuiltinFunction::ImplicitLayoutInfo(orient) => {
            if let [Expression::PropertyReference(pr)] = arguments {
                let item = access_member(pr, ctx);
                let window_adapter_tokens = access_window_adapter_field(ctx);
                item.then(|item| {
                    let item_rc = access_item_rc(pr, ctx);
                    quote!(
                        sp::Item::layout_info(#item, #orient, #window_adapter_tokens, &#item_rc)
                    )
                })
            } else {
                panic!("internal error: invalid args to ImplicitLayoutInfo {arguments:?}")
            }
        }
        BuiltinFunction::RegisterCustomFontByPath => {
            if let [Expression::StringLiteral(path)] = arguments {
                let window_adapter_tokens = access_window_adapter_field(ctx);
                let path = path.as_str();
                quote!(#window_adapter_tokens.renderer().register_font_from_path(&std::path::PathBuf::from(#path)).unwrap())
            } else {
                panic!("internal error: invalid args to RegisterCustomFontByPath {arguments:?}")
            }
        }
        BuiltinFunction::RegisterCustomFontByMemory => {
            if let [Expression::NumberLiteral(resource_id)] = &arguments {
                let resource_id: usize = *resource_id as _;
                let symbol = format_ident!("SLINT_EMBEDDED_RESOURCE_{}", resource_id);
                let window_adapter_tokens = access_window_adapter_field(ctx);
                quote!(#window_adapter_tokens.renderer().register_font_from_memory(#symbol.into()).unwrap())
            } else {
                panic!("internal error: invalid args to RegisterCustomFontByMemory {arguments:?}")
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
            quote!(sp::WindowInner::from_pub(#window_adapter_tokens.window()).scale_factor())
        }
        BuiltinFunction::GetWindowDefaultFontSize => {
            let window_adapter_tokens = access_window_adapter_field(ctx);
            quote!(sp::WindowInner::from_pub(#window_adapter_tokens.window()).window_item().unwrap().as_pin_ref().default_font_size().get())
        }
        BuiltinFunction::AnimationTick => {
            quote!(sp::animation_tick())
        }
        BuiltinFunction::Debug => quote!(slint::private_unstable_api::debug(#(#a)*)),
        BuiltinFunction::Mod => {
            let (a1, a2) = (a.next().unwrap(), a.next().unwrap());
            quote!(sp::Euclid::rem_euclid(&(#a1 as f64), &(#a2 as f64)))
        }
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
        BuiltinFunction::ATan2 => {
            let (a1, a2) = (a.next().unwrap(), a.next().unwrap());
            quote!((#a1 as f64).atan2(#a2 as f64).to_degrees())
        }
        BuiltinFunction::Log => {
            let (a1, a2) = (a.next().unwrap(), a.next().unwrap());
            quote!((#a1 as f64).log(#a2 as f64))
        }
        BuiltinFunction::Ln => quote!((#(#a)* as f64).ln()),
        BuiltinFunction::Pow => {
            let (a1, a2) = (a.next().unwrap(), a.next().unwrap());
            quote!((#a1 as f64).powf(#a2 as f64))
        }
        BuiltinFunction::Exp => quote!((#(#a)* as f64).exp()),
        BuiltinFunction::ToFixed => {
            let (a1, a2) = (a.next().unwrap(), a.next().unwrap());
            quote!(sp::shared_string_from_number_fixed(#a1 as f64, (#a2 as i32).max(0) as usize))
        }
        BuiltinFunction::ToPrecision => {
            let (a1, a2) = (a.next().unwrap(), a.next().unwrap());
            quote!(sp::shared_string_from_number_precision(#a1 as f64, (#a2 as i32).max(0) as usize))
        }
        BuiltinFunction::StringToFloat => {
            quote!(#(#a)*.as_str().parse::<f64>().unwrap_or_default())
        }
        BuiltinFunction::StringIsFloat => quote!(#(#a)*.as_str().parse::<f64>().is_ok()),
        BuiltinFunction::StringIsEmpty => quote!(#(#a)*.is_empty()),
        BuiltinFunction::StringCharacterCount => {
            quote!( sp::UnicodeSegmentation::graphemes(#(#a)*.as_str(), true).count() as i32 )
        }
        BuiltinFunction::StringToLowercase => quote!(sp::SharedString::from(#(#a)*.to_lowercase())),
        BuiltinFunction::StringToUppercase => quote!(sp::SharedString::from(#(#a)*.to_uppercase())),
        BuiltinFunction::ColorRgbaStruct => quote!( #(#a)*.to_argb_u8()),
        BuiltinFunction::ColorHsvaStruct => quote!( #(#a)*.to_hsva()),
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
        BuiltinFunction::ColorTransparentize => {
            let x = a.next().unwrap();
            let factor = a.next().unwrap();
            quote!(#x.transparentize(#factor as f32))
        }
        BuiltinFunction::ColorMix => {
            let x = a.next().unwrap();
            let y = a.next().unwrap();
            let factor = a.next().unwrap();
            quote!(#x.mix(&#y.into(), #factor as f32))
        }
        BuiltinFunction::ColorWithAlpha => {
            let x = a.next().unwrap();
            let alpha = a.next().unwrap();
            quote!(#x.with_alpha(#alpha as f32))
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
                let r: u8 = (#r as u32).min(255) as u8;
                let g: u8 = (#g as u32).min(255) as u8;
                let b: u8 = (#b as u32).min(255) as u8;
                let a: u8 = (255. * (#a as f32)).max(0.).min(255.) as u8;
                sp::Color::from_argb_u8(a, r, g, b)
            })
        }
        BuiltinFunction::Hsv => {
            let (h, s, v, a) =
                (a.next().unwrap(), a.next().unwrap(), a.next().unwrap(), a.next().unwrap());
            quote!({
                let s: f32 = (#s as f32).max(0.).min(1.) as f32;
                let v: f32 = (#v as f32).max(0.).min(1.) as f32;
                let a: f32 = (1. * (#a as f32)).max(0.).min(1.) as f32;
                sp::Color::from_hsva(#h as f32, s, v, a)
            })
        }
        BuiltinFunction::ColorScheme => {
            let window_adapter_tokens = access_window_adapter_field(ctx);
            quote!(sp::WindowInner::from_pub(#window_adapter_tokens.window()).color_scheme())
        }
        BuiltinFunction::SupportsNativeMenuBar => {
            let window_adapter_tokens = access_window_adapter_field(ctx);
            quote!(sp::WindowInner::from_pub(#window_adapter_tokens.window()).supports_native_menu_bar())
        }
        BuiltinFunction::SetupNativeMenuBar => {
            let window_adapter_tokens = access_window_adapter_field(ctx);
            if let [Expression::PropertyReference(entries_r), Expression::PropertyReference(sub_menu_r), Expression::PropertyReference(activated_r), Expression::NumberLiteral(tree_index), Expression::BoolLiteral(no_native)] =
                arguments
            {
                // We have an MenuItem tree
                let current_sub_component = ctx.current_sub_component().unwrap();
                let item_tree_id = inner_component_id(
                    &ctx.compilation_unit.sub_components
                        [current_sub_component.menu_item_trees[*tree_index as usize].root],
                );

                let access_entries = access_member(entries_r, ctx).unwrap();
                let access_sub_menu = access_member(sub_menu_r, ctx).unwrap();
                let access_activated = access_member(activated_r, ctx).unwrap();

                let native_impl = if *no_native {
                    quote!()
                } else {
                    quote!(if sp::WindowInner::from_pub(#window_adapter_tokens.window()).supports_native_menu_bar() {
                        sp::WindowInner::from_pub(#window_adapter_tokens.window()).setup_menubar(sp::VBox::new(menu_item_tree));
                    } else)
                };

                quote!({
                    let menu_item_tree_instance = #item_tree_id::new(_self.self_weak.get().unwrap().clone()).unwrap();
                    let menu_item_tree = sp::MenuFromItemTree::new(sp::VRc::into_dyn(menu_item_tree_instance));
                    #native_impl
                    /*else*/ {
                        let menu_item_tree = sp::Rc::new(menu_item_tree);
                        let menu_item_tree_ = menu_item_tree.clone();
                        #access_entries.set_binding(move || {
                            let mut entries = sp::SharedVector::default();
                            sp::Menu::sub_menu(&*menu_item_tree_, sp::Option::None, &mut entries);
                            sp::ModelRc::new(sp::SharedVectorModel::from(entries))
                        });
                        let menu_item_tree_ = menu_item_tree.clone();
                        #access_sub_menu.set_handler(move |entry| {
                            let mut entries = sp::SharedVector::default();
                            sp::Menu::sub_menu(&*menu_item_tree_, sp::Option::Some(&entry.0), &mut entries);
                            sp::ModelRc::new(sp::SharedVectorModel::from(entries))
                        });
                        #access_activated.set_handler(move |entry| {
                            sp::Menu::activate(&*menu_item_tree, &entry.0);
                        });
                    }
                })
            } else if let [entries, Expression::PropertyReference(sub_menu), Expression::PropertyReference(activated)] =
                arguments
            {
                let entries = compile_expression(entries, ctx);
                let sub_menu = access_member(sub_menu, ctx).unwrap();
                let activated = access_member(activated, ctx).unwrap();
                let inner_component_id =
                    self::inner_component_id(ctx.current_sub_component().unwrap());
                quote! {
                    if sp::WindowInner::from_pub(#window_adapter_tokens.window()).supports_native_menu_bar() {
                        // May seem overkill to have an instance of the struct for each call, but there should only be one call per component anyway
                        struct MenuBarWrapper(sp::VWeakMapped<sp::ItemTreeVTable, #inner_component_id>);
                        const _ : () = {
                            use slint::private_unstable_api::re_exports::*;
                            MenuVTable_static!(static VT for MenuBarWrapper);
                        };
                        impl sp::Menu for MenuBarWrapper {
                            fn sub_menu(&self, parent: sp::Option<&sp::MenuEntry>, result: &mut sp::SharedVector<sp::MenuEntry>) {
                                let Some(self_rc) = self.0.upgrade() else { return };
                                let _self = self_rc.as_pin_ref();
                                let model = match parent {
                                    None => #entries,
                                    Some(parent) => #sub_menu.call(&(parent.clone(),))
                                };
                                *result = model.iter().map(|v| v.try_into().unwrap()).collect();
                            }
                            fn activate(&self, entry: &sp::MenuEntry) {
                                let Some(self_rc) = self.0.upgrade() else { return };
                                let _self = self_rc.as_pin_ref();
                                #activated.call(&(entry.clone(),))
                            }
                        }
                        sp::WindowInner::from_pub(#window_adapter_tokens.window()).setup_menubar(sp::VBox::new(MenuBarWrapper(_self.self_weak.get().unwrap().clone())));
                    }
                }
            } else {
                panic!("internal error: incorrect arguments to SetupNativeMenuBar")
            }
        }
        BuiltinFunction::MonthDayCount => {
            let (m, y) = (a.next().unwrap(), a.next().unwrap());
            quote!(sp::month_day_count(#m as u32, #y as i32).unwrap_or(0))
        }
        BuiltinFunction::MonthOffset => {
            let (m, y) = (a.next().unwrap(), a.next().unwrap());
            quote!(sp::month_offset(#m as u32, #y as i32))
        }
        BuiltinFunction::FormatDate => {
            let (f, d, m, y) =
                (a.next().unwrap(), a.next().unwrap(), a.next().unwrap(), a.next().unwrap());
            quote!(sp::format_date(&#f, #d as u32, #m as u32, #y as i32))
        }
        BuiltinFunction::ValidDate => {
            let (d, f) = (a.next().unwrap(), a.next().unwrap());
            quote!(sp::parse_date(#d.as_str(), #f.as_str()).is_some())
        }
        BuiltinFunction::ParseDate => {
            let (d, f) = (a.next().unwrap(), a.next().unwrap());
            quote!(sp::ModelRc::new(sp::parse_date(#d.as_str(), #f.as_str()).map(|d| sp::VecModel::from_slice(&d)).unwrap_or_default()))
        }
        BuiltinFunction::DateNow => {
            quote!(sp::ModelRc::new(sp::VecModel::from_slice(&sp::date_now())))
        }
        BuiltinFunction::TextInputFocused => {
            let window_adapter_tokens = access_window_adapter_field(ctx);
            quote!(sp::WindowInner::from_pub(#window_adapter_tokens.window()).text_input_focused())
        }
        BuiltinFunction::SetTextInputFocused => {
            let window_adapter_tokens = access_window_adapter_field(ctx);
            quote!(sp::WindowInner::from_pub(#window_adapter_tokens.window()).set_text_input_focused(#(#a)*))
        }
        BuiltinFunction::Translate => {
            quote!(slint::private_unstable_api::translate(#((#a) as _),*))
        }
        BuiltinFunction::Use24HourFormat => {
            quote!(slint::private_unstable_api::use_24_hour_format())
        }
        BuiltinFunction::ItemAbsolutePosition => {
            if let [Expression::PropertyReference(pr)] = arguments {
                let item_rc = access_item_rc(pr, ctx);
                quote!(
                    sp::logical_position_to_api((*#item_rc).map_to_window(::core::default::Default::default()))
                )
            } else {
                panic!("internal error: invalid args to MapPointToWindow {arguments:?}")
            }
        }
        BuiltinFunction::UpdateTimers => {
            quote!(_self.update_timers())
        }
    }
}

/// Return a TokenStream for a name (as in [`Type::Struct::name`])
fn struct_name_to_tokens(name: &str) -> TokenStream {
    // the name match the C++ signature so we need to change that to the rust namespace
    let mut name = name.replace("slint::private_api::", "sp::").replace('-', "_");
    if !name.contains("::") {
        name.insert_str(0, "r#")
    }
    name.parse().unwrap()
}

fn box_layout_function(
    cells_variable: &str,
    repeated_indices: Option<&str>,
    elements: &[Either<Expression, llr::RepeatedElementIdx>],
    orientation: Orientation,
    sub_expression: &Expression,
    ctx: &EvaluationContext,
) -> TokenStream {
    let repeated_indices = repeated_indices.map(ident);
    let inner_component_id = self::inner_component_id(ctx.current_sub_component().unwrap());
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
                let repeater_id = format_ident!("repeater{}", usize::from(*repeater));
                let rep_inner_component_id = self::inner_component_id(
                    &ctx.compilation_unit.sub_components
                        [ctx.current_sub_component().unwrap().repeated[*repeater].sub_tree.root],
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
                            || { #rep_inner_component_id::new(_self.self_weak.get().unwrap().clone()).unwrap().into() }
                        );
                        let internal_vec = _self.#repeater_id.instances_vec();
                        #ri
                        for sub_comp in &internal_vec {
                            items_vec.push(sub_comp.as_pin_ref().box_layout_data(#orientation))
                        }
                    ));
            }
        }
    }

    let ri = repeated_indices.as_ref().map(|ri| quote!(let mut #ri = [0u32; 2 * #repeater_idx];));
    let ri2 = repeated_indices.map(|ri| quote!(let #ri = sp::Slice::from_slice(&#ri);));
    let cells_variable = ident(cells_variable);
    let sub_expression = compile_expression(sub_expression, ctx);

    quote! { {
        #ri
        let mut items_vec = sp::Vec::with_capacity(#fixed_count #repeated_count);
        #(#push_code)*
        let #cells_variable = sp::Slice::from_slice(&items_vec);
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

fn generate_resources(doc: &Document) -> Vec<TokenStream> {
    #[cfg(feature = "software-renderer")]
    let link_section = std::env::var("SLINT_ASSET_SECTION")
        .ok()
        .map(|section| quote!(#[unsafe(link_section = #section)]));

    doc.embedded_file_resources
        .borrow()
        .iter()
        .map(|(path, er)| {
            let symbol = format_ident!("SLINT_EMBEDDED_RESOURCE_{}", er.id);
            match &er.kind {
                &crate::embedded_resources::EmbeddedResourcesKind::ListOnly => {
                    quote!()
                },
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
                        quote!(sp::Color::from_rgb_u8(#r, #g, #b))
                    } else {
                        quote!(sp::Color::from_argb_encoded(0))
                    };
                    let symbol_data = format_ident!("SLINT_EMBEDDED_RESOURCE_{}_DATA", er.id);
                    let data_size = data.len();
                    quote!(
                        #link_section
                        static #symbol_data : [u8; #data_size]= [#(#data),*];
                        #link_section
                        static #symbol: sp::StaticTextures = sp::StaticTextures{
                            size: sp::IntSize::new(#width as _, #height as _),
                            original_size: sp::IntSize::new(#unscaled_width as _, #unscaled_height as _),
                            data: sp::Slice::from_slice(&#symbol_data),
                            textures: sp::Slice::from_slice(&[
                                sp::StaticTexture {
                                    rect: sp::euclid::rect(#r_x as _, #r_y as _, #r_w as _, #r_h as _),
                                    format: #format,
                                    color: #color,
                                    index: 0,
                                }
                            ])
                        };
                    )
                },
                #[cfg(feature = "software-renderer")]
                crate::embedded_resources::EmbeddedResourcesKind::BitmapFontData(crate::embedded_resources::BitmapFont { family_name, character_map, units_per_em, ascent, descent, x_height, cap_height, glyphs, weight, italic, sdf }) => {

                    let character_map_size = character_map.len();

                    let character_map = character_map.iter().map(|crate::embedded_resources::CharacterMapEntry{code_point, glyph_index}| quote!(sp::CharacterMapEntry { code_point: #code_point, glyph_index: #glyph_index }));

                    let glyphs_size = glyphs.len();

                    let glyphs = glyphs.iter().map(|crate::embedded_resources::BitmapGlyphs{pixel_size, glyph_data}| {
                        let glyph_data_size = glyph_data.len();
                        let glyph_data = glyph_data.iter().map(|crate::embedded_resources::BitmapGlyph{x, y, width, height, x_advance, data}|{
                            let data_size = data.len();
                            quote!(
                                sp::BitmapGlyph {
                                    x: #x,
                                    y: #y,
                                    width: #width,
                                    height: #height,
                                    x_advance: #x_advance,
                                    data: sp::Slice::from_slice({
                                        #link_section
                                        static DATA : [u8; #data_size] = [#(#data),*];
                                        &DATA
                                    }),
                                }
                            )
                        });

                        quote!(
                            sp::BitmapGlyphs {
                                pixel_size: #pixel_size,
                                glyph_data: sp::Slice::from_slice({
                                    #link_section
                                    static GDATA : [sp::BitmapGlyph; #glyph_data_size] = [#(#glyph_data),*];
                                    &GDATA
                                }),
                            }
                        )
                    });

                    quote!(
                        #link_section
                        static #symbol: sp::BitmapFont = sp::BitmapFont {
                            family_name: sp::Slice::from_slice(#family_name.as_bytes()),
                            character_map: sp::Slice::from_slice({
                                #link_section
                                static CM : [sp::CharacterMapEntry; #character_map_size] = [#(#character_map),*];
                                &CM
                            }),
                            units_per_em: #units_per_em,
                            ascent: #ascent,
                            descent: #descent,
                            x_height: #x_height,
                            cap_height: #cap_height,
                            glyphs: sp::Slice::from_slice({
                                #link_section
                                static GLYPHS : [sp::BitmapGlyphs; #glyphs_size] = [#(#glyphs),*];
                                &GLYPHS
                            }),
                            weight: #weight,
                            italic: #italic,
                            sdf: #sdf,
                        };
                    )
                },
            }
        })
        .collect()
}

fn generate_named_exports(doc: &Document) -> Vec<TokenStream> {
    doc.exports
        .iter()
        .filter_map(|export| match &export.1 {
            Either::Left(component) if !component.is_global() => {
                Some((&export.0.name, &component.id))
            }
            Either::Right(ty) => match &ty {
                Type::Struct(s) if s.name.is_some() && s.node.is_some() => {
                    Some((&export.0.name, s.name.as_ref().unwrap()))
                }
                Type::Enumeration(en) => Some((&export.0.name, &en.name)),
                _ => None,
            },
            _ => None,
        })
        .filter(|(export_name, type_name)| export_name != type_name)
        .map(|(export_name, type_name)| {
            let type_id = ident(type_name);
            let export_id = ident(export_name);
            quote!(#type_id as #export_id)
        })
        .collect::<Vec<_>>()
}

#[cfg(feature = "bundle-translations")]
fn generate_translations(
    translations: &crate::translations::Translations,
    compilation_unit: &llr::CompilationUnit,
) -> TokenStream {
    let strings = translations.strings.iter().map(|strings| {
        let array = strings.iter().map(|s| match s.as_ref().map(SmolStr::as_str) {
            Some(s) => quote!(Some(#s)),
            None => quote!(None),
        });
        quote!(&[#(#array),*])
    });
    let plurals = translations.plurals.iter().map(|plurals| {
        let array = plurals.iter().map(|p| match p {
            Some(p) => {
                let p = p.iter().map(SmolStr::as_str);
                quote!(Some(&[#(#p),*]))
            }
            None => quote!(None),
        });
        quote!(&[#(#array),*])
    });

    let ctx = EvaluationContext {
        compilation_unit,
        current_sub_component: None,
        current_global: None,
        generator_state: RustGeneratorContext {
            global_access: quote!(compile_error!("language rule can't access state")),
        },
        parent: None,
        argument_types: &[Type::Int32],
    };
    let rules = translations.plural_rules.iter().map(|rule| {
        let rule = match rule {
            Some(rule) => {
                let rule = compile_expression(rule, &ctx);
                quote!(Some(|arg: i32| { let args = (arg,); (#rule) as usize } ))
            }
            None => quote!(None),
        };
        quote!(#rule)
    });
    let lang = translations.languages.iter().map(SmolStr::as_str).map(|lang| quote!(#lang));

    quote!(
        const _SLINT_TRANSLATED_STRINGS: &[&[sp::Option<&str>]] = &[#(#strings),*];
        const _SLINT_TRANSLATED_STRINGS_PLURALS: &[&[sp::Option<&[&str]>]] = &[#(#plurals),*];
        #[allow(unused)]
        const _SLINT_TRANSLATED_PLURAL_RULES: &[sp::Option<fn(i32) -> usize>] = &[#(#rules),*];
        const _SLINT_BUNDLED_LANGUAGES: &[&str] = &[#(#lang),*];
    )
}
