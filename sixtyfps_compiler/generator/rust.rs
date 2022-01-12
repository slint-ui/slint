// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

/*! module for the Rust code generator

Some convention used in the generated code:
 - `_self` is of type `Pin<&ComponentType>`  where ComponentType is the type of the generated sub component,
    this is existing for any evaluation of a binding
 - `self_rc` is of type `VRc<ComponentVTable, ComponentType>` or Rc<ComponentType> for globals
    this is usually a local variable to the init code that shouldn't rbe relied upon by the binding code.
*/

use crate::diagnostics::BuildDiagnostics;
use crate::expression_tree::{BuiltinFunction, EasingCurve, OperatorClass};
use crate::langtype::Type;
use crate::layout::Orientation;
use crate::llr::{self, EvaluationContext as _, Expression};
use crate::object_tree::Document;
use itertools::Either;
use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use std::collections::BTreeMap;
use std::num::NonZeroUsize;

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
            Orientation::Horizontal => quote!(sixtyfps::re_exports::Orientation::Horizontal),
            Orientation::Vertical => quote!(sixtyfps::re_exports::Orientation::Vertical),
        };
        tokens.extend(tks);
    }
}

impl quote::ToTokens for crate::embedded_resources::PixelFormat {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        use crate::embedded_resources::PixelFormat::*;
        let tks = match self {
            Rgb => quote!(sixtyfps::re_exports::PixelFormat::Rgb),
            Rgba => quote!(sixtyfps::re_exports::PixelFormat::Rgba),
            AlphaMap(_) => quote!(sixtyfps::re_exports::PixelFormat::AlphaMap),
        };
        tokens.extend(tks);
    }
}

