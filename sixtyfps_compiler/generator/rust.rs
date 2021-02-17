/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
/*! module for the Rust code generator

Some convention used in the generated code:
 - `_self` is of type `Pin<&ComponentType>`  where ComponentType is the type of the generated component
*/

use crate::diagnostics::{BuildDiagnostics, CompilerDiagnostic, Level, Spanned};
use crate::expression_tree::{
    BuiltinFunction, EasingCurve, Expression, NamedReference, OperatorClass, Path,
};
use crate::langtype::Type;
use crate::layout::LayoutGeometry;
use crate::object_tree::{Component, Document, ElementRc};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use std::{collections::BTreeMap, rc::Rc};

fn rust_type(
    ty: &Type,
    span: &crate::diagnostics::Span,
) -> Result<proc_macro2::TokenStream, CompilerDiagnostic> {
    match ty {
        Type::Int32 => Ok(quote!(i32)),
        Type::Float32 => Ok(quote!(f32)),
        Type::String => Ok(quote!(sixtyfps::re_exports::SharedString)),
        Type::Color => Ok(quote!(sixtyfps::re_exports::Color)),
        Type::Duration => Ok(quote!(i64)),
        Type::Angle => Ok(quote!(f32)),
        Type::Length => Ok(quote!(f32)),
        Type::LogicalLength => Ok(quote!(f32)),
        Type::Percent => Ok(quote!(f32)),
        Type::Bool => Ok(quote!(bool)),
        Type::Resource => Ok(quote!(sixtyfps::re_exports::Resource)),
        Type::Object { fields, name: None } => {
            let elem =
                fields.values().map(|v| rust_type(v, span)).collect::<Result<Vec<_>, _>>()?;
            // This will produce a tuple
            Ok(quote!((#(#elem,)*)))
        }
        Type::Object { name: Some(name), .. } => Ok(name.parse().unwrap()),
        Type::Array(o) => {
            let inner = rust_type(&o, span)?;
            Ok(quote!(sixtyfps::re_exports::ModelHandle<#inner>))
        }
        Type::Enumeration(e) => {
            let e = format_ident!("{}", e.name);
            Ok(quote!(sixtyfps::re_exports::#e))
        }
        Type::Brush => Ok(quote!(sixtyfps::Brush)),
        _ => Err(CompilerDiagnostic {
            message: format!("Cannot map property type {} to Rust", ty),
            span: span.clone(),
            level: Level::Error,
        }),
    }
}

/// Generate the rust code for the given component.
///
/// Fill the diagnostic in case of error.
pub fn generate(doc: &Document, diag: &mut BuildDiagnostics) -> Option<TokenStream> {
    let (structs_ids, structs): (Vec<_>, Vec<_>) = doc
        .root_component
        .used_structs
        .borrow()
        .iter()
        .filter_map(|ty| {
            if let Type::Object { fields, name: Some(name) } = ty {
                Some((format_ident!("{}", name), generate_struct(name, fields, diag)))
            } else {
                None
            }
        })
        .unzip();
    let compo = generate_component(&doc.root_component, diag)?;
    let compo_id = public_component_id(&doc.root_component);
    let compo_module = format_ident!("sixtyfps_generated_{}", compo_id);
    let version_check = format_ident!(
        "VersionCheck_{}_{}_{}",
        env!("CARGO_PKG_VERSION_MAJOR"),
        env!("CARGO_PKG_VERSION_MINOR"),
        env!("CARGO_PKG_VERSION_PATCH"),
    );
    let globals = doc
        .root_component
        .used_global
        .borrow()
        .iter()
        .filter(|glob| !matches!(glob.root_element.borrow().base_type, Type::Builtin(_)))
        .filter_map(|glob| generate_component(glob, diag))
        .collect::<Vec<_>>();
    Some(quote! {
        #[allow(non_snake_case)]
        mod #compo_module {
            use sixtyfps::re_exports::*;
            #(#structs)*
            #(#globals)*
            #compo
            const _THE_SAME_VERSION_MUST_BE_USED_FOR_THE_COMPILER_AND_THE_RUNTIME : sixtyfps::#version_check = sixtyfps::#version_check;
        }
        pub use #compo_module::{#compo_id #(,#structs_ids)* };
        pub use sixtyfps::IntoWeak;
    })
}

fn generate_struct(
    name: &str,
    fields: &BTreeMap<String, Type>,
    diag: &mut BuildDiagnostics,
) -> TokenStream {
    let component_id: TokenStream = name.parse().unwrap();
    let (declared_property_vars, declared_property_types): (Vec<_>, Vec<_>) = fields
        .iter()
        .map(|(name, ty)| {
            (
                format_ident!("{}", name),
                rust_type(ty, &Default::default()).unwrap_or_else(|err| {
                    diag.push_internal_error(err.into());
                    quote!(())
                }),
            )
        })
        .unzip();

    quote! {
        #[derive(Default, PartialEq, Debug, Clone)]
        pub struct #component_id {
            #(pub #declared_property_vars : #declared_property_types),*
        }
    }
}

fn handle_property_binding(
    component: &Rc<Component>,
    item_rc: &ElementRc,
    prop_name: &str,
    binding_expression: &Expression,
    init: &mut Vec<TokenStream>,
) {
    let rust_property = access_member(item_rc, prop_name, component, quote!(_self), false);
    let prop_type = item_rc.borrow().lookup_property(prop_name).property_type;
    let (downgrade, upgrade) = if item_rc
        .borrow()
        .enclosing_component
        .upgrade()
        .unwrap()
        .is_global()
    {
        (
            quote!(let self_weak = sixtyfps::re_exports::PinWeak::downgrade(self_pinned.clone());),
            quote!(
                let self_pinned = self_weak.upgrade().unwrap();
                let _self = self_pinned.as_ref();
            ),
        )
    } else {
        (
            quote!(let self_weak = sixtyfps::re_exports::VRc::downgrade(&self_pinned);),
            quote!(
                let self_pinned = self_weak.upgrade().unwrap();
                let _self = self_pinned.as_pin_ref();
            ),
        )
    };

    if matches!(prop_type, Type::Callback { .. }) {
        let tokens_for_expression = compile_expression(binding_expression, &component);
        init.push(quote!(
            #rust_property.set_handler({
                #downgrade
                move |args| {
                    #upgrade
                    (#tokens_for_expression) as _
                }
            });
        ));
    } else if let Expression::TwoWayBinding(nr, next) = &binding_expression {
        let p2 = access_member(
            &nr.element.upgrade().unwrap(),
            &nr.name,
            component,
            quote!(_self),
            false,
        );
        init.push(quote!(
            Property::link_two_way(#rust_property, #p2);
        ));
        if let Some(next) = next {
            handle_property_binding(component, item_rc, prop_name, next, init)
        }
    } else {
        let tokens_for_expression = compile_expression(binding_expression, &component);
        init.push(if binding_expression.is_constant() {
            let t = rust_type(&prop_type, &Default::default()).unwrap_or(quote!(_));
            quote! { #rust_property.set((||-> #t { (#tokens_for_expression) as #t })()); }
        } else {
            let binding_tokens = quote!({
                #downgrade
                move || {
                    #upgrade
                    (#tokens_for_expression) as _
                }
            });

            let is_state_info = matches!(prop_type, Type::Object { name: Some(name), .. } if name.ends_with("::StateInfo"));
            if is_state_info {
                quote! { sixtyfps::re_exports::set_state_binding(#rust_property, #binding_tokens); }
            } else {
                match item_rc.borrow().property_animations.get(prop_name) {
                    Some(crate::object_tree::PropertyAnimation::Static(anim)) => {
                        let anim = property_animation_tokens(component, anim);
                        quote! { #rust_property.set_animated_binding(#binding_tokens, #anim); }
                    }
                    Some(crate::object_tree::PropertyAnimation::Transition {
                        state_ref,
                        animations,
                    }) => {
                        let state_tokens = compile_expression(state_ref, component);
                        let anim_expr = animations.iter().map(|a| {
                            let cond = compile_expression(
                                &a.condition(Expression::ReadLocalVariable {
                                    name: "state".into(),
                                    ty: state_ref.ty(),
                                }),
                                component,
                            );
                            let a_tokens = property_animation_tokens(component, &a.animation);
                            quote!(if #cond { #a_tokens })
                        });
                        quote! {
                            #downgrade
                            #rust_property.set_animated_binding_for_transition(#binding_tokens, move || {
                                #upgrade
                                let state = #state_tokens;
                                ({ #(#anim_expr else)* { sixtyfps::re_exports::PropertyAnimation::default() }  }, state.change_time)
                            });
                        }
                    }
                    None => {
                        quote! { #rust_property.set_binding(#binding_tokens); }
                    }
                }
            }
        });
    }
}

/// Generate the rust code for the given component.
///
/// Fill the diagnostic in case of error.
fn generate_component(
    component: &Rc<Component>,
    diag: &mut BuildDiagnostics,
) -> Option<TokenStream> {
    let inner_component_id = inner_component_id(component);

    let mut extra_components = component
        .popup_windows
        .borrow()
        .iter()
        .filter_map(|c| generate_component(&c.component, diag))
        .collect::<Vec<_>>();
    let mut declared_property_vars = vec![];
    let mut declared_property_types = vec![];
    let mut declared_callbacks = vec![];
    let mut declared_callbacks_types = vec![];
    let mut declared_callbacks_ret = vec![];
    let mut property_and_callback_accessors: Vec<TokenStream> = vec![];
    for (prop_name, property_decl) in component.root_element.borrow().property_declarations.iter() {
        let prop_ident = format_ident!("{}", prop_name);
        if let Type::Callback { args, return_type } = &property_decl.property_type {
            declared_callbacks.push(prop_ident.clone());
            let callback_args = args
                .iter()
                .map(|a| rust_type(a, &property_decl.type_node.span()))
                .collect::<Result<Vec<_>, _>>()
                .unwrap_or_else(|err| {
                    diag.push_internal_error(err.into());
                    vec![]
                });
            let return_type = return_type.as_ref().map_or(quote!(()), |a| {
                rust_type(&a, &property_decl.type_node.span()).unwrap_or_else(|err| {
                    diag.push_internal_error(err.into());
                    quote!()
                })
            });

            if property_decl.expose_in_public_api {
                let args_name = (0..callback_args.len())
                    .map(|i| format_ident!("arg_{}", i))
                    .collect::<Vec<_>>();
                let caller_ident = format_ident!("call_{}", prop_name);
                property_and_callback_accessors.push(
                    quote!(
                        #[allow(dead_code)]
                        pub fn #caller_ident(&self, #(#args_name : #callback_args,)*) -> #return_type {
                            let self_pinned = vtable::VRc::as_pin_ref(&self.0);
                            #inner_component_id::FIELD_OFFSETS.#prop_ident.apply_pin(self_pinned).call(&(#(#args_name,)*))
                        }
                    )
                    ,
                );

                let on_ident = format_ident!("on_{}", prop_name);
                let args_index =
                    (0..callback_args.len()).map(proc_macro2::Literal::usize_unsuffixed);
                property_and_callback_accessors.push(
                    quote!(
                        #[allow(dead_code)]
                        pub fn #on_ident(&self, f: impl Fn(#(#callback_args),*) -> #return_type + 'static) {
                            let self_pinned = vtable::VRc::as_pin_ref(&self.0);
                            #[allow(unused)]
                            #inner_component_id::FIELD_OFFSETS.#prop_ident.apply_pin(self_pinned).set_handler(
                                // FIXME: why do i need to clone here?
                                move |args| f(#(args.#args_index.clone()),*)
                            )
                        }
                    )
                    ,
                );
            }
            declared_callbacks_types.push(callback_args);
            declared_callbacks_ret.push(return_type);
        } else {
            let rust_property_type =
                rust_type(&property_decl.property_type, &property_decl.type_node.span())
                    .unwrap_or_else(|err| {
                        diag.push_internal_error(err.into());
                        quote!()
                    });
            if property_decl.expose_in_public_api {
                let getter_ident = format_ident!("get_{}", prop_name);
                let setter_ident = format_ident!("set_{}", prop_name);

                let prop = if let Some(alias) = &property_decl.is_alias {
                    access_named_reference(alias, component, quote!(_self))
                } else {
                    quote!(#inner_component_id::FIELD_OFFSETS.#prop_ident.apply_pin(_self))
                };

                property_and_callback_accessors.push(quote!(
                    #[allow(dead_code)]
                    pub fn #getter_ident(&self) -> #rust_property_type {
                        #[allow(unused_imports)]
                        use sixtyfps::re_exports::*;
                        let _self = vtable::VRc::as_pin_ref(&self.0);
                        #prop.get()
                    }
                ));

                let set_value = property_set_value_tokens(
                    component,
                    &component.root_element,
                    prop_name,
                    quote!(value),
                );
                property_and_callback_accessors.push(quote!(
                    #[allow(dead_code)]
                    pub fn #setter_ident(&self, value: #rust_property_type) {
                        #[allow(unused_imports)]
                        use sixtyfps::re_exports::*;
                        let _self = vtable::VRc::as_pin_ref(&self.0);
                        #prop.#set_value
                    }
                ));
            }

            if property_decl.is_alias.is_none() {
                declared_property_vars.push(prop_ident.clone());
                declared_property_types.push(rust_property_type.clone());
            }
        }
    }

    if diag.has_error() {
        return None;
    }

    let mut item_tree_array = Vec::new();
    let mut item_names = Vec::new();
    let mut item_types = Vec::new();
    let mut repeated_element_names = Vec::new();
    let mut repeated_element_layouts = Vec::new();
    let mut repeated_element_components = Vec::new();
    let mut repeated_visit_branch = Vec::new();
    let mut repeated_input_branch = Vec::new();
    let mut init = Vec::new();
    let mut window_field_init = None;
    let mut window_parent_param = None;
    super::build_array_helper(
        component,
        |item_rc, children_index, parent_index, is_flickable_rect| {
            let parent_index = parent_index as u32;
            let item = item_rc.borrow();
            if is_flickable_rect {
                let field_name = format_ident!("{}", item.id);
                let children_count = item.children.len() as u32;
                let children_index = item_tree_array.len() as u32 + 1;

                item_tree_array.push(quote!(
                    sixtyfps::re_exports::ItemTreeNode::Item{
                        item: VOffset::new(#inner_component_id::FIELD_OFFSETS.#field_name + sixtyfps::re_exports::Flickable::FIELD_OFFSETS.viewport),
                        chilren_count: #children_count,
                        children_index: #children_index,
                        parent_index: #parent_index
                    }
                ));
            } else if item.base_type == Type::Void {
                assert!(component.is_global());
                for (k, binding_expression) in &item.bindings {
                    handle_property_binding(component, item_rc, k, binding_expression, &mut init);
                }
            } else if let Some(repeated) = &item.repeated {
                let base_component = item.base_type.as_component();
                let repeater_index = repeated_element_names.len();
                let repeater_id = format_ident!("repeater_{}", item.id);
                let rep_inner_component_id = self::inner_component_id(&*base_component);

                extra_components.push(generate_component(&*base_component, diag).unwrap_or_else(
                    || {
                        assert!(diag.has_error());
                        Default::default()
                    },
                ));

                let extra_fn = if repeated.is_listview.is_some() {
                    let am = |prop| {
                        access_member(
                            &base_component.root_element,
                            prop,
                            base_component,
                            quote!(self),
                            false,
                        )
                    };
                    let p_y = am("y");
                    let p_height = am("height");
                    let p_width = am("width");
                    quote! {
                        fn listview_layout(
                            self: core::pin::Pin<&Self>,
                            offset_y: &mut f32,
                            viewport_width: core::pin::Pin<&sixtyfps::re_exports::Property<f32>>,
                        ) {
                            use sixtyfps::re_exports::*;
                            let vp_w = viewport_width.get();
                            self.apply_layout(Rect::new(Point::new(0., *offset_y), Size::new(vp_w, 0.)));
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
                    let root_id = format_ident!("{}", base_component.root_element.borrow().id);
                    let root_c = &base_component.layouts.borrow().root_constraints;
                    let width = if root_c.fixed_width {
                        quote!(None)
                    } else {
                        quote!(Some(&self.get_ref().#root_id.width))
                    };
                    let height = if root_c.fixed_height {
                        quote!(None)
                    } else {
                        quote!(Some(&self.get_ref().#root_id.height))
                    };
                    quote! {
                        fn box_layout_data<'a>(self: ::core::pin::Pin<&'a Self>) -> sixtyfps::re_exports::BoxLayoutCellData<'a> {
                            use sixtyfps::re_exports::*;
                            BoxLayoutCellData {
                                constraint: self.layout_info(),
                                x: Some(&self.get_ref().#root_id.x),
                                y: Some(&self.get_ref().#root_id.y),
                                width: #width,
                                height: #height,
                            }
                        }
                    }
                };

                extra_components.push(if repeated.is_conditional_element {
                    quote! {
                        impl sixtyfps::re_exports::RepeatedComponent for #rep_inner_component_id {
                            type Data = ();
                            fn update(&self, _: usize, _: Self::Data) { }
                            #extra_fn
                        }
                    }
                } else {
                    let data_type = rust_type(
                        &Expression::RepeaterModelReference { element: Rc::downgrade(item_rc) }
                            .ty(),
                        &item.node.as_ref().map_or_else(Default::default, |n| n.span()),
                    )
                    .unwrap_or_else(|err| {
                        diag.push_internal_error(err.into());
                        quote!()
                    });

                    quote! {
                        impl sixtyfps::re_exports::RepeatedComponent for #rep_inner_component_id {
                            type Data = #data_type;
                            fn update(&self, index: usize, data: Self::Data) {
                                self.index.set(index);
                                self.model_data.set(data);
                            }
                            #extra_fn
                        }
                    }
                });

                let mut model = compile_expression(&repeated.model, component);
                if repeated.is_conditional_element {
                    model = quote!(sixtyfps::re_exports::ModelHandle::new(std::rc::Rc::<bool>::new(#model)))
                }

                // FIXME: there could be an optimization if `repeated.model.is_constant()`, we don't need a binding
                init.push(quote! {
                    self_pinned.#repeater_id.set_model_binding({
                        let self_weak = sixtyfps::re_exports::VRc::downgrade(&self_pinned);
                        move || {
                            let self_pinned = self_weak.upgrade().unwrap();
                            let _self = self_pinned.as_pin_ref();
                            (#model) as _
                        }
                    });
                });

                if let Some(listview) = &repeated.is_listview {
                    let vp_y =
                        access_named_reference(&listview.viewport_y, component, quote!(_self));
                    let vp_h =
                        access_named_reference(&listview.viewport_height, component, quote!(_self));
                    let lv_h =
                        access_named_reference(&listview.listview_height, component, quote!(_self));
                    let vp_w =
                        access_named_reference(&listview.viewport_width, component, quote!(_self));
                    let lv_w =
                        access_named_reference(&listview.listview_width, component, quote!(_self));

                    let ensure_updated = quote! {
                        #inner_component_id::FIELD_OFFSETS.#repeater_id.apply_pin(self_pinned).ensure_updated_listview(
                            || { #rep_inner_component_id::new(self_pinned.self_weak.get().unwrap().clone(), &self_pinned.window).into() },
                            #vp_w, #vp_h, #vp_y, #lv_w.get(), #lv_h
                        );
                    };

                    repeated_visit_branch.push(quote!(
                        #repeater_index => {
                            #ensure_updated
                            self_pinned.#repeater_id.visit(order, visitor)
                        }
                    ));

                    repeated_element_layouts.push(quote!(
                        #ensure_updated
                    ));
                } else {
                    let ensure_updated = quote! {
                        #inner_component_id::FIELD_OFFSETS.#repeater_id.apply_pin(self_pinned).ensure_updated(
                            || { #rep_inner_component_id::new(self_pinned.self_weak.get().unwrap().clone(), &self_pinned.window).into() }
                        );
                    };

                    repeated_visit_branch.push(quote!(
                        #repeater_index => {
                            #ensure_updated
                            self_pinned.#repeater_id.visit(order, visitor)
                        }
                    ));

                    repeated_element_layouts.push(quote!(
                        #ensure_updated
                        self_pinned.#repeater_id.compute_layout();
                    ));
                }

                repeated_input_branch.push(quote!(
                    #repeater_index => self.#repeater_id.input_event(rep_index, event, window),
                ));

                item_tree_array.push(quote!(
                    sixtyfps::re_exports::ItemTreeNode::DynamicTree {
                        index: #repeater_index,
                        parent_index: #parent_index,
                    }
                ));

                repeated_element_names.push(repeater_id);
                repeated_element_components.push(rep_inner_component_id);
            } else {
                let field_name = format_ident!("{}", item.id);
                let children_count =
                    if super::is_flickable(item_rc) { 1 } else { item.children.len() as u32 };

                item_tree_array.push(quote!(
                    sixtyfps::re_exports::ItemTreeNode::Item{
                        item: VOffset::new(#inner_component_id::FIELD_OFFSETS.#field_name),
                        chilren_count: #children_count,
                        children_index: #children_index,
                        parent_index: #parent_index,
                    }
                ));
                for (k, binding_expression) in &item.bindings {
                    handle_property_binding(component, item_rc, k, binding_expression, &mut init);
                }
                item_names.push(field_name);
                item_types.push(format_ident!("{}", item.base_type.as_native().class_name));
            }
        },
    );

    let resource_symbols: Vec<proc_macro2::TokenStream> = component
        .embedded_file_resources
        .borrow()
        .iter()
        .map(|(path, id)| {
            let symbol = format_ident!("SFPS_EMBEDDED_RESOURCE_{}", id);
            quote!(const #symbol: &'static [u8] = ::core::include_bytes!(#path);)
        })
        .collect();

    let layouts = compute_layout(component, &repeated_element_layouts);
    let mut visibility = None;
    let mut parent_component_type = None;
    let mut has_window_impl = None;
    let mut window_field = Some(quote!(window: sixtyfps::re_exports::ComponentWindow));
    if let Some(parent_element) = component.parent_element.upgrade() {
        if parent_element.borrow().repeated.as_ref().map_or(false, |r| !r.is_conditional_element) {
            declared_property_vars.push(format_ident!("index"));
            declared_property_types.push(quote!(usize));
            declared_property_vars.push(format_ident!("model_data"));
            declared_property_types.push(
                rust_type(
                    &Expression::RepeaterModelReference {
                        element: component.parent_element.clone(),
                    }
                    .ty(),
                    &parent_element
                        .borrow()
                        .node
                        .as_ref()
                        .map_or_else(Default::default, |n| n.span()),
                )
                .unwrap_or_else(|err| {
                    diag.push_internal_error(err.into());
                    quote!()
                }),
            );
        }

        parent_component_type = Some(self::inner_component_id(
            &parent_element.borrow().enclosing_component.upgrade().unwrap(),
        ));
        window_field_init = Some(quote!(window: parent_window.clone()));
        window_parent_param = Some(quote!(, parent_window: &sixtyfps::re_exports::ComponentWindow))
    } else if !component.is_global() {
        // FIXME: This field is public for testing.
        window_field = Some(quote!(pub window: sixtyfps::re_exports::ComponentWindow));
        window_field_init = Some(quote!(window: sixtyfps::create_window()));

        visibility = Some(quote!(pub));

        init.push(quote!(_self.window.set_component(&VRc::into_dyn(_self.as_ref().self_weak.get().unwrap().upgrade().unwrap()));));

        has_window_impl = Some(quote!(
            impl sixtyfps::testing::HasWindow for #inner_component_id {
                fn component_window(&self) -> &sixtyfps::re_exports::ComponentWindow {
                    &self.window
                }
            }
        ))
    } else {
        window_field = None;
    };

    // Trick so we can use `#()` as a `if let Some` in `quote!`
    let parent_component_type = parent_component_type.iter().collect::<Vec<_>>();

    if diag.has_error() {
        return None;
    }

    let (drop_impl, pin) = if component.is_global() {
        (None, quote!(#[pin]))
    } else {
        (
            Some(quote!(impl sixtyfps::re_exports::PinnedDrop for #inner_component_id {
                fn drop(self: core::pin::Pin<&mut #inner_component_id>) {
                    use sixtyfps::re_exports::*;
                    let items = [
                        #(VRef::new_pin(Self::FIELD_OFFSETS.#item_names.apply_pin(self.as_ref())),)*
                    ];
                    self.window.free_graphics_resources(&Slice::from_slice(&items));
                }
            })),
            quote!(#[pin_drop]),
        )
    };

    for extra_init_code in component.setup_code.borrow().iter() {
        init.push(compile_expression(extra_init_code, component));
    }

    let (item_tree_impl, component_impl) = if component.is_global() {
        (None, None)
    } else {
        let item_tree_array_len = item_tree_array.len();
        let parent_item_index = component
            .parent_element
            .upgrade()
            .and_then(|e| e.borrow().item_index.get().map(|x| *x));
        let parent_item_index = parent_item_index.iter();
        init.insert(0, quote!(sixtyfps::re_exports::init_component_items(_self, Self::item_tree(), &_self.window);));
        (
            Some(quote! {
                fn item_tree() -> &'static [sixtyfps::re_exports::ItemTreeNode<Self>] {
                    use sixtyfps::re_exports::*;
                    ComponentVTable_static!(static VT for #inner_component_id);
                    // FIXME: ideally this should be a const
                    static ITEM_TREE : Lazy<[sixtyfps::re_exports::ItemTreeNode<#inner_component_id>; #item_tree_array_len]>  =
                        Lazy::new(|| [#(#item_tree_array),*]);
                    &*ITEM_TREE
                }
            }),
            Some(quote! {
                impl sixtyfps::re_exports::Component for #inner_component_id {
                    fn visit_children_item(self: ::core::pin::Pin<&Self>, index: isize, order: sixtyfps::re_exports::TraversalOrder, visitor: sixtyfps::re_exports::ItemVisitorRefMut)
                        -> sixtyfps::re_exports::VisitChildrenResult
                    {
                        use sixtyfps::re_exports::*;
                        return sixtyfps::re_exports::visit_item_tree(self, &VRc::into_dyn(self.as_ref().self_weak.get().unwrap().upgrade().unwrap()), Self::item_tree(), index, order, visitor, visit_dynamic);
                        #[allow(unused)]
                        fn visit_dynamic(self_pinned: ::core::pin::Pin<&#inner_component_id>, order: sixtyfps::re_exports::TraversalOrder, visitor: ItemVisitorRefMut, dyn_index: usize) -> VisitChildrenResult  {
                            let _self = self_pinned;
                            match dyn_index {
                                #(#repeated_visit_branch)*
                                _ => panic!("invalid dyn_index {}", dyn_index),
                            }
                        }
                    }


                    #layouts

                    fn get_item_ref(self: ::core::pin::Pin<&Self>, index: usize) -> ::core::pin::Pin<ItemRef> {
                        match &Self::item_tree()[index] {
                            ItemTreeNode::Item { item, .. } => item.apply_pin(self),
                            ItemTreeNode::DynamicTree { .. } => panic!("get_item_ref called on dynamic tree"),

                        }
                    }

                    fn parent_item(self: ::core::pin::Pin<&Self>, index: usize, result: &mut sixtyfps::re_exports::ItemWeak) {
                        if index == 0 {
                            #(
                                if let Some(parent) = self.parent.clone().into_dyn().upgrade() {
                                    *result = sixtyfps::re_exports::ItemRc::new(parent, #parent_item_index).parent_item();
                                }
                            )*
                            return;
                        }
                        let parent_index = match &Self::item_tree()[index] {
                            ItemTreeNode::Item { parent_index, .. } => *parent_index,
                            ItemTreeNode::DynamicTree { parent_index, .. } => *parent_index,
                        };
                        let self_rc = self.self_weak.get().unwrap().clone().into_dyn().upgrade().unwrap();
                        *result = ItemRc::new(self_rc, parent_index as _).downgrade()
                    }
                }
            }),
        )
    };

    let (global_name, global_type): (Vec<_>, Vec<_>) = component
        .used_global
        .borrow()
        .iter()
        .map(|g| (format_ident!("global_{}", g.id), self::inner_component_id(g)))
        .unzip();

    let new_code = if !component.is_global() {
        quote! {
            let self_pinned = VRc::new(self_);
            self_pinned.self_weak.set(VRc::downgrade(&self_pinned)).map_err(|_|())
                .expect("Can only be pinned once");
            let _self = self_pinned.as_pin_ref();
        }
    } else {
        quote! {
            let self_pinned = ::std::rc::Rc::pin(self_);
            let _self = self_pinned.as_ref();
        }
    };
    let self_weak = if !component.is_global() { Some(quote!(self_weak)) } else { None };
    let self_weak = self_weak.into_iter().collect::<Vec<_>>();
    let component_handle = if !component.is_global() {
        quote!(vtable::VRc<sixtyfps::re_exports::ComponentVTable, Self>)
    } else {
        quote!(::core::pin::Pin<::std::rc::Rc<Self>>)
    };

    let public_interface = if !component.is_global() && component.parent_element.upgrade().is_none()
    {
        let public_component_id = public_component_id(component);

        let parent_name =
            if !parent_component_type.is_empty() { Some(quote!(parent)) } else { None };
        let window_parent_name = window_parent_param.as_ref().map(|_| quote!(, parent_window));

        let run_fun = if component.parent_element.upgrade().is_none() {
            Some(quote!(
                pub fn run(&self) {
                    self.show();
                    sixtyfps::run_event_loop();
                    self.hide();
                }

                pub fn show(&self) {
                    vtable::VRc::as_pin_ref(&self.0).window.show();
                }

                pub fn hide(&self) {
                    vtable::VRc::as_pin_ref(&self.0).window.hide();
                }
            ))
        } else {
            None
        };

        Some(quote!(
            #[derive(Clone)]
            #visibility struct #public_component_id(vtable::VRc<sixtyfps::re_exports::ComponentVTable, #inner_component_id>);

            impl #public_component_id {
                pub fn new(#(parent: sixtyfps::re_exports::VWeak::<sixtyfps::re_exports::ComponentVTable, #parent_component_type>)* #window_parent_param) -> Self {
                    Self(#inner_component_id::new(#parent_name #window_parent_name))
                }
                #(#property_and_callback_accessors)*

                #run_fun
            }

            impl sixtyfps::IntoWeak for #public_component_id {
                type Inner = #inner_component_id;
                fn as_weak(&self) -> sixtyfps::Weak<Self> {
                    sixtyfps::Weak::new(&self.0)
                }

                fn from_inner(inner: vtable::VRc<sixtyfps::re_exports::ComponentVTable, #inner_component_id>) -> Self {
                    Self(inner)
                }
            }

            impl From<#public_component_id> for vtable::VRc<sixtyfps::re_exports::ComponentVTable, #inner_component_id> {
                fn from(value: #public_component_id) -> Self {
                    value.0
                }
            }
        ))
    } else {
        None
    };

    Some(quote!(
        #(#resource_symbols)*

        #[derive(sixtyfps::re_exports::FieldOffsets)]
        #[const_field_offset(sixtyfps::re_exports::const_field_offset)]
        #[repr(C)]
        #pin
        #visibility struct #inner_component_id {
            #(#item_names : sixtyfps::re_exports::#item_types,)*
            #(#declared_property_vars : sixtyfps::re_exports::Property<#declared_property_types>,)*
            #(#declared_callbacks : sixtyfps::re_exports::Callback<(#(#declared_callbacks_types,)*), #declared_callbacks_ret>,)*
            #(#repeated_element_names : sixtyfps::re_exports::Repeater<#repeated_element_components>,)*
            #(#self_weak : sixtyfps::re_exports::OnceCell<sixtyfps::re_exports::VWeak<sixtyfps::re_exports::ComponentVTable, #inner_component_id>>,)*
            #(parent : sixtyfps::re_exports::VWeak<sixtyfps::re_exports::ComponentVTable, #parent_component_type>,)*
            #(#global_name : ::core::pin::Pin<::std::rc::Rc<#global_type>>,)*
            #window_field
        }

        #component_impl

        impl #inner_component_id{
            pub fn new(#(parent: sixtyfps::re_exports::VWeak::<sixtyfps::re_exports::ComponentVTable, #parent_component_type>)* #window_parent_param)
                -> #component_handle
            {
                #![allow(unused)]
                use sixtyfps::re_exports::*;
                let mut self_ = Self {
                    #(#item_names : ::core::default::Default::default(),)*
                    #(#declared_property_vars : ::core::default::Default::default(),)*
                    #(#declared_callbacks : ::core::default::Default::default(),)*
                    #(#repeated_element_names : ::core::default::Default::default(),)*
                    #(#self_weak : ::core::default::Default::default(),)*
                    #(parent : parent as sixtyfps::re_exports::VWeak::<sixtyfps::re_exports::ComponentVTable, #parent_component_type>,)*
                    #(#global_name : #global_type::new(),)*
                    #window_field_init
                };
                #new_code
                #(#init)*
                self_pinned.into()
            }
            #item_tree_impl

        }

        #public_interface

        #drop_impl

        #has_window_impl

        #(#extra_components)*
    ))
}

/// Return an identifier suitable for this component for internal use
fn inner_component_id(component: &Component) -> proc_macro2::Ident {
    if component.is_global()
        && matches!(&component.root_element.borrow().base_type, Type::Builtin(_))
    {
        public_component_id(component)
    } else {
        format_ident!("Inner{}", public_component_id(component))
    }
}

/// Return an identifier suitable for this component for the developer facing API
fn public_component_id(component: &Component) -> proc_macro2::Ident {
    if component.id.is_empty() {
        let s = &component.root_element.borrow().id;
        // Capitalize first leter:
        let mut it = s.chars();
        let id =
            it.next().map(|c| c.to_ascii_uppercase()).into_iter().chain(it).collect::<String>();
        format_ident!("{}", id)
    } else {
        format_ident!("{}", component.id)
    }
}

fn property_animation_tokens(
    component: &Rc<Component>,
    animation: &ElementRc,
) -> Option<TokenStream> {
    let animation = animation.borrow();
    let bindings = animation.bindings.iter().map(|(prop, initializer)| {
        let prop_ident = format_ident!("{}", prop);
        let initializer = compile_expression(initializer, component);
        quote!(#prop_ident: #initializer as _)
    });

    Some(quote!(sixtyfps::re_exports::PropertyAnimation{
        #(#bindings, )*
        ..::core::default::Default::default()
    }))
}

fn property_set_value_tokens(
    component: &Rc<Component>,
    element: &ElementRc,
    property_name: &str,
    value_tokens: TokenStream,
) -> TokenStream {
    match element.borrow().property_animations.get(property_name) {
        Some(crate::object_tree::PropertyAnimation::Static(animation)) => {
            let animation_tokens = property_animation_tokens(component, animation);
            quote!(set_animated_value(#value_tokens, #animation_tokens))
        }
        _ => quote!(set(#value_tokens)),
    }
}

/// Returns the code that can access the given property or callback (but without the set or get)
///
/// to be used like:
/// ```ignore
/// let access = access_member(...)
/// quote!(#access.get())
/// ```
fn access_member(
    element: &ElementRc,
    name: &str,
    component: &Rc<Component>,
    component_rust: TokenStream,
    is_special: bool,
) -> TokenStream {
    let e = element.borrow();

    let enclosing_component = e.enclosing_component.upgrade().unwrap();
    if Rc::ptr_eq(component, &enclosing_component) {
        let inner_component_id = inner_component_id(&enclosing_component);
        let name_ident = format_ident!("{}", name);
        if e.property_declarations.contains_key(name) || is_special || component.is_global() {
            quote!(#inner_component_id::FIELD_OFFSETS.#name_ident.apply_pin(#component_rust))
        } else if let Some(vp) = super::as_flickable_viewport_property(element, name) {
            let name_ident = format_ident!("{}", vp);
            let elem_ident = format_ident!("{}", e.id);

            quote!((#inner_component_id::FIELD_OFFSETS.#elem_ident
                + sixtyfps::re_exports::Flickable::FIELD_OFFSETS.viewport
                + sixtyfps::re_exports::Rectangle::FIELD_OFFSETS.#name_ident)
                    .apply_pin(#component_rust)
            )
        } else {
            let elem_ident = format_ident!("{}", e.id);
            let elem_ty = format_ident!("{}", e.base_type.as_native().class_name);

            quote!((#inner_component_id::FIELD_OFFSETS.#elem_ident + #elem_ty::FIELD_OFFSETS.#name_ident)
                .apply_pin(#component_rust)
            )
        }
    } else if enclosing_component.is_global() {
        let mut root_component = component.clone();
        let mut component_rust = component_rust;
        while let Some(p) = root_component.parent_element.upgrade() {
            root_component = p.borrow().enclosing_component.upgrade().unwrap();
            component_rust = quote!(#component_rust.parent.upgrade().unwrap().as_pin_ref());
        }
        let global_id = format_ident!("global_{}", enclosing_component.id);
        let global_comp = quote!(#component_rust.#global_id.as_ref());
        access_member(element, name, &enclosing_component, global_comp, is_special)
    } else {
        access_member(
            element,
            name,
            &component
                .parent_element
                .upgrade()
                .unwrap()
                .borrow()
                .enclosing_component
                .upgrade()
                .unwrap(),
            quote!(#component_rust.parent.upgrade().unwrap().as_pin_ref()),
            is_special,
        )
    }
}

/// Call access_member  for a NamedReference
fn access_named_reference(
    nr: &NamedReference,
    component: &Rc<Component>,
    component_rust: TokenStream,
) -> TokenStream {
    access_member(&nr.element.upgrade().unwrap(), &nr.name, component, component_rust, false)
}

fn compile_expression(expr: &Expression, component: &Rc<Component>) -> TokenStream {
    match expr {
        Expression::StringLiteral(s) => quote!(sixtyfps::re_exports::SharedString::from(#s)),
        Expression::NumberLiteral(n, unit) => {
            let n = unit.normalize(*n);
            quote!(#n)
        }
        Expression::BoolLiteral(b) => quote!(#b),
        Expression::Cast { from, to } => {
            let f = compile_expression(&*from, &component);
            match (from.ty(), to) {
                (Type::Float32, Type::String) | (Type::Int32, Type::String) => {
                    quote!(sixtyfps::re_exports::SharedString::from(format!("{}", #f).as_str()))
                }
                (Type::Float32, Type::Model) | (Type::Int32, Type::Model) => {
                    quote!(sixtyfps::re_exports::ModelHandle::new(std::rc::Rc::<usize>::new(#f as usize)))
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
                (Type::Object { ref fields, .. }, Type::Component(c)) => {
                    let fields = fields.iter().enumerate().map(|(index, (name, _))| {
                        let index = proc_macro2::Literal::usize_unsuffixed(index);
                        let name = format_ident!("{}", name);
                        quote!(#name: obj.#index as _)
                    });
                    let id : TokenStream = c.id.parse().unwrap();
                    quote!({ let obj = #f; #id { #(#fields),*} })
                }
                (Type::Object { ref fields, .. }, Type::Object{  name: Some(n), .. }) => {
                    let fields = fields.iter().enumerate().map(|(index, (name, _))| {
                        let index = proc_macro2::Literal::usize_unsuffixed(index);
                        let name = format_ident!("{}", name);
                        quote!(#name: obj.#index as _)
                    });
                    let id : TokenStream = n.parse().unwrap();
                    quote!({ let obj = #f; #id { #(#fields),*} })
                }
                _ => f,
            }
        }
        Expression::PropertyReference(nr) => {
            let access = access_named_reference(nr, component, quote!(_self));
            quote!(#access.get())
        }
        Expression::BuiltinFunctionReference(funcref) => match funcref {
            BuiltinFunction::GetWindowScaleFactor => {
                quote!(_self.window.scale_factor)
            }
            BuiltinFunction::Debug => quote!((|x| println!("{:?}", x))),
            BuiltinFunction::Mod => quote!((|a1, a2| (a1 as i32) % (a2 as i32))),
            BuiltinFunction::Round => quote!((|a| (a as f64).round())),
            BuiltinFunction::Ceil => quote!((|a| (a as f64).ceil())),
            BuiltinFunction::Floor => quote!((|a| (a as f64).floor())),
            BuiltinFunction::Sin => quote!((|a| (a as f64).to_radians().sin())),
            BuiltinFunction::Cos => quote!((|a| (a as f64).to_radians().cos())),
            BuiltinFunction::Tan => quote!((|a| (a as f64).to_radians().tan())),
            BuiltinFunction::ASin => quote!((|a| (a as f64).asin().to_degrees())),
            BuiltinFunction::ACos => quote!((|a| (a as f64).acos().to_degrees())),
            BuiltinFunction::ATan => quote!((|a| (a as f64).atan().to_degrees())),
            BuiltinFunction::SetFocusItem | BuiltinFunction::ShowPopupWindow | BuiltinFunction::ImplicitItemSize => {
                panic!("internal error: should be handled directly in CallFunction")
            }
            BuiltinFunction::StringToFloat => {
                quote!((|x: SharedString| -> f64 { ::core::str::FromStr::from_str(x.as_str()).unwrap_or_default() } ))
            }
            BuiltinFunction::StringIsFloat => {
                quote!((|x: SharedString| { <f64 as ::core::str::FromStr>::from_str(x.as_str()).is_ok() } ))
            }
        },
        Expression::ElementReference(_) => todo!("Element references are only supported in the context of built-in function calls at the moment"),
        Expression::MemberFunction{ .. } => panic!("member function expressions must not appear in the code generator anymore"),
        Expression::BuiltinMacroReference { .. } => panic!("macro expressions must not appear in the code generator anymore"),
        Expression::RepeaterIndexReference { element } => {
            let access = access_member(
                &element.upgrade().unwrap().borrow().base_type.as_component().root_element,
                "index",
                component,
                quote!(_self),
                true,
            );
            quote!(#access.get())
        }
        Expression::RepeaterModelReference { element } => {
            let access = access_member(
                &element.upgrade().unwrap().borrow().base_type.as_component().root_element,
                "model_data",
                component,
                quote!(_self),
                true,
            );
            quote!(#access.get())
        }
        Expression::FunctionParameterReference { index, .. } => {
            let i = proc_macro2::Literal::usize_unsuffixed(*index);
            quote! {args.#i.clone()}
        }
        Expression::ObjectAccess { base, name } => match base.ty() {
            Type::Object { fields, name: None } => {
                let index = fields
                    .keys()
                    .position(|k| k == name)
                    .expect("Expression::ObjectAccess: Cannot find a key in an object");
                let index = proc_macro2::Literal::usize_unsuffixed(index);
                let base_e = compile_expression(base, component);
                quote!((#base_e).#index )
            }
            Type::Object { .. } => {
                let name = format_ident!("{}", name);
                let base_e = compile_expression(base, component);
                quote!((#base_e).#name)
            }
            _ => panic!("Expression::ObjectAccess's base expression is not an Object type"),
        },
        Expression::CodeBlock(sub) => {
            let map = sub.iter().map(|e| compile_expression(e, &component));
            quote!({ #(#map);* })
        }
        Expression::CallbackReference(nr) => access_named_reference(
            nr,
            component,
            quote!(_self),
        ),
        Expression::FunctionCall { function, arguments,  source_location: _ } => {
            match &**function {
                Expression::BuiltinFunctionReference(BuiltinFunction::SetFocusItem) => {
                    if arguments.len() != 1 {
                        panic!("internal error: incorrect argument count to SetFocusItem call");
                    }
                    if let Expression::ElementReference(focus_item) = &arguments[0] {
                        let focus_item = focus_item.upgrade().unwrap();
                        let focus_item = focus_item.borrow();
                        let item_index = focus_item.item_index.get().unwrap();
                        quote!(
                            _self.window.set_focus_item(&ItemRc::new(VRc::into_dyn(_self.self_weak.get().unwrap().upgrade().unwrap()), #item_index));
                        )
                    } else {
                        panic!("internal error: argument to SetFocusItem must be an element")
                    }
                }
                Expression::BuiltinFunctionReference(BuiltinFunction::ShowPopupWindow) => {
                    if arguments.len() != 1 {
                        panic!("internal error: incorrect argument count to ShowPopupWindow call");
                    }
                    if let Expression::ElementReference(popup_window) = &arguments[0] {
                        let popup_window = popup_window.upgrade().unwrap();
                        let pop_comp = popup_window.borrow().enclosing_component.upgrade().unwrap();
                        let popup_window_id = inner_component_id(&pop_comp);
                        let parent_component = pop_comp.parent_element.upgrade().unwrap().borrow().enclosing_component.upgrade().unwrap();
                        let popup_list = parent_component.popup_windows.borrow();
                        let popup = popup_list.iter().find(|p| Rc::ptr_eq(&p.component, &pop_comp)).unwrap();
                        let x = access_named_reference(&popup.x, component, quote!(_self));
                        let y = access_named_reference(&popup.y, component, quote!(_self));
                        quote!(
                            _self.window.show_popup(
                                &VRc::into_dyn(#popup_window_id::new(_self.self_weak.get().unwrap().clone(), &_self.window).into()),
                                Point::new(#x.get(), #y.get())
                            );
                        )
                    } else {
                        panic!("internal error: argument to SetFocusItem must be an element")
                    }
                }
                Expression::BuiltinFunctionReference(BuiltinFunction::ImplicitItemSize) => {
                    if arguments.len() != 1 {
                        panic!("internal error: incorrect argument count to ImplicitItemSize call");
                    }
                    if let Expression::ElementReference(item) = &arguments[0] {
                        let item = item.upgrade().unwrap();
                        let item = item.borrow();
                        let item_id = format_ident!("{}", item.id);
                        quote!(
                            Self::FIELD_OFFSETS.#item_id.apply_pin(_self).implicit_size(&_self.window)
                        )
                    } else {
                        panic!("internal error: argument to ImplicitItemSize must be an element")
                    }
                }
                _ => {
                    let f = compile_expression(function, &component);
                    let a = arguments.iter().map(|a| compile_expression(a, &component));
                    let function_type = function.ty();
                    if let Type::Callback { args, .. } = function_type {
                        let cast = args.iter().map(|ty| match ty {
                            Type::Bool => quote!(as bool),
                            Type::Int32 => quote!(as i32),
                            Type::Float32 => quote!(as f32),
                            _ => quote!(.clone()),
                        });
                        quote! { #f.call(&(#((#a)#cast,)*).into())}
                    } else {
                        quote! { #f(#(#a.clone()),*)}
                    }
                }
            }

        }
        Expression::SelfAssignment { lhs, rhs, op } => {
            let rhs = compile_expression(&*rhs, &component);
            compile_assignment(lhs, *op, rhs, component)
        }
        Expression::BinaryExpression { lhs, rhs, op } => {
            let (conv1, conv2) = match crate::expression_tree::operator_class(*op) {
                OperatorClass::ArithmeticOp => if lhs.ty() == Type::String {
                    (None, Some(quote!(.as_str())))
                } else {
                    (Some(quote!(as f64)), Some(quote!(as f64)))
                },
                OperatorClass::ComparisonOp
                    if matches!(
                        lhs.ty(),
                        Type::Int32
                            | Type::Float32
                            | Type::Duration
                            | Type::Length
                            | Type::LogicalLength
                            | Type::Angle
                    ) =>
                {
                    (Some(quote!(as f64)), Some(quote!(as f64)))
                }
                _ => (None, None),
            };
            let lhs = compile_expression(&*lhs, &component);
            let rhs = compile_expression(&*rhs, &component);

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
            let sub = compile_expression(&*sub, &component);
            let op = proc_macro2::Punct::new(*op, proc_macro2::Spacing::Alone);
            quote!( #op #sub )
        }
        Expression::ResourceReference(resource_ref) => {
            match resource_ref {
                crate::expression_tree::ResourceReference::None => {
                    quote!(sixtyfps::re_exports::Resource::None)
                }
                crate::expression_tree::ResourceReference::AbsolutePath(path) => {
                     quote!(sixtyfps::re_exports::Resource::AbsoluteFilePath(sixtyfps::re_exports::SharedString::from(#path)))
                },
                crate::expression_tree::ResourceReference::EmbeddedData(resource_id) => {
                    let symbol = format_ident!("SFPS_EMBEDDED_RESOURCE_{}", resource_id);
                    quote!(sixtyfps::re_exports::Resource::EmbeddedData(#symbol.into()))
                }
            }
        }
        Expression::Condition { condition, true_expr, false_expr } => {
            let condition_code = compile_expression(&*condition, component);
            let true_code = compile_expression(&*true_expr, component);
            let false_code = compile_expression(&*false_expr, component);
            quote!(
                if #condition_code {
                    #true_code
                } else {
                    (#false_code) as _
                }
            )
        }
        Expression::Invalid | Expression::Uncompiled(_) | Expression::TwoWayBinding(..) => {
            let error = format!("unsupported expression {:?}", expr);
            quote!(compile_error! {#error})
        }
        Expression::Array { values, element_ty } => {
            let rust_element_ty = rust_type(&element_ty, &Default::default()).unwrap();
            let val = values.iter().map(|e| compile_expression(e, component));
            quote!(sixtyfps::re_exports::ModelHandle::new(
                std::rc::Rc::new(sixtyfps::re_exports::VecModel::<#rust_element_ty>::from(vec![#(#val as _),*]))
            ))
        }
        Expression::Object { ty, values } => {
            if let Type::Object { fields, name } = ty {
                let elem = fields.iter().map(|(k, t)| {
                    values.get(k).map(|e| {
                        let ce = compile_expression(e, component);
                        let t = rust_type(t, &Default::default()).unwrap_or_default();
                        quote!(#ce as #t)
                    })
                });
                if let Some(name) = name {
                    let name : TokenStream = name.parse().unwrap();
                    let keys = fields.keys().map(|k| k.parse::<TokenStream>().unwrap());
                    quote!(#name { #(#keys: #elem,)* })
                } else {
                    // This will produce a tuple
                    quote!((#(#elem,)*))
                }
            } else {
                panic!("Expression::Object is not a Type::Object")
            }
        }
        Expression::PathElements { elements } => compile_path(elements, component),
        Expression::StoreLocalVariable { name, value } => {
            let value = compile_expression(value, component);
            let name = format_ident!("{}", name);
            quote!(let #name = #value;)
        }
        Expression::ReadLocalVariable { name, .. } => {
            let name = format_ident!("{}", name);
            quote!(#name)
        }
        Expression::EasingCurve(EasingCurve::Linear) => {
            quote!(sixtyfps::re_exports::EasingCurve::Linear)
        }
        Expression::EasingCurve(EasingCurve::CubicBezier(a, b, c, d)) => {
            quote!(sixtyfps::re_exports::EasingCurve::CubicBezier([#a, #b, #c, #d]))
        }
        Expression::LinearGradient{angle, stops} => {
            let angle = compile_expression(angle, component);
            let stops = stops.iter().map(|(color, stop)| {
                let color = compile_expression(color, component);
                let position = compile_expression(stop, component);
                quote!(sixtyfps::re_exports::GradientStop{ color: #color, position: #position as _ })
            });
            quote!(sixtyfps::Brush::LinearGradient(
                sixtyfps::re_exports::LinearGradientBrush::new(#angle as _, [#(#stops),*].iter().cloned())
            ))
        }
        Expression::EnumerationValue(value) => {
            let base_ident = format_ident!("{}", value.enumeration.name);
            let value_ident = format_ident!("{}", value.to_string());
            quote!(sixtyfps::re_exports::#base_ident::#value_ident)
        }
        Expression::ReturnStatement(expr) => {
            let return_expr = expr.as_ref().map(|expr| compile_expression(expr, component));
            quote!(return (#return_expr) as _;)
        },
    }
}

fn compile_assignment(
    lhs: &Expression,
    op: char,
    rhs: TokenStream,
    component: &Rc<Component>,
) -> TokenStream {
    match lhs {
        Expression::PropertyReference(nr) => {
            let lhs_ = access_named_reference(nr, component, quote!(_self));
            if op == '=' {
                quote!( #lhs_.set((#rhs) as _) )
            } else {
                let op = proc_macro2::Punct::new(op, proc_macro2::Spacing::Alone);
                if lhs.ty() == Type::String {
                    quote!( #lhs_.set(#lhs_.get() #op #rhs.as_str()) )
                } else {
                    quote!( #lhs_.set(((#lhs_.get() as f64) #op (#rhs as f64)) as _) )
                }
            }
        }
        Expression::ObjectAccess { base, name } => {
            let tmpobj = quote!(tmpobj);
            let get_obj = compile_expression(base, component);
            let ty = base.ty();
            let (member, member_ty) = match &ty {
                Type::Object { fields, name: None } => {
                    let index = fields
                        .keys()
                        .position(|k| k == name)
                        .expect("Expression::ObjectAccess: Cannot find a key in an object");
                    let index = proc_macro2::Literal::usize_unsuffixed(index);
                    (quote!(#index), fields[name].clone())
                }
                Type::Object { fields, name: Some(_) } => {
                    let n = format_ident!("{}", name);
                    (quote!(#n), fields[name].clone())
                }
                _ => panic!("Expression::ObjectAccess's base expression is not an Object type"),
            };

            let conv = if member_ty == Type::String {
                if op == '=' {
                    quote!()
                } else {
                    quote!(.as_str())
                }
            } else {
                let member_ty = rust_type(&member_ty, &Default::default()).unwrap_or_default();
                quote!(as #member_ty)
            };

            let op = match op {
                '+' => quote!(+=),
                '*' => quote!(*=),
                '-' => quote!(-=),
                '/' => quote!(/=),
                '=' => quote!(=),
                _ => panic!("Unkown assignment op {:?}", op),
            };

            let new_value = quote!({
               let mut #tmpobj = #get_obj;
               #tmpobj.#member #op (#rhs #conv);
               #tmpobj
            });
            compile_assignment(base, '=', new_value, component)
        }
        Expression::RepeaterModelReference { element } => {
            let element = element.upgrade().unwrap();
            let parent_component = element.borrow().base_type.as_component().clone();
            let repeater_access = access_member(
                &parent_component
                    .parent_element
                    .upgrade()
                    .unwrap()
                    .borrow()
                    .enclosing_component
                    .upgrade()
                    .unwrap()
                    .root_element,
                &format!("repeater_{}", element.borrow().id),
                component,
                quote!(_self),
                true,
            );
            let index_access = access_member(
                &parent_component.root_element,
                "index",
                component,
                quote!(_self),
                true,
            );
            if op == '=' {
                quote!(#repeater_access.model_set_row_data(#index_access.get(), #rhs as _))
            } else {
                let op = proc_macro2::Punct::new(op, proc_macro2::Spacing::Alone);
                let old_data = compile_expression(lhs, component);
                if lhs.ty() == Type::String {
                    quote!(#repeater_access.model_set_row_data(#index_access.get(), #old_data #op &#rhs))
                } else {
                    quote!(#repeater_access.model_set_row_data(#index_access.get(), ((#old_data as f64) #op (#rhs as f64)) as _))
                }
            }
        }
        _ => panic!("typechecking should make sure this was a PropertyReference"),
    }
}

struct RustLanguageLayoutGen;
impl crate::layout::gen::Language for RustLanguageLayoutGen {
    type CompiledCode = TokenStream;

    fn make_grid_layout_cell_data<'a, 'b>(
        item: &'a crate::layout::LayoutItem,
        col: u16,
        row: u16,
        colspan: u16,
        rowspan: u16,
        layout_tree: &'b mut Vec<LayoutTreeItem<'a>>,
        component: &Rc<Component>,
    ) -> TokenStream {
        let get_property_ref = |p: &Option<NamedReference>| match p {
            Some(nr) => {
                let p = access_named_reference(nr, component, quote!(_self));
                quote!(Some(#p.get_ref()))
            }
            None => quote!(None),
        };
        let lay_rect = item.rect();
        let width = get_property_ref(&lay_rect.width_reference);
        let height = get_property_ref(&lay_rect.height_reference);
        let x = get_property_ref(&lay_rect.x_reference);
        let y = get_property_ref(&lay_rect.y_reference);
        let layout_info = get_layout_info_ref(item, layout_tree, component);
        quote!(GridLayoutCellData {
            x: #x,
            y: #y,
            width: #width,
            height: #height,
            col: #col,
            row: #row,
            colspan: #colspan,
            rowspan: #rowspan,
            constraint: #layout_info,
        })
    }

    fn grid_layout_tree_item<'a, 'b>(
        layout_tree: &'b mut Vec<crate::layout::gen::LayoutTreeItem<'a, Self>>,
        geometry: &'a crate::layout::LayoutGeometry,
        cells: Vec<Self::CompiledCode>,
        component: &Rc<Component>,
    ) -> crate::layout::gen::LayoutTreeItem<'a, Self> {
        let cell_ref_variable = format_ident!("cells_{}", layout_tree.len());
        let cell_creation_code = quote!(let #cell_ref_variable
                = [#( #cells ),*];);
        let (padding, spacing, spacing_creation_code) =
            generate_layout_padding_and_spacing(&layout_tree, geometry, component);

        LayoutTreeItem::GridLayout {
            geometry,
            var_creation_code: quote!(#cell_creation_code #spacing_creation_code),
            cell_ref_variable: quote!(#cell_ref_variable),
            spacing,
            padding,
        }
    }

    fn box_layout_tree_item<'a, 'b>(
        layout_tree: &'b mut Vec<crate::layout::gen::LayoutTreeItem<'a, Self>>,
        box_layout: &'a crate::layout::BoxLayout,
        component: &Rc<Component>,
    ) -> crate::layout::gen::LayoutTreeItem<'a, Self> {
        let is_static_array = box_layout
            .elems
            .iter()
            .all(|i| i.element.as_ref().map_or(true, |x| x.borrow().repeated.is_none()));

        let mut make_box_layout_cell_data = |cell: &'a crate::layout::LayoutItem| {
            let get_property_ref = |p: &Option<NamedReference>| match p {
                Some(nr) => {
                    let p = access_named_reference(nr, component, quote!(_self));
                    quote!(Some(#p.get_ref()))
                }
                None => quote!(None),
            };
            let lay_rect = cell.rect();
            let width = get_property_ref(&lay_rect.width_reference);
            let height = get_property_ref(&lay_rect.height_reference);
            let x = get_property_ref(&lay_rect.x_reference);
            let y = get_property_ref(&lay_rect.y_reference);
            let layout_info = get_layout_info_ref(cell, layout_tree, component);
            quote!(BoxLayoutCellData {
                x: #x,
                y: #y,
                width: #width,
                height: #height,
                constraint: #layout_info,
            })
        };
        let cell_creation_code = if is_static_array {
            let cells: Vec<_> = box_layout.elems.iter().map(make_box_layout_cell_data).collect();
            let cell_ref_variable = format_ident!("cells_{}", layout_tree.len());
            quote!(let #cell_ref_variable = [#( #cells ),*];)
        } else {
            let mut fixed_count = 0usize;
            let mut repeated_count = quote!();
            let mut push_code = quote!();
            let inner_component_id = inner_component_id(component);
            for item in &box_layout.elems {
                match &item.element {
                    Some(elem) if elem.borrow().repeated.is_some() => {
                        let repeater_id = format_ident!("repeater_{}", elem.borrow().id);
                        let rep_inner_component_id =
                            self::inner_component_id(&elem.borrow().base_type.as_component());
                        repeated_count = quote!(#repeated_count + self.#repeater_id.len());
                        push_code = quote! {
                            #push_code
                            #inner_component_id::FIELD_OFFSETS.#repeater_id.apply_pin(self).ensure_updated(
                                || { #rep_inner_component_id::new(self.self_weak.get().unwrap().clone(), &self.window).into() }
                            );
                            let internal_vec = self.#repeater_id.components_vec();
                            for sub_comp in &internal_vec {
                                items_vec.push(sub_comp.as_pin_ref().box_layout_data())
                            }
                        }
                    }
                    _ => {
                        let e = make_box_layout_cell_data(item);
                        fixed_count += 1;
                        push_code = quote! {
                            #push_code
                            items_vec.push(#e);
                        }
                    }
                }
            }
            let cell_ref_variable = format_ident!("cells_{}", layout_tree.len());
            quote! {
                let mut items_vec = Vec::with_capacity(#fixed_count #repeated_count);
                #push_code
                let #cell_ref_variable = items_vec;
            }
        };
        let cell_ref_variable = format_ident!("cells_{}", layout_tree.len());

        let (padding, spacing, spacing_creation_code) =
            generate_layout_padding_and_spacing(&layout_tree, &box_layout.geometry, component);

        let alignment = if let Some(expr) = &box_layout.geometry.alignment {
            let p = access_named_reference(expr, component, quote!(_self));
            quote!(#p.get())
        } else {
            quote!(::core::default::Default::default())
        };

        LayoutTreeItem::BoxLayout {
            is_horizontal: box_layout.is_horizontal,
            geometry: &box_layout.geometry,
            var_creation_code: quote!(#cell_creation_code #spacing_creation_code),
            cell_ref_variable: quote!(#cell_ref_variable),
            spacing,
            padding,
            alignment,
        }
    }
}

type LayoutTreeItem<'a> = crate::layout::gen::LayoutTreeItem<'a, RustLanguageLayoutGen>;

impl<'a> LayoutTreeItem<'a> {
    fn layout_info(&self) -> TokenStream {
        match self {
            LayoutTreeItem::GridLayout { cell_ref_variable, spacing, padding, .. } => {
                quote!(grid_layout_info(&Slice::from_slice(&#cell_ref_variable), #spacing, #padding))
            }
            LayoutTreeItem::BoxLayout {
                cell_ref_variable,
                spacing,
                padding,
                alignment,
                is_horizontal,
                ..
            } => {
                quote!(box_layout_info(&Slice::from_slice(&#cell_ref_variable), #spacing, #padding, #alignment, #is_horizontal))
            }
            LayoutTreeItem::PathLayout(_) => quote!(todo!("layout_info for PathLayout in rust.rs")),
        }
    }
}

fn get_layout_info_ref<'a, 'b>(
    item: &'a crate::layout::LayoutItem,
    layout_tree: &'b mut Vec<LayoutTreeItem<'a>>,
    component: &Rc<Component>,
) -> TokenStream {
    let layout_info = item.layout.as_ref().map(|l| {
        crate::layout::gen::collect_layouts_recursively(layout_tree, l, component).layout_info()
    });
    let elem_info = item.element.as_ref().map(|elem| {
        let e = format_ident!("{}", elem.borrow().id);
        quote!(Self::FIELD_OFFSETS.#e.apply_pin(self).layouting_info(&window))
    });
    let layout_info = match (layout_info, elem_info) {
        (None, None) => quote!(),
        (None, Some(x)) => x,
        (Some(x), None) => x,
        (Some(layout_info), Some(elem_info)) => quote!(#layout_info.merge(&#elem_info)),
    };
    apply_layout_constraint(layout_info, &item.constraints, component)
}

fn apply_layout_constraint(
    layout_info: TokenStream,
    constraints: &crate::layout::LayoutConstraints,
    component: &Rc<Component>,
) -> TokenStream {
    if constraints.has_explicit_restrictions() {
        let (name, expr): (Vec<_>, Vec<_>) = constraints
            .for_each_restrictions()
            .map(|(e, s)| {
                (format_ident!("{}", s), access_named_reference(e, component, quote!(_self)))
            })
            .unzip();
        quote!({
            let mut layout_info = #layout_info;
                #(layout_info.#name = #expr.get();)*
            layout_info
        })
    } else {
        layout_info
    }
}

fn generate_layout_padding_and_spacing<'a, 'b>(
    layout_tree: &'b [LayoutTreeItem<'a>],
    layout_geometry: &'a LayoutGeometry,
    component: &Rc<Component>,
) -> (TokenStream, TokenStream, Option<TokenStream>) {
    let (spacing, spacing_creation_code) = if let Some(spacing) = &layout_geometry.spacing {
        let variable = format_ident!("spacing_{}", layout_tree.len());
        let spacing_code = access_named_reference(spacing, component, quote!(_self));
        (quote!(#variable), Some(quote!(let #variable = #spacing_code.get();)))
    } else {
        (quote!(0.), None)
    };
    let padding = {
        let padding_prop = |expr| {
            if let Some(expr) = expr {
                let p = access_named_reference(expr, component, quote!(_self));
                quote!(#p.get())
            } else {
                quote!(0.)
            }
        };
        let left = padding_prop(layout_geometry.padding.left.as_ref());
        let right = padding_prop(layout_geometry.padding.right.as_ref());
        let top = padding_prop(layout_geometry.padding.top.as_ref());
        let bottom = padding_prop(layout_geometry.padding.bottom.as_ref());
        quote!(&sixtyfps::re_exports::Padding {
            left: #left,
            right: #right,
            top: #top,
            bottom: #bottom,
        })
    };

    (padding, spacing, spacing_creation_code)
}

impl<'a> LayoutTreeItem<'a> {
    fn emit_solve_calls(&self, component: &Rc<Component>, code_stream: &mut Vec<TokenStream>) {
        let layout_prop = |p: &Option<NamedReference>| {
            if let Some(nr) = p {
                let p = access_named_reference(nr, component, quote!(_self));
                quote!(#p.get())
            } else {
                quote!(::core::default::Default::default())
            }
        };
        match self {
            LayoutTreeItem::GridLayout {
                geometry, cell_ref_variable, spacing, padding, ..
            } => {
                let x_pos = layout_prop(&geometry.rect.x_reference);
                let y_pos = layout_prop(&geometry.rect.y_reference);
                let width = layout_prop(&geometry.rect.width_reference);
                let height = layout_prop(&geometry.rect.height_reference);

                code_stream.push(quote! {
                    solve_grid_layout(&GridLayoutData {
                        width: #width,
                        height: #height,
                        x: #x_pos,
                        y: #y_pos,
                        cells: Slice::from_slice(&#cell_ref_variable),
                        spacing: #spacing,
                        padding: #padding,
                    });
                });
            }
            LayoutTreeItem::BoxLayout {
                geometry,
                cell_ref_variable,
                spacing,
                padding,
                alignment,
                is_horizontal,
                ..
            } => {
                let x_pos = layout_prop(&geometry.rect.x_reference);
                let y_pos = layout_prop(&geometry.rect.y_reference);
                let width = layout_prop(&geometry.rect.width_reference);
                let height = layout_prop(&geometry.rect.height_reference);

                code_stream.push(quote! {
                    solve_box_layout(&BoxLayoutData {
                        width: #width,
                        height: #height,
                        x: #x_pos,
                        y: #y_pos,
                        cells: Slice::from_slice(&#cell_ref_variable),
                        spacing: #spacing,
                        padding: #padding,
                        alignment: #alignment
                    }, #is_horizontal);
                });
            }
            LayoutTreeItem::PathLayout(path_layout) => {
                let path_layout_item_data =
                    |elem: &ElementRc, elem_rs: TokenStream, component_rust: TokenStream| {
                        let prop_ref = |n: &str| {
                            if elem.borrow().lookup_property(n).property_type == Type::Length {
                                let n = format_ident!("{}", n);
                                quote! {Some(& #elem_rs.#n)}
                            } else {
                                quote! {None}
                            }
                        };
                        let prop_value = |n: &str| {
                            if elem.borrow().lookup_property(n).property_type == Type::Length {
                                let accessor = access_member(
                                    &elem,
                                    n,
                                    &elem.borrow().enclosing_component.upgrade().unwrap(),
                                    component_rust.clone(),
                                    false,
                                );
                                quote!(#accessor.get())
                            } else {
                                quote! {0.}
                            }
                        };
                        let x = prop_ref("x");
                        let y = prop_ref("y");
                        let width = prop_value("width");
                        let height = prop_value("height");
                        quote!(PathLayoutItemData {
                            x: #x,
                            y: #y,
                            width: #width,
                            height: #height,
                        })
                    };
                let path_layout_item_data_for_elem = |elem: &ElementRc| {
                    let e = format_ident!("{}", elem.borrow().id);
                    path_layout_item_data(elem, quote!(self.#e), quote!(self))
                };

                let is_static_array =
                    path_layout.elements.iter().all(|elem| elem.borrow().repeated.is_none());

                let slice = if is_static_array {
                    let items = path_layout.elements.iter().map(path_layout_item_data_for_elem);
                    quote!( Slice::from_slice(&[#( #items ),*]) )
                } else {
                    let mut fixed_count = 0usize;
                    let mut repeated_count = quote!();
                    let mut push_code = quote!();
                    for elem in &path_layout.elements {
                        if elem.borrow().repeated.is_some() {
                            let repeater_id = format_ident!("repeater_{}", elem.borrow().id);
                            repeated_count = quote!(#repeated_count + self.#repeater_id.len());
                            let root_element =
                                elem.borrow().base_type.as_component().root_element.clone();
                            let root_id = format_ident!("{}", root_element.borrow().id);
                            let e = path_layout_item_data(
                                &root_element,
                                quote!(sub_comp.#root_id),
                                quote!(sub_comp.as_pin_ref()),
                            );
                            push_code = quote! {
                                #push_code
                                let internal_vec = self.#repeater_id.components_vec();
                                for sub_comp in &internal_vec {
                                    items_vec.push(#e)
                                }
                            }
                        } else {
                            fixed_count += 1;
                            let e = path_layout_item_data_for_elem(elem);
                            push_code = quote! {
                                #push_code
                                items_vec.push(#e);
                            }
                        }
                    }

                    code_stream.push(quote! {
                        let mut items_vec = Vec::with_capacity(#fixed_count #repeated_count);
                        #push_code
                    });
                    quote!(Slice::from_slice(items_vec.as_slice()))
                };

                let path = compile_path(&path_layout.path, &component);

                let x_pos = layout_prop(&path_layout.rect.x_reference);
                let y_pos = layout_prop(&path_layout.rect.y_reference);
                let width = layout_prop(&path_layout.rect.width_reference);
                let height = layout_prop(&path_layout.rect.width_reference);
                let offset = layout_prop(&Some(path_layout.offset_reference.clone()));

                code_stream.push(quote! {
                    solve_path_layout(&PathLayoutData {
                        items: #slice,
                        elements: &#path,
                        x: #x_pos,
                        y: #y_pos,
                        width: #width,
                        height: #height,
                        offset: #offset,
                    });
                });
            }
        }
    }
}

fn compute_layout(
    component: &Rc<Component>,
    repeated_element_layouts: &[TokenStream],
) -> TokenStream {
    let mut layouts = vec![];
    let root_id = format_ident!("{}", component.root_element.borrow().id);
    let inner_component_id = inner_component_id(component);
    let mut layout_info =
        quote!(#inner_component_id::FIELD_OFFSETS.#root_id.apply_pin(self).layouting_info(&window));
    let component_layouts = component.layouts.borrow();

    component_layouts.iter().enumerate().for_each(|(idx, layout)| {
        let mut inverse_layout_tree = Vec::new();

        let layout_item = crate::layout::gen::collect_layouts_recursively(
            &mut inverse_layout_tree,
            layout,
            component,
        );

        if component_layouts.main_layout == Some(idx) {
            layout_info = layout_item.layout_info()
        }

        let mut creation_code = inverse_layout_tree
            .iter()
            .filter_map(|layout| match layout {
                LayoutTreeItem::GridLayout { var_creation_code, .. } => {
                    Some(var_creation_code.clone())
                }
                LayoutTreeItem::BoxLayout { var_creation_code, .. } => {
                    Some(var_creation_code.clone())
                }
                LayoutTreeItem::PathLayout(_) => None,
            })
            .collect::<Vec<_>>();

        if component_layouts.main_layout == Some(idx) {
            layout_info = quote!({#(#creation_code)* #layout_info});
        }

        layouts.append(&mut creation_code);

        inverse_layout_tree
            .iter()
            .rev()
            .for_each(|layout| layout.emit_solve_calls(component, &mut layouts));
    });

    layout_info = apply_layout_constraint(
        layout_info,
        &component.layouts.borrow().root_constraints,
        component,
    );

    quote! {
        fn layout_info(self: ::core::pin::Pin<&Self>) -> sixtyfps::re_exports::LayoutInfo {
            #![allow(unused)]
            use sixtyfps::re_exports::*;
            let _self = self;
            let window = _self.window.clone();
            #layout_info
        }
        fn apply_layout(self: ::core::pin::Pin<&Self>, _: sixtyfps::re_exports::Rect) {
            #![allow(unused)]
            use sixtyfps::re_exports::*;
            let dummy = Property::<f32>::default();
            let _self = self;
            let window = _self.window.clone();
            #(#layouts)*

            let self_pinned = self;
            let _self = self;
            #(#repeated_element_layouts)*
        }
    }
}

fn compile_path_events(events: &crate::expression_tree::PathEvents) -> TokenStream {
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
                            let prop_ident = format_ident!("{}", property);
                            let binding_expr = compile_expression(expr, component);

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