fn rust_type(ty: &Type) -> Option<proc_macro2::TokenStream> {
    match ty {
        Type::Int32 => Some(quote!(i32)),
        Type::Float32 => Some(quote!(f32)),
        Type::String => Some(quote!(sixtyfps::re_exports::SharedString)),
        Type::Color => Some(quote!(sixtyfps::re_exports::Color)),
        Type::Duration => Some(quote!(i64)),
        Type::Angle => Some(quote!(f32)),
        Type::PhysicalLength => Some(quote!(f32)),
        Type::LogicalLength => Some(quote!(f32)),
        Type::Percent => Some(quote!(f32)),
        Type::Bool => Some(quote!(bool)),
        Type::Image => Some(quote!(sixtyfps::re_exports::Image)),
        Type::Struct { fields, name: None, .. } => {
            let elem = fields.values().map(rust_type).collect::<Option<Vec<_>>>()?;
            // This will produce a tuple
            Some(quote!((#(#elem,)*)))
        }
        Type::Struct { name: Some(name), .. } => Some(struct_name_to_tokens(name)),
        Type::Array(o) => {
            let inner = rust_type(o)?;
            Some(quote!(sixtyfps::re_exports::ModelHandle<#inner>))
        }
        Type::Enumeration(e) => {
            let e = ident(&e.name);
            Some(quote!(sixtyfps::re_exports::#e))
        }
        Type::Brush => Some(quote!(sixtyfps::Brush)),
        Type::LayoutCache => Some(quote!(SharedVector<f32>)),
        _ => None,
    }
}

/// Generate the rust code for the given component.
///
/// Fill the diagnostic in case of error.
pub fn generate(doc: &Document, _diag: &mut BuildDiagnostics) -> Option<TokenStream> {
    if matches!(doc.root_component.root_element.borrow().base_type, Type::Invalid | Type::Void) {
        // empty document, nothing to generate
        return None;
    }

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

    let llr = crate::llr::lower_to_item_tree::lower_to_item_tree(&doc.root_component);

    let sub_compos = llr
        .sub_components
        .values()
        .map(|sub_compo| generate_sub_component(&sub_compo, &llr, None, quote!()))
        .collect::<Option<Vec<_>>>()?;

    let compo = generate_public_component(&llr)?;
    let compo_id = public_component_id(&llr.item_tree.root);
    let compo_module = format_ident!("sixtyfps_generated_{}", compo_id);
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

    let resource_symbols = doc.root_component
        .embedded_file_resources
        .borrow()
        .iter()
        .map(|(path, er)| {
            let symbol = format_ident!("SFPS_EMBEDDED_RESOURCE_{}", er.id);
            match &er.kind {
                crate::embedded_resources::EmbeddedResourcesKind::RawData => {
                    let data = embedded_file_tokens(path);
                    quote!(const #symbol: &'static [u8] = #data;)
                }
                crate::embedded_resources::EmbeddedResourcesKind::TextureData(crate::embedded_resources::Texture { data, format, rect, total_size: crate::embedded_resources::Size{width, height} }) => {
                    let (r_x, r_y, r_w, r_h) = (rect.x(), rect.y(), rect.width(), rect.height());
                    let color = if let crate::embedded_resources::PixelFormat::AlphaMap([r, g, b]) = format {
                        quote!(sixtyfps::re_exports::Color::from_rgb_u8(#r, #g, #b))
                    } else {
                        quote!(sixtyfps::re_exports::Color::from_argb_encoded(0))
                    };
                    quote!(
                        const #symbol: sixtyfps::re_exports::ImageInner = sixtyfps::re_exports::ImageInner::StaticTextures {
                            size: sixtyfps::re_exports::IntSize::new(#width as _, #height as _),
                            data: Slice::from_slice(&[#(#data),*]),
                            textures: Slice::from_slice(&[
                                sixtyfps::re_exports::StaticTexture {
                                    rect: sixtyfps::re_exports::euclid::rect(#r_x as _, #r_y as _, #r_w as _, #r_h as _),
                                    format: #format,
                                    color: #color,
                                    index: 0,
                                }
                            ])
                        };
                    )
                },
            }
        }).collect::<Vec<_>>();

    Some(quote! {
        #[allow(non_snake_case)]
        #[allow(non_camel_case_types)]
        #[allow(clippy::style)]
        #[allow(clippy::complexity)]
        #[allow(unused_braces)]
        mod #compo_module {
            use sixtyfps::re_exports::*;
            #(#structs)*
            #(#globals)*
            #(#sub_compos)*
            #compo
            #(#resource_symbols)*
            const _THE_SAME_VERSION_MUST_BE_USED_FOR_THE_COMPILER_AND_THE_RUNTIME : sixtyfps::#version_check = sixtyfps::#version_check;
        }
        pub use #compo_module::{#compo_id #(,#structs_ids)* #(,#globals_ids)* };
        pub use sixtyfps::{ComponentHandle, Global};
    })
}

fn generate_public_component(llr: &llr::PublicComponent) -> Option<TokenStream> {
    let public_component_id = public_component_id(&llr.item_tree.root);
    let inner_component_id = inner_component_id(&llr.item_tree.root);
    let global_container_id = format_ident!("Globals_{}", public_component_id);

    let component =
        generate_item_tree(&llr.item_tree, llr, None, quote!(globals: #global_container_id))?;

    let ctx = EvaluationContext {
        public_component: llr,
        current_sub_component: Some(&llr.item_tree.root),
        current_global: None,
        root_access: quote!(_self),
        parent: None,
        argument_types: &[],
    };

    let property_and_callback_accessors =
        public_api(&llr.public_properties, quote!(vtable::VRc::as_pin_ref(&self.0)), &ctx);

    let global_names =
        llr.globals.iter().map(|g| format_ident!("global_{}", ident(&g.name))).collect::<Vec<_>>();
    let global_types = llr.globals.iter().map(|g| global_inner_name(g)).collect::<Vec<_>>();

    Some(quote!(
        #component
        pub struct #public_component_id(vtable::VRc<sixtyfps::re_exports::ComponentVTable, #inner_component_id>);

        impl #public_component_id {
            pub fn new() -> Self {
                Self(#inner_component_id::new())
            }

            #property_and_callback_accessors
        }

        impl From<#public_component_id> for vtable::VRc<sixtyfps::re_exports::ComponentVTable, #inner_component_id> {
            fn from(value: #public_component_id) -> Self {
                value.0
            }
        }

        impl sixtyfps::ComponentHandle for #public_component_id {
            type Inner = #inner_component_id;
            fn as_weak(&self) -> sixtyfps::Weak<Self> {
                sixtyfps::Weak::new(&self.0)
            }

            fn clone_strong(&self) -> Self {
                Self(self.0.clone())
            }

            fn from_inner(inner: vtable::VRc<sixtyfps::re_exports::ComponentVTable, #inner_component_id>) -> Self {
                Self(inner)
            }

            fn run(&self) {
                self.show();
                sixtyfps::run_event_loop();
                self.hide();
            }

            fn show(&self) {
                self.window().show();
            }

            fn hide(&self) {
                self.window().hide()
            }

            fn window(&self) -> &sixtyfps::Window {
                vtable::VRc::as_pin_ref(&self.0).get_ref().window.get().unwrap()
            }

            fn global<'a, T: sixtyfps::Global<'a, Self>>(&'a self) -> T {
                T::get(&self)
            }
        }

        struct #global_container_id {
            #(#global_names : ::core::pin::Pin<sixtyfps::re_exports::Rc<#global_types>>,)*
        }
        impl Default for #global_container_id {
            fn default() -> Self {
                Self {
                    #(#global_names : #global_types::new(),)*
                }
            }
        }
    ))
}

fn generate_struct(name: &str, fields: &BTreeMap<String, Type>) -> TokenStream {
    let component_id = struct_name_to_tokens(name);
    let (declared_property_vars, declared_property_types): (Vec<_>, Vec<_>) =
        fields.iter().map(|(name, ty)| (ident(name), rust_type(ty).unwrap())).unzip();

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

    if let Type::Callback { args, .. } = &prop_type {
        let mut ctx2 = ctx.clone();
        ctx2.argument_types = &args;
        let tokens_for_expression = compile_expression(&binding_expression.expression, &ctx2);
        init.push(quote!({
            #[allow(unreachable_code, unused)]
            sixtyfps::internal::set_callback_handler(#rust_property, &self_rc, {
                move |self_rc, args| {
                    #init_self_pin_ref
                    (#tokens_for_expression) as _
                }
            });
        }));
    } else {
        let tokens_for_expression = compile_expression(&binding_expression.expression, ctx);
        init.push(if binding_expression.is_constant {
            let t = rust_type(&prop_type).unwrap_or(quote!(_));

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
            let binding_tokens = quote!(move |self_rc| {
                #init_self_pin_ref
                (#tokens_for_expression) as _
            });

            let is_state_info = matches!(prop_type, Type::Struct { name: Some(name), .. } if name.ends_with("::StateInfo"));
            if is_state_info {
                quote! { {
                    sixtyfps::internal::set_property_state_binding(#rust_property, &self_rc, #binding_tokens);
                } }
            } else {
                match &binding_expression.animation {
                    Some(llr::Animation::Static(anim)) => {
                        let anim = compile_expression(anim, ctx);
                        quote! { {
                            #init_self_pin_ref
                            sixtyfps::internal::set_animated_property_binding(#rust_property, &self_rc, #binding_tokens, #anim);
                        } }
                    }
                    Some(llr::Animation::Transition(anim)) => {
                        let anim = compile_expression(anim, ctx);
                        quote! {
                            sixtyfps::internal::set_animated_property_binding_for_transition(
                                #rust_property, &self_rc, #binding_tokens, move |self_rc| {
                                    #init_self_pin_ref
                                    #anim
                                }
                            );
                        }
                    }
                    None => {
                        quote! { {
                            sixtyfps::internal::set_property_binding(#rust_property, &self_rc, #binding_tokens);
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
    self_init: TokenStream,
    ctx: &EvaluationContext,
) -> TokenStream {
    let mut property_and_callback_accessors: Vec<TokenStream> = vec![];
    for (p, (ty, r)) in public_properties {
        let prop_ident = ident(&p);
        let prop = access_member(r, &ctx);

        if let Type::Callback { args, return_type } = ty {
            let callback_args = args.iter().map(|a| rust_type(a).unwrap()).collect::<Vec<_>>();
            let return_type = return_type.as_ref().map_or(quote!(()), |a| rust_type(a).unwrap());
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
        } else {
            let rust_property_type = rust_type(ty).unwrap();

            let getter_ident = format_ident!("get_{}", prop_ident);
            let setter_ident = format_ident!("set_{}", prop_ident);

            property_and_callback_accessors.push(quote!(
                #[allow(dead_code)]
                pub fn #getter_ident(&self) -> #rust_property_type {
                    #[allow(unused_imports)]
                    use sixtyfps::re_exports::*;
                    let _self = #self_init;
                    #prop.get()
                }
            ));

            let set_value = property_set_value_tokens(r, quote!(value), ctx);
            property_and_callback_accessors.push(quote!(
                #[allow(dead_code)]
                pub fn #setter_ident(&self, value: #rust_property_type) {
                    #[allow(unused_imports)]
                    use sixtyfps::re_exports::*;
                    let _self = #self_init;
                    #set_value
                }
            ));
        }
    }

    quote!(#(#property_and_callback_accessors)*)
}

/// Generate the rust code for the given component.
///
/// Fill the diagnostic in case of error.
fn generate_sub_component(
    component: &llr::SubComponent,
    root: &llr::PublicComponent,
    parent_ctx: Option<ParentCtx>,
    extra_fields: TokenStream,
) -> Option<TokenStream> {
    let inner_component_id = inner_component_id(component);

    let ctx = EvaluationContext::new_sub_component(root, component, parent_ctx);
    let mut extra_components = component
        .popup_windows
        .iter()
        .map(|c| generate_item_tree(c, root, Some(ParentCtx::new(&ctx, None)), quote!()))
        .collect::<Vec<_>>();

    let mut declared_property_vars = vec![];
    let mut declared_property_types = vec![];
    let mut declared_callbacks = vec![];
    let mut declared_callbacks_types = vec![];
    let mut declared_callbacks_ret = vec![];

    for property in &component.properties {
        let prop_ident = ident(&property.name);
        if let Type::Callback { args, return_type } = &property.ty {
            let callback_args = args.iter().map(|a| rust_type(a).unwrap()).collect::<Vec<_>>();
            let return_type = return_type.as_ref().map_or(quote!(()), |a| rust_type(a).unwrap());
            declared_callbacks.push(prop_ident.clone());
            declared_callbacks_types.push(callback_args);
            declared_callbacks_ret.push(return_type);
        } else {
            let rust_property_type = rust_type(&property.ty).unwrap();
            declared_property_vars.push(prop_ident.clone());
            declared_property_types.push(rust_property_type.clone());
        }
    }

    let mut init = vec![];
    let mut item_names = vec![];
    let mut item_types = vec![];

    for item in &component.items {
        if item.is_flickable_viewport {
            continue;
        }
        item_names.push(ident(&item.name));
        item_types.push(ident(&item.ty.class_name));
        #[cfg(sixtyfps_debug_property)]
        for (prop, info) in &item.ty.properties {
            if info.ty.is_property_type() && !prop.starts_with("viewport") && prop != "commands" {
                let name = format!("{}::{}.{}", component.name, item.name, prop);
                let elem_name = ident(&item.id);
                let prop = ident(&prop);
                init.push(quote!(self_rc.#elem_name.#prop.debug_name.replace(#name.into());));
            }
        }
    }

    let mut repeated_element_names: Vec<Ident> = vec![];
    let mut repeated_visit_branch: Vec<TokenStream> = vec![];
    let mut repeated_element_components: Vec<Ident> = vec![];

    for (idx, repeated) in component.repeated.iter().enumerate() {
        extra_components.push(generate_repeated_component(
            &repeated,
            root,
            ParentCtx::new(&ctx, Some(idx)),
        ));
        let repeater_id = format_ident!("repeater{}", idx);
        let rep_inner_component_id = self::inner_component_id(&repeated.sub_tree.root);

        let mut model = compile_expression(&repeated.model, &ctx);
        if repeated.model.ty(&ctx) == Type::Bool {
            model = quote!(sixtyfps::re_exports::ModelHandle::new(sixtyfps::re_exports::Rc::<bool>::new(#model)))
        }

        init.push(quote! {
            _self.#repeater_id.set_model_binding({
                let self_weak = sixtyfps::re_exports::VRcMapped::downgrade(&self_rc);
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
        repeated_element_names.push(repeater_id);
        repeated_element_components.push(rep_inner_component_id);
    }

    let mut sub_component_names: Vec<Ident> = vec![];
    let mut sub_component_types: Vec<Ident> = vec![];

    for sub in &component.sub_components {
        let field_name = ident(&sub.name);
        let sub_component_id = self::inner_component_id(&sub.ty);
        let tree_index: u32 = sub.index_in_tree as _;
        //let tree_index_of_first_child: u32 = children_offset as _;
        let root_ref_tokens = &ctx.root_access;

        // For children of sub-components, the item index generated by the generate_item_indices pass
        // starts at 1 (0 is the root element).
        let item_index_base_tokens = if tree_index == 0 {
            quote!(tree_index_of_first_child - 1 + #tree_index)
        } else {
            quote!(tree_index)
        };

        init.push(quote!(#sub_component_id::init(
            VRcMapped::map(self_rc.clone(), |x| Self::FIELD_OFFSETS.#field_name.apply_pin(x)),
            &#root_ref_tokens,
            #tree_index,
            /*TODO: tree_index_of_first_child + #tree_item_total*/ 0
        );));

        let sub_component_repeater_count = sub.ty.repeater_count();
        if sub_component_repeater_count > 0 {
            let repeater_offset = sub.repeater_offset;
            let last_repeater: usize = repeater_offset + sub_component_repeater_count - 1;
            repeated_visit_branch.push(quote!(
                #repeater_offset..=#last_repeater => {
                    Self::FIELD_OFFSETS.#field_name.apply_pin(_self).visit_dynamic_children(dyn_index - #repeater_offset, order, visitor)
                }
            ));
        }

        sub_component_names.push(field_name);
        sub_component_types.push(sub_component_id);
    }

    #[cfg(sixtyfps_debug_property)]
    builder.init.push(quote!(
        #(self_rc.#declared_property_vars.debug_name.replace(
            concat!(stringify!(#inner_component_id), ".", stringify!(#declared_property_vars)).into());)*
    ));

    for (prop1, prop2) in &component.two_way_bindings {
        let p1 = access_member(prop1, &ctx);
        let p2 = access_member(prop2, &ctx);
        init.push(quote!(
            Property::link_two_way(#p1, #p2);
        ));
    }

    for (prop, expression) in &component.property_init {
        handle_property_init(prop, expression, &mut init, &ctx)
    }
    for prop in &component.const_properties {
        let rust_property = access_member(prop, &ctx);
        init.push(quote!(#rust_property.set_constant();))
    }

    let root_component_id = self::inner_component_id(&root.item_tree.root);

    let parent_component_type = parent_ctx.iter().map(|parent| {
        let parent_component_id = self::inner_component_id(&parent.ctx.current_sub_component.unwrap());
        quote!(sixtyfps::re_exports::VWeakMapped::<sixtyfps::re_exports::ComponentVTable, #parent_component_id>)
    });

    let layout_info_h = compile_expression(&component.layout_info_h, &ctx);
    let layout_info_v = compile_expression(&component.layout_info_v, &ctx);

    // FIXME! this is only public because of the ComponentHandle::Inner. we should find another way
    let visibility =
        core::ptr::eq(&root.item_tree.root as *const _, component as *const _).then(|| quote!(pub));

    Some(quote!(
        #[derive(sixtyfps::re_exports::FieldOffsets, Default)]
        #[const_field_offset(sixtyfps::re_exports::const_field_offset)]
        #[repr(C)]
        #[pin]
        #visibility
        struct #inner_component_id {
            #(#item_names : sixtyfps::re_exports::#item_types,)*
            #(#sub_component_names : #sub_component_types,)*
            #(#declared_property_vars : sixtyfps::re_exports::Property<#declared_property_types>,)*
            #(#declared_callbacks : sixtyfps::re_exports::Callback<(#(#declared_callbacks_types,)*), #declared_callbacks_ret>,)*
            #(#repeated_element_names : sixtyfps::re_exports::Repeater<#repeated_element_components>,)*
            self_weak : sixtyfps::re_exports::OnceCell<sixtyfps::re_exports::VWeakMapped<sixtyfps::re_exports::ComponentVTable, #inner_component_id>>,
            #(parent : #parent_component_type,)*
            // FIXME: Do we really need a window all the time?
            window: sixtyfps::re_exports::OnceCell<sixtyfps::Window>,
            root : sixtyfps::re_exports::OnceCell<sixtyfps::re_exports::VWeak<sixtyfps::re_exports::ComponentVTable, #root_component_id>>,
            tree_index: ::core::cell::Cell<u32>,
            tree_index_of_first_child: ::core::cell::Cell<u32>,
            #extra_fields
        }

        impl #inner_component_id {
            pub fn init(self_rc: sixtyfps::re_exports::VRcMapped<sixtyfps::re_exports::ComponentVTable, Self>,
                    root : &sixtyfps::re_exports::VRc<sixtyfps::re_exports::ComponentVTable, #root_component_id>,
                    tree_index: u32, tree_index_of_first_child: u32) {
                #![allow(unused)]
                let _self = self_rc.as_pin_ref();
                _self.self_weak.set(VRcMapped::downgrade(&self_rc));
                _self.root.set(VRc::downgrade(root));
                _self.window.set(root.window.get().unwrap().window_handle().clone().into());
                _self.tree_index.set(tree_index);
                _self.tree_index_of_first_child.set(tree_index_of_first_child);
                #(#init)*
            }

            fn visit_dynamic_children(
                self: ::core::pin::Pin<&Self>,
                dyn_index: usize,
                order: sixtyfps::re_exports::TraversalOrder,
                visitor: sixtyfps::re_exports::ItemVisitorRefMut
            ) -> sixtyfps::re_exports::VisitChildrenResult {
                #![allow(unused)]
                use sixtyfps::re_exports::*;
                let _self = self;
                match dyn_index {
                    #(#repeated_visit_branch)*
                    _ => panic!("invalid dyn_index {}", dyn_index),
                }
            }

            fn layout_info(self: ::core::pin::Pin<&Self>, orientation: sixtyfps::re_exports::Orientation) -> sixtyfps::re_exports::LayoutInfo {
                #![allow(unused)]
                use sixtyfps::re_exports::*;
                let _self = self;
                match orientation {
                    sixtyfps::re_exports::Orientation::Horizontal => #layout_info_h,
                    sixtyfps::re_exports::Orientation::Vertical => #layout_info_v,
                }
            }
        }

        #(#extra_components)*
    ))
}

fn generate_global(global: &llr::GlobalComponent, root: &llr::PublicComponent) -> TokenStream {
    let mut declared_property_vars = vec![];
    let mut declared_property_types = vec![];
    let mut declared_callbacks = vec![];
    let mut declared_callbacks_types = vec![];
    let mut declared_callbacks_ret = vec![];

    for property in &global.properties {
        let prop_ident = ident(&property.name);
        if let Type::Callback { args, return_type } = &property.ty {
            let callback_args = args.iter().map(|a| rust_type(a).unwrap()).collect::<Vec<_>>();
            let return_type = return_type.as_ref().map_or(quote!(()), |a| rust_type(a).unwrap());
            declared_callbacks.push(prop_ident.clone());
            declared_callbacks_types.push(callback_args);
            declared_callbacks_ret.push(return_type);
        } else {
            let rust_property_type = rust_type(&property.ty).unwrap();
            declared_property_vars.push(prop_ident.clone());
            declared_property_types.push(rust_property_type.clone());
        }
    }

    let mut init = vec![];

    let ctx = EvaluationContext {
        public_component: root,
        current_sub_component: None,
        current_global: Some(global),
        root_access: quote!(compilation_error("can't access root from global")),
        parent: None,
        argument_types: &[],
    };

    for (property_index, expression) in global.init_values.iter().enumerate() {
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
        if *cst {
            let rust_property = access_member(
                &llr::PropertyReference::Local { sub_component_path: vec![], property_index },
                &ctx,
            );
            init.push(quote!(#rust_property.set_constant();))
        }
    }

    let inner_component_id = format_ident!("Inner{}", ident(&global.name));

    let public_interface = global.exported.then(|| {
        let property_and_callback_accessors = public_api(&global.public_properties, quote!(self.0.as_ref()), &ctx);
        let public_component_id = ident(&global.name);
        let root_component_id = self::public_component_id(&root.item_tree.root);
        let global_id = format_ident!("global_{}", public_component_id);

        let aliases = global.aliases.iter().map(|name| ident(&name));
        quote!(
            pub struct #public_component_id<'a>(&'a ::core::pin::Pin<sixtyfps::re_exports::Rc<#inner_component_id>>);

            impl<'a> #public_component_id<'a> {
                #property_and_callback_accessors
            }

            #(pub type #aliases<'a> = #public_component_id<'a>;)*

            impl<'a> sixtyfps::Global<'a, #root_component_id> for #public_component_id<'a> {
                fn get(component: &'a #root_component_id) -> Self {
                    Self(&component.0 .globals.#global_id)
                }
            }
        )
    });

    quote!(
        #[derive(sixtyfps::re_exports::FieldOffsets, Default)]
        #[const_field_offset(sixtyfps::re_exports::const_field_offset)]
        #[repr(C)]
        #[pin]
        struct #inner_component_id {
            #(#declared_property_vars: sixtyfps::re_exports::Property<#declared_property_types>,)*
            #(#declared_callbacks: sixtyfps::re_exports::Callback<(#(#declared_callbacks_types,)*), #declared_callbacks_ret>,)*
        }

        impl #inner_component_id {
            fn new() -> ::core::pin::Pin<sixtyfps::re_exports::Rc<Self>> {
                let self_rc = sixtyfps::re_exports::Rc::pin(Self::default());
                let _self = self_rc.as_ref();
                #(#init)*
                self_rc
            }
        }

        #public_interface
    )
}

fn generate_item_tree(
    sub_tree: &llr::ItemTree,
    root: &llr::PublicComponent,
    parent_ctx: Option<ParentCtx>,
    extra_fields: TokenStream,
) -> Option<TokenStream> {
    let sub_comp = generate_sub_component(&sub_tree.root, root, parent_ctx, extra_fields)?;
    let inner_component_id = self::inner_component_id(&sub_tree.root);
    let parent_component_type = parent_ctx.iter().map(|parent| {
        let parent_component_id = self::inner_component_id(&parent.ctx.current_sub_component.unwrap());
        quote!(sixtyfps::re_exports::VWeakMapped::<sixtyfps::re_exports::ComponentVTable, #parent_component_id>)
    }).collect::<Vec<_>>();
    let root_token = if parent_ctx.is_some() {
        quote!(&parent.upgrade().unwrap().root.get().unwrap().upgrade().unwrap())
    } else {
        quote!(&self_rc)
    };
    let maybe_create_window = parent_ctx.is_none().then(|| {
        quote!(
            _self.window.set(sixtyfps::create_window().into());
            _self.window.get().unwrap().window_handle().set_component(&VRc::into_dyn(self_rc.clone()));
        )
    });

    let parent_item_index = parent_ctx.and_then(|parent| {
        parent
            .repeater_index
            .map(|idx| parent.ctx.current_sub_component.unwrap().repeated[idx].index_in_tree)
    });
    let parent_item_index = parent_item_index.iter();
    let mut item_tree_array = vec![];
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
                sixtyfps::re_exports::ItemTreeNode::DynamicTree {
                    index: #repeater_index,
                    parent_index: #parent_index,
                }
            ));
        } else {
            let item = &component.items[node.item_index];
            let flick = item
                .is_flickable_viewport
                .then(|| quote!(+ sixtyfps::re_exports::Flickable::FIELD_OFFSETS.viewport));

            let field = access_component_field_offset(
                &self::inner_component_id(component),
                &ident(&item.name),
            );

            let children_count = node.children.len() as u32;
            let children_index = children_offset as u32;
            item_tree_array.push(quote!(
                sixtyfps::re_exports::ItemTreeNode::Item{
                    item: VOffset::new(#path #field #flick),
                    children_count: #children_count,
                    children_index: #children_index,
                    parent_index: #parent_index,
                }
            ))
        }
    });

    let item_tree_array_len = item_tree_array.len();

    Some(quote!(
        #sub_comp

        impl #inner_component_id {
            pub fn new(#(parent: #parent_component_type)*)
                -> vtable::VRc<sixtyfps::re_exports::ComponentVTable, Self>
            {
                #![allow(unused)]
                use sixtyfps::re_exports::*;
                let mut _self = Self::default();
                #(_self.parent = parent.clone() as #parent_component_type;)*
                let self_rc = VRc::new(_self);
                let _self = self_rc.as_pin_ref();
                #maybe_create_window;
                sixtyfps::re_exports::init_component_items(_self, Self::item_tree(), #root_token.window.get().unwrap().window_handle());
                Self::init(sixtyfps::re_exports::VRc::map(self_rc.clone(), |x| x), #root_token, 0 , 1);
                self_rc
            }

            fn item_tree() -> &'static [sixtyfps::re_exports::ItemTreeNode<Self>] {
                use sixtyfps::re_exports::*;
                ComponentVTable_static!(static VT for #inner_component_id);
                // FIXME: ideally this should be a const, but we can't because of the pointer to the vtable
                static ITEM_TREE : sixtyfps::re_exports::OnceBox<
                    [sixtyfps::re_exports::ItemTreeNode<#inner_component_id>; #item_tree_array_len]
                > = sixtyfps::re_exports::OnceBox::new();
                &*ITEM_TREE.get_or_init(|| Box::new([#(#item_tree_array),*]))
            }
        }

        impl sixtyfps::re_exports::PinnedDrop for #inner_component_id {
            fn drop(self: core::pin::Pin<&mut #inner_component_id>) {
                sixtyfps::re_exports::free_component_item_graphics_resources(self.as_ref(), Self::item_tree(), self.window.get().unwrap().window_handle());
            }
        }

        impl sixtyfps::re_exports::WindowHandleAccess for #inner_component_id {
            fn window_handle(&self) -> &sixtyfps::re_exports::Rc<sixtyfps::re_exports::Window> {
                self.window.get().unwrap().window_handle()
            }
        }

        impl sixtyfps::re_exports::Component for #inner_component_id {
            fn visit_children_item(self: ::core::pin::Pin<&Self>, index: isize, order: sixtyfps::re_exports::TraversalOrder, visitor: sixtyfps::re_exports::ItemVisitorRefMut)
                -> sixtyfps::re_exports::VisitChildrenResult
            {
                use sixtyfps::re_exports::*;
                return sixtyfps::re_exports::visit_item_tree(self, &VRcMapped::origin(&self.as_ref().self_weak.get().unwrap().upgrade().unwrap()), Self::item_tree(), index, order, visitor, visit_dynamic);
                #[allow(unused)]
                fn visit_dynamic(_self: ::core::pin::Pin<&#inner_component_id>, order: sixtyfps::re_exports::TraversalOrder, visitor: ItemVisitorRefMut, dyn_index: usize) -> VisitChildrenResult  {
                    _self.visit_dynamic_children(dyn_index, order, visitor)
                }
            }

            fn get_item_ref(self: ::core::pin::Pin<&Self>, index: usize) -> ::core::pin::Pin<ItemRef> {
                match &Self::item_tree()[index] {
                    ItemTreeNode::Item { item, .. } => item.apply_pin(self),
                    ItemTreeNode::DynamicTree { .. } => panic!("get_item_ref called on dynamic tree"),

                }
            }

            fn parent_item(self: ::core::pin::Pin<&Self>, index: usize, result: &mut sixtyfps::re_exports::ItemWeak) {
                if index == 0 {
                    #(
                        if let Some(parent) = self.parent.clone().upgrade().map(|sc| VRcMapped::origin(&sc)) {
                            *result = sixtyfps::re_exports::ItemRc::new(parent, #parent_item_index).parent_item();
                        }
                    )*
                    return;
                }
                let parent_index = match &Self::item_tree()[index] {
                    ItemTreeNode::Item { parent_index, .. } => *parent_index,
                    ItemTreeNode::DynamicTree { parent_index, .. } => *parent_index,
                };
                let self_rc = sixtyfps::re_exports::VRcMapped::origin(&self.self_weak.get().unwrap().upgrade().unwrap());
                *result = ItemRc::new(self_rc, parent_index as _).downgrade();
            }

            fn layout_info(self: ::core::pin::Pin<&Self>, orientation: sixtyfps::re_exports::Orientation) -> sixtyfps::re_exports::LayoutInfo {
                self.layout_info(orientation)
            }
        }


    ))
}

fn generate_repeated_component(
    repeated: &llr::RepeatedElement,
    root: &llr::PublicComponent,
    parent_ctx: ParentCtx,
) -> Option<TokenStream> {
    let component = generate_item_tree(&repeated.sub_tree, root, Some(parent_ctx), quote!())?;

    let ctx = EvaluationContext {
        public_component: root,
        current_sub_component: Some(&repeated.sub_tree.root),
        current_global: None,
        root_access: quote!(_self),
        parent: Some(parent_ctx),
        argument_types: &[],
    };

    let inner_component_id = self::inner_component_id(&repeated.sub_tree.root);

    // let rep_inner_component_id = self::inner_component_id(&repeated.sub_tree.root.name);
    //  let inner_component_id = self::inner_component_id(&parent_compo);

    let extra_fn = if let Some(listview) = &repeated.listview {
        let p_y = access_member(&listview.prop_y, &ctx);
        let p_height = access_member(&listview.prop_height, &ctx);
        let p_width = access_member(&listview.prop_width, &ctx);
        quote! {
            fn listview_layout(
                self: core::pin::Pin<&Self>,
                offset_y: &mut f32,
                viewport_width: core::pin::Pin<&sixtyfps::re_exports::Property<f32>>,
            ) {
                use sixtyfps::re_exports::*;
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
            fn box_layout_data(self: ::core::pin::Pin<&Self>, o: sixtyfps::re_exports::Orientation)
                -> sixtyfps::re_exports::BoxLayoutCellData
            {
                use sixtyfps::re_exports::*;
                BoxLayoutCellData { constraint: self.as_ref().layout_info(o) }
            }
        }
    };

    let data_type = if let Some(data_prop) = repeated.data_prop {
        rust_type(&repeated.sub_tree.root.properties[data_prop].ty).unwrap()
    } else {
        quote!(())
    };

    let access_prop = |&property_index| {
        access_member(
            &llr::PropertyReference::Local { sub_component_path: vec![], property_index },
            &ctx,
        )
    };
    let index_prop = repeated.index_prop.iter().map(access_prop);
    let data_prop = repeated.data_prop.iter().map(access_prop);

    Some(quote!(
        #component

        impl sixtyfps::re_exports::RepeatedComponent for #inner_component_id {
            type Data = #data_type;
            fn update(&self, _index: usize, _data: Self::Data) {
                let self_rc = self.self_weak.get().unwrap().upgrade().unwrap();
                let _self = self_rc.as_pin_ref();
                #(#index_prop.set(_index as _);)*
                #(#data_prop.set(_data);)*
            }
            #extra_fn
        }
    ))
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
    // FIXME! animation
    /*if let Some(binding) = element.borrow().bindings.get(property_name) {
        if let Some(crate::object_tree::PropertyAnimation::Static(animation)) =
        binding.borrow().animation.as_ref()
        {
            let animation_tokens = property_animation_tokens(component, animation);
            return quote!(set_animated_value(#value_tokens, #animation_tokens));
        }
    }*/
    let prop = access_member(property, ctx);
    quote!(#prop.set(#value_tokens as _))
}

/// Returns the code that can access the given property or callback (but without the set or get)
///
/// to be used like:
/// ```ignore
/// let access = access_member(...)
/// quote!(#access.get())
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
        if prop_name.is_empty() {
            // then this is actually a reference to the element itself
            quote!((#compo_path #component_id::FIELD_OFFSETS.#item_name).apply_pin(_self))
        } else {
            let property_name = ident(&prop_name);
            let item_ty = ident(&sub_component.items[item_index].ty.class_name);
            let flick = sub_component.items[item_index]
                .is_flickable_viewport
                .then(|| quote!(+ sixtyfps::re_exports::Flickable::FIELD_OFFSETS.viewport));
            quote!((#compo_path #component_id::FIELD_OFFSETS.#item_name #flick + #item_ty::FIELD_OFFSETS.#property_name).apply_pin(#path))
        }
    }

    match reference {
        llr::PropertyReference::Local { sub_component_path, property_index } => {
            if let Some(sub_component) = ctx.current_sub_component {
                let (compo_path, sub_component) =
                    follow_sub_component_path(sub_component, sub_component_path);
                let component_id = inner_component_id(sub_component);
                let property_name = ident(&sub_component.properties[*property_index].name);
                quote!((#compo_path #component_id::FIELD_OFFSETS.#property_name).apply_pin(_self))
            } else if let Some(current_global) = ctx.current_global {
                let global_name = global_inner_name(&current_global);
                let property_name = ident(&current_global.properties[*property_index].name);
                quote!(#global_name::FIELD_OFFSETS.#property_name.apply_pin(_self))
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
                ctx = ctx.parent.unwrap().ctx;
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
                llr::PropertyReference::InParent { .. } | llr::PropertyReference::Global { .. } => {
                    unreachable!()
                }
            }
        }
        llr::PropertyReference::Global { global_index, property_index } => {
            let root_access = &ctx.root_access;
            let global = &ctx.public_component.globals[*global_index];
            let global_id = format_ident!("global_{}", ident(&global.name));
            let global_name = global_inner_name(global);
            let property_name = ident(
                &ctx.public_component.globals[*global_index].properties[*property_index].name,
            );
            quote!(#global_name::FIELD_OFFSETS.#property_name.apply_pin(#root_access.globals.#global_id.as_ref()))
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

fn access_window_field(ctx: &EvaluationContext) -> TokenStream {
    let root = &ctx.root_access;
    quote!(#root.window.get().unwrap().window_handle())
}

/*
/// Returns the code that creates a VRc<ComponentVTable, Dyn> for the component of the given element
fn element_component_vrc(element: &ElementRc, component: &Rc<Component>) -> TokenStream {
    let enclosing_component = element.borrow().enclosing_component.upgrade().unwrap();

    let mut access_component = quote!(_self);

    let mut component = component.clone();
    while !Rc::ptr_eq(&component, &enclosing_component) {
        access_component = quote!(#access_component.parent.upgrade().unwrap().as_pin_ref());
        component = component
            .parent_element
            .upgrade()
            .unwrap()
            .borrow()
            .enclosing_component
            .upgrade()
            .unwrap();
    }

    if component.is_sub_component() {
        quote!(VRcMapped::origin(&#access_component.self_weak.get().unwrap().upgrade().unwrap()))
    } else {
        quote!(VRc::into_dyn(#access_component.self_weak.get().unwrap().upgrade().unwrap()))
    }
}

// Returns an expression that will compute the absolute item index in the item tree for a
// given element. For elements of a child component or the root component, the item_index
// is already absolute within the corresponding item tree. For sub-components we return an
// expression that computes the value at run-time.
fn absolute_element_item_index_expression(element: &ElementRc) -> TokenStream {
    let element = element.borrow();
    let local_index = element.item_index.get().unwrap();
    let enclosing_component = element.enclosing_component.upgrade().unwrap();
    if enclosing_component.is_sub_component() {
        if *local_index == 0 {
            quote!(_self.tree_index.get() as usize)
        } else if *local_index == 1 {
            quote!(_self.tree_index_of_first_child.get() as usize)
        } else {
            quote!(_self.tree_index_of_first_child.get() as usize + #local_index)
        }
    } else {
        quote!(#local_index)
    }
}*/

/// Given a property reference to a native item (eg, the property name is empty)
/// return tokens to the `ItemRc`
fn access_item_rc(pr: &llr::PropertyReference, ctx: &EvaluationContext) -> TokenStream {
    /*
    match pr {
        llr::PropertyReference::InNativeItem { sub_component_path, item_index, prop_name } => {
            assert!(prop_name.is_empty());
            let _ =
                follow_sub_component_path(ctx.current_sub_component.unwrap(), sub_component_path);

        }
        llr::PropertyReference::InParent { level, parent_reference } => todo!(),
        _ => unreachable!(),
    }
    let focus_item = focus_item.upgrade().unwrap();
    let component_vrc = element_component_vrc(&focus_item, component);
    let item_index_tokens = absolute_element_item_index_expression(&focus_item);
    quote!(&ItemRc::new(#component_vrc, #item_index_tokens))
    */
    quote!(todo!("access_item_rc"))
}

#[derive(Clone, Copy)]
struct ParentCtx<'a> {
    ctx: &'a EvaluationContext<'a>,
    // Index of the repeater within the ctx.current_sub_component
    repeater_index: Option<usize>,
}

impl<'a> ParentCtx<'a> {
    fn new(ctx: &'a EvaluationContext<'a>, repeater_index: Option<usize>) -> Self {
        Self { ctx, repeater_index }
    }
}

#[derive(Clone)]
struct EvaluationContext<'a> {
    public_component: &'a llr::PublicComponent,
    current_sub_component: Option<&'a llr::SubComponent>,
    current_global: Option<&'a llr::GlobalComponent>,
    /// path to access the public_component (so one can access the globals).
    /// e.g: `_self` in case we already are the root
    root_access: TokenStream,
    /// The repeater parent
    parent: Option<ParentCtx<'a>>,

    /// The callback argument types
    argument_types: &'a [Type],
}

impl<'a> EvaluationContext<'a> {
    fn new_sub_component(
        public_component: &'a llr::PublicComponent,
        sub_component: &'a llr::SubComponent,
        parent: Option<ParentCtx<'a>>,
    ) -> Self {
        /*let root_access = if let Some(parent) = &parent {
            let p = &parent.ctx.root_access;
            quote!(parent.)
        } else {
            quote!(_self)
        };*/
        Self {
            public_component,
            current_sub_component: Some(sub_component),
            current_global: None,
            root_access: quote!(_self.root.get().unwrap().upgrade().unwrap()),
            parent,
            argument_types: &[],
        }
    }
}

impl<'a> llr::EvaluationContext for EvaluationContext<'a> {
    fn property_ty(&self, prop: &llr::PropertyReference) -> &Type {
        match prop {
            llr::PropertyReference::Local { sub_component_path, property_index } => {
                if let Some(mut sub_component) = self.current_sub_component {
                    for i in sub_component_path {
                        sub_component = &sub_component.sub_components[*i].ty;
                    }
                    &sub_component.properties[*property_index].ty
                } else if let Some(current_global) = self.current_global {
                    &current_global.properties[*property_index].ty
                } else {
                    unreachable!()
                }
            }
            llr::PropertyReference::InNativeItem { sub_component_path, item_index, prop_name } => {
                if prop_name == "elements" {
                    // The `Path::elements` property is not in the NativeClasss
                    return &Type::PathData;
                }

                let mut sub_component = self.current_sub_component.unwrap();
                for i in sub_component_path {
                    sub_component = &sub_component.sub_components[*i].ty;
                }
                sub_component.items[*item_index].ty.lookup_property(prop_name).unwrap()
            }
            llr::PropertyReference::InParent { level, parent_reference } => {
                let mut ctx = self;
                for _ in 0..level.get() {
                    ctx = ctx.parent.unwrap().ctx;
                }
                ctx.property_ty(parent_reference)
            }
            llr::PropertyReference::Global { global_index, property_index } => {
                &self.public_component.globals[*global_index].properties[*property_index].ty
            }
        }
    }

    fn arg_type(&self, index: usize) -> &Type {
        &self.argument_types[index]
    }
}

fn compile_expression(expr: &Expression, ctx: &EvaluationContext) -> TokenStream {
    match expr {
        Expression::StringLiteral(s) => quote!(sixtyfps::re_exports::SharedString::from(#s)),
        Expression::NumberLiteral(n) => quote!(#n),
        Expression::BoolLiteral(b) => quote!(#b),
        Expression::Cast { from, to } => {
            let f = compile_expression(&*from, ctx);
            match (from.ty(ctx), to) {
                (Type::Float32, Type::String) | (Type::Int32, Type::String) => {
                    quote!(sixtyfps::re_exports::SharedString::from(
                        sixtyfps::re_exports::format!("{}", #f).as_str()
                    ))
                }
                (Type::Float32, Type::Model) | (Type::Int32, Type::Model) => {
                    quote!(sixtyfps::re_exports::ModelHandle::new(sixtyfps::re_exports::Rc::<usize>::new(#f as usize)))
                }
                (Type::Float32, Type::Color) => {
                    quote!(sixtyfps::re_exports::Color::from_argb_encoded(#f as u32))
                }
                (Type::Color, Type::Brush) => {
                    quote!(sixtyfps::Brush::SolidColor(#f))
                }
                (Type::Brush, Type::Color) => {
                    quote!(#f.color())
                }
                (Type::Struct { ref fields, .. }, Type::Component(c)) => {
                    let fields = fields.iter().enumerate().map(|(index, (name, _))| {
                        let index = proc_macro2::Literal::usize_unsuffixed(index);
                        let name = ident(name);
                        quote!(#name: obj.#index as _)
                    });
                    let id: TokenStream = c.id.parse().unwrap();
                    quote!({ let obj = #f; #id { #(#fields),*} })
                }
                (Type::Struct { ref fields, .. }, Type::Struct { name: Some(n), .. }) => {
                    let fields = fields.iter().enumerate().map(|(index, (name, _))| {
                        let index = proc_macro2::Literal::usize_unsuffixed(index);
                        let name = ident(name);
                        quote!(#name: obj.#index as _)
                    });
                    let id = struct_name_to_tokens(n);
                    quote!({ let obj = #f; #id { #(#fields),*} })
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
                            .map(|path_elem_expr| compile_expression(path_elem_expr, ctx)),
                        _ => {
                            unreachable!()
                        }
                    };
                    quote!(sixtyfps::re_exports::PathData::Elements(sixtyfps::re_exports::SharedVector::<_>::from_slice(&[#((#path_elements).into()),*])))
                }
                _ => f,
            }
        }
        Expression::PropertyReference(nr) => {
            let access = access_member(nr, ctx);
            quote!(#access.get())
        }
        Expression::BuiltinFunctionCall { function, arguments } => {
            compile_builtin_function_call(*function, &arguments, ctx)
        }
        Expression::CallBackCall { callback, arguments } => {
            let f = access_member(callback, ctx);
            let a = arguments.iter().map(|a| compile_expression(a, ctx));
            quote! { #f.call(&(#(#a.clone() as _,)*).into())}
        }
        Expression::ExtraBuiltinFunctionCall { function, arguments } => {
            let f = ident(&function);
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
                quote!(sixtyfps::re_exports::Image::default())
            }
            crate::expression_tree::ImageReference::AbsolutePath(path) => {
                quote!(sixtyfps::re_exports::Image::load_from_path(::std::path::Path::new(#path)).unwrap())
            }
            crate::expression_tree::ImageReference::EmbeddedData { resource_id, extension } => {
                let symbol = format_ident!("SFPS_EMBEDDED_RESOURCE_{}", resource_id);
                let format = proc_macro2::Literal::byte_string(extension.as_bytes());
                quote!(
                    sixtyfps::re_exports::Image::from(
                        sixtyfps::re_exports::ImageInner::EmbeddedData{ data: #symbol.into(), format: Slice::from_slice(#format) }
                    )
                )
            }
            crate::expression_tree::ImageReference::EmbeddedTexture { resource_id } => {
                let symbol = format_ident!("SFPS_EMBEDDED_RESOURCE_{}", resource_id);
                quote!(
                    sixtyfps::re_exports::Image::from(#symbol)
                )
            }
        },
        Expression::Condition { condition, true_expr, false_expr } => {
            let condition_code = compile_expression(&*condition, ctx);
            let true_code = compile_expression(&*true_expr, ctx);
            let false_code = false_expr.as_ref().map(|e| compile_expression(e, ctx));
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
                let rust_element_ty = rust_type(element_ty).unwrap();
                quote!(sixtyfps::re_exports::ModelHandle::new(
                    sixtyfps::re_exports::Rc::new(sixtyfps::re_exports::VecModel::<#rust_element_ty>::from(
                        sixtyfps::re_exports::vec![#(#val as _),*]
                    ))
                ))
            } else {
                quote!(Slice::from_slice(&[#(#val),*]))
            }
        }
        Expression::Struct { ty, values } => {
            if let Type::Struct { fields, name, node } = ty {
                let elem = fields.keys().map(|k| {
                    values.get(k).map(|e| {
                        let e = compile_expression(e, ctx);
                        if node.is_none() && k == "padding" {
                            // FIXME: it would be nice if we didn't have to handle this field specially
                            quote!(&#e)
                        } else {
                            e
                        }
                    })
                });
                if let Some(name) = name {
                    let name: TokenStream = struct_name_to_tokens(name.as_str());
                    let keys = fields.keys().map(|k| ident(k));
                    quote!(#name { #(#keys: #elem as _,)* })
                } else {
                    let as_ = fields.values().map(|t| {
                        if t.as_unit_product().is_some() {
                            // number needs to be converted to the right things because intermediate
                            // result might be f64 and that's usually not what the type of the tuple is in the end
                            let t = rust_type(t).unwrap();
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
        Expression::PathEvents(_) => quote!(todo!("Expression::PathEvents")),
        Expression::StoreLocalVariable { name, value } => {
            let value = compile_expression(value, ctx);
            let name = ident(name);
            quote!(let #name = #value;)
        }
        Expression::ReadLocalVariable { name, .. } => {
            let name = ident(name);
            quote!(#name)
        }
        Expression::EasingCurve(EasingCurve::Linear) => {
            quote!(sixtyfps::re_exports::EasingCurve::Linear)
        }
        Expression::EasingCurve(EasingCurve::CubicBezier(a, b, c, d)) => {
            quote!(sixtyfps::re_exports::EasingCurve::CubicBezier([#a, #b, #c, #d]))
        }
        Expression::LinearGradient { angle, stops } => {
            let angle = compile_expression(angle, ctx);
            let stops = stops.iter().map(|(color, stop)| {
                let color = compile_expression(color, ctx);
                let position = compile_expression(stop, ctx);
                quote!(sixtyfps::re_exports::GradientStop{ color: #color, position: #position as _ })
            });
            quote!(sixtyfps::Brush::LinearGradient(
                sixtyfps::re_exports::LinearGradientBrush::new(#angle as _, [#(#stops),*].iter().cloned())
            ))
        }
        Expression::EnumerationValue(value) => {
            let base_ident = ident(&value.enumeration.name);
            let value_ident = ident(&value.to_string());
            quote!(sixtyfps::re_exports::#base_ident::#value_ident)
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
                    *cache.get((cache[#index] as usize) + #offset as usize * 2).unwrap_or(&0.)
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
                let window_tokens = access_window_field(ctx);
                let focus_item = access_item_rc(pr, ctx);
                quote!(
                    #window_tokens.clone().set_focus_item(#focus_item);
                )
            } else {
                panic!("internal error: invalid args to SetFocusItem {:?}", arguments)
            }
        }
        BuiltinFunction::ShowPopupWindow => {
            if let [Expression::NumberLiteral(popup_index), x, y, Expression::PropertyReference(parent_ref)] =
                arguments
            {
                let current_sub_component = ctx.current_sub_component.unwrap();
                let popup_window_id = inner_component_id(
                    &current_sub_component.popup_windows[*popup_index as usize].root,
                );
                let parent_component = quote!(todo!("BuiltinFunction::ShowPopupWindow"));
                let x = compile_expression(x, ctx);
                let y = compile_expression(y, ctx);
                let window_tokens = access_window_field(ctx);
                quote!(
                    #window_tokens.show_popup(
                        &VRc::into_dyn(#popup_window_id::new(_self.self_weak.get().unwrap().clone()).into()),
                        Point::new(#x, #y),
                        #parent_component
                    );
                )
            } else {
                panic!("internal error: invalid args to ShowPopupWindow {:?}", arguments)
            }
        }
        BuiltinFunction::ImplicitLayoutInfo(orient) => {
            if let [Expression::PropertyReference(pr)] = arguments {
                let item = access_member(pr, ctx);
                let window_tokens = access_window_field(ctx);
                quote!(
                    #item.layout_info(#orient, #window_tokens)
                )
            } else {
                panic!("internal error: invalid args to ImplicitLayoutInfo {:?}", arguments)
            }
        }
        BuiltinFunction::RegisterCustomFontByPath => {
            if let [Expression::StringLiteral(path)] = arguments {
                quote!(sixtyfps::register_font_from_path(&std::path::PathBuf::from(#path));)
            } else {
                panic!("internal error: invalid args to RegisterCustomFontByPath {:?}", arguments)
            }
        }
        BuiltinFunction::RegisterCustomFontByMemory => {
            if let [Expression::NumberLiteral(resource_id)] = &arguments {
                let resource_id: usize = *resource_id as _;
                let symbol = format_ident!("SFPS_EMBEDDED_RESOURCE_{}", resource_id);
                quote!(sixtyfps::register_font_from_memory(#symbol.into());)
            } else {
                panic!("internal error: invalid args to RegisterCustomFontByMemory {:?}", arguments)
            }
        }
        BuiltinFunction::GetWindowScaleFactor => {
            let window_tokens = access_window_field(ctx);
            quote!(#window_tokens.scale_factor())
        }
        BuiltinFunction::Debug => quote!(println!("{:?}", #(#a)*)),
        BuiltinFunction::Mod => quote!((#(#a as i32)%*)),
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
            quote!(match #(#a)* { x => {
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
                sixtyfps::re_exports::Color::from_argb_u8(a, r, g, b)
            })
        }
    }
}

/// Return a TokenStream for a name (as in [`Type::Struct::name`])
fn struct_name_to_tokens(name: &str) -> TokenStream {
    // the name match the C++ signature so we need to change that to the rust namespace
    let mut name = name.replace("::private_api::", "::re_exports::").replace('-', "_");
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
    let ri2 =
        repeated_indices.map(|ri| quote!(let #ri = sixtyfps::re_exports::Slice::from_slice(&#ri);));
    let cells_variable = ident(cells_variable);
    let sub_expression = compile_expression(sub_expression, ctx);

    quote! { {
        #ri
        let mut items_vec = sixtyfps::re_exports::Vec::with_capacity(#fixed_count #repeated_count);
        #(#push_code)*
        let #cells_variable = sixtyfps::re_exports::Slice::from_slice(&items_vec);
        #ri2
        #sub_expression
    } }
}

/*
fn compile_path_events(events: &[crate::expression_tree::PathEvent]) -> TokenStream {
    use lyon_path::Event;

    let mut coordinates = Vec::new();

    let converted_events: Vec<proc_macro2::TokenStream> = events
        .iter()
        .map(|event| match event {
            Event::Begin { at } => {
                coordinates.push(at);
                quote!(sixtyfps::re_exports::PathEvent::Begin)
            }
            Event::Line { from, to } => {
                coordinates.push(from);
                coordinates.push(to);
                quote!(sixtyfps::re_exports::PathEvent::Line)
            }
            Event::Quadratic { from, ctrl, to } => {
                coordinates.push(from);
                coordinates.push(ctrl);
                coordinates.push(to);
                quote!(sixtyfps::re_exports::PathEvent::Quadratic)
            }
            Event::Cubic { from, ctrl1, ctrl2, to } => {
                coordinates.push(from);
                coordinates.push(ctrl1);
                coordinates.push(ctrl2);
                coordinates.push(to);
                quote!(sixtyfps::re_exports::PathEvent::Cubic)
            }
            Event::End { close, .. } => {
                if *close {
                    quote!(sixtyfps::re_exports::PathEvent::EndClosed)
                } else {
                    quote!(sixtyfps::re_exports::PathEvent::EndOpen)
                }
            }
        })
        .collect();

    let coordinates: Vec<TokenStream> = coordinates
        .into_iter()
        .map(|pt| {
            let x = pt.x;
            let y = pt.y;
            quote!(sixtyfps::re_exports::Point::new(#x, #y))
        })
        .collect();

    quote!(sixtyfps::re_exports::SharedVector::<sixtyfps::re_exports::PathEvent>::from_slice(&[#(#converted_events),*]),
           sixtyfps::re_exports::SharedVector::<sixtyfps::re_exports::Point>::from_slice(&[#(#coordinates),*]))
}

fn compile_path(path: &Path, component: &Rc<Component>) -> TokenStream {
    match path {
        Path::Elements(elements) => {
            let converted_elements: Vec<TokenStream> = elements
                .iter()
                .map(|element| {
                    let mut bindings = element
                        .bindings
                        .iter()
                        .map(|(property, expr)| {
                            let prop_ident = ident(property);
                            let binding_expr = compile_expression(&expr.borrow(), component);

                            quote!(#prop_ident: #binding_expr as _).to_string()
                        })
                        .collect::<Vec<String>>();

                    if bindings.len() < element.element_type.properties.len() {
                        bindings.push("..Default::default()".into())
                    }

                    let bindings = bindings.join(",");

                    let ctor_format_string = element
                        .element_type
                        .native_class.rust_type_constructor
                        .as_ref()
                        .expect(
                        "Unexpected error in type registry: path element is lacking rust type name",
                    );

                    ctor_format_string
                        .replace("{}", &bindings)
                        .parse()
                        .expect("Error parsing rust path element constructor")
                })
                .collect();

            quote!(sixtyfps::re_exports::PathData::Elements(
                sixtyfps::re_exports::SharedVector::<sixtyfps::re_exports::PathElement>::from_slice(&[#(#converted_elements),*])
            ))
        }
        Path::Events(events) => {
            let events = compile_path_events(events);
            quote!(sixtyfps::re_exports::PathData::Events(#events))
        }
    }
}
*/
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
