/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
/*! module for the Rust code generator
*/

use crate::diagnostics::{BuildDiagnostics, CompilerDiagnostic, Level, Spanned};
use crate::expression_tree::{
    BuiltinFunction, EasingCurve, Expression, NamedReference, OperatorClass, Path,
};
use crate::layout::{gen::LayoutItemCodeGen, Layout, LayoutElement};
use crate::object_tree::{Component, ElementRc};
use crate::typeregister::Type;
use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use std::rc::Rc;

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
        Type::Length => Ok(quote!(f32)),
        Type::LogicalLength => Ok(quote!(f32)),
        Type::Bool => Ok(quote!(bool)),
        Type::Resource => Ok(quote!(sixtyfps::re_exports::Resource)),
        Type::Object(o) => {
            let elem = o.values().map(|v| rust_type(v, span)).collect::<Result<Vec<_>, _>>()?;
            // This will produce a tuple
            Ok(quote!((#(#elem,)*)))
        }
        Type::Array(o) => {
            let inner = rust_type(&o, span)?;
            Ok(quote!(sixtyfps::re_exports::ModelRc<#inner>))
        }
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
pub fn generate(component: &Rc<Component>, diag: &mut BuildDiagnostics) -> Option<TokenStream> {
    let compo = generate_component(component, diag)?;
    let compo_id = component_id(component);
    let compo_module = format_ident!("sixtyfps_generated_{}", compo_id);
    /*let (version_major, version_minor, version_patch): (usize, usize, usize) = (
        env!("CARGO_PKG_VERSION_MAJOR").parse().unwrap(),
        env!("CARGO_PKG_VERSION_MINOR").parse().unwrap(),
        env!("CARGO_PKG_VERSION_PATCH").parse().unwrap(),
    );*/
    let version_check = format_ident!(
        "VersionCheck_{}_{}_{}",
        env!("CARGO_PKG_VERSION_MAJOR"),
        env!("CARGO_PKG_VERSION_MINOR"),
        env!("CARGO_PKG_VERSION_PATCH"),
    );
    Some(quote! {
        #[allow(non_snake_case)]
        mod #compo_module {
             #compo
             const _THE_SAME_VERSION_MUST_BE_USED_FOR_THE_COMPILER_AND_THE_RUNTIME : sixtyfps::#version_check = sixtyfps::#version_check;
        }
        pub use #compo_module::#compo_id;
    })
}

/// Generate the rust code for the given component.
///
/// Fill the diagnostic in case of error.
fn generate_component(
    component: &Rc<Component>,
    diag: &mut BuildDiagnostics,
) -> Option<TokenStream> {
    let mut extra_components = vec![];
    let mut declared_property_vars = vec![];
    let mut declared_property_types = vec![];
    let mut declared_signals = vec![];
    let mut declared_signals_types = vec![];
    let mut property_and_signal_accessors: Vec<TokenStream> = vec![];
    for (prop_name, property_decl) in component.root_element.borrow().property_declarations.iter() {
        let prop_ident = format_ident!("{}", prop_name);
        if let Type::Signal { args } = &property_decl.property_type {
            declared_signals.push(prop_ident.clone());
            let signal_args = args
                .iter()
                .map(|a| rust_type(a, &property_decl.type_node.span()))
                .collect::<Result<Vec<_>, _>>()
                .unwrap_or_else(|err| {
                    diag.push_internal_error(err.into());
                    vec![]
                });

            if property_decl.expose_in_public_api {
                let args_name =
                    (0..signal_args.len()).map(|i| format_ident!("arg_{}", i)).collect::<Vec<_>>();
                let emitter_ident = format_ident!("emit_{}", prop_name);
                property_and_signal_accessors.push(
                    quote!(
                        #[allow(dead_code)]
                        pub fn #emitter_ident(self: ::core::pin::Pin<&Self>, #(#args_name : #signal_args,)*) {
                            Self::FIELD_OFFSETS.#prop_ident.apply_pin(self).emit(&(#(#args_name,)*))
                        }
                    )
                    .into(),
                );
                let on_ident = format_ident!("on_{}", prop_name);
                let args_index = (0..signal_args.len()).map(proc_macro2::Literal::usize_unsuffixed);
                property_and_signal_accessors.push(
                    quote!(
                        #[allow(dead_code)]
                        pub fn #on_ident(self: ::core::pin::Pin<&Self>, f: impl Fn(#(#signal_args),*) + 'static) {
                            #[allow(unused)]
                            Self::FIELD_OFFSETS.#prop_ident.apply_pin(self).set_handler(
                                // FIXME: why do i need to clone here?
                                move |args| f(#(args.#args_index.clone()),*)
                            )
                        }
                    )
                    .into(),
                );
            }
            declared_signals_types.push(signal_args);
        } else {
            declared_property_vars.push(prop_ident.clone());
            let rust_property_type =
                rust_type(&property_decl.property_type, &property_decl.type_node.span())
                    .unwrap_or_else(|err| {
                        diag.push_internal_error(err.into());
                        quote!().into()
                    });
            declared_property_types.push(rust_property_type.clone());

            if property_decl.expose_in_public_api {
                let getter_ident = format_ident!("get_{}", prop_name);
                let setter_ident = format_ident!("set_{}", prop_name);

                property_and_signal_accessors.push(
                    quote!(
                        #[allow(dead_code)]
                        pub fn #getter_ident(self: ::core::pin::Pin<&Self>) -> #rust_property_type {
                            Self::FIELD_OFFSETS.#prop_ident.apply_pin(self).get()
                        }
                    )
                    .into(),
                );

                let set_value = property_set_value_tokens(
                    component,
                    &component.root_element,
                    prop_name,
                    quote!(value),
                );
                property_and_signal_accessors.push(
                    quote!(
                        #[allow(dead_code)]
                        pub fn #setter_ident(&self, value: #rust_property_type) {
                            Self::FIELD_OFFSETS.#prop_ident.apply(self).#set_value
                        }
                    )
                    .into(),
                );
            }
        }
    }

    if diag.has_error() {
        return None;
    }

    let component_id = component_id(component);

    let mut item_tree_array = Vec::new();
    let mut item_names = Vec::new();
    let mut item_types = Vec::new();
    let mut repeated_element_names = Vec::new();
    let mut repeated_element_components = Vec::new();
    let mut repeated_visit_branch = Vec::new();
    let mut repeated_input_branch = Vec::new();
    let mut init = Vec::new();
    let mut maybe_window_field_decl = None;
    let mut maybe_window_field_init = None;
    super::build_array_helper(component, |item_rc, children_index, is_flickable_rect| {
        let item = item_rc.borrow();
        if is_flickable_rect {
            let field_name = format_ident!("{}", item.id);
            let children_count = item.children.len() as u32;
            let children_index = item_tree_array.len() as u32 + 1;

            item_tree_array.push(quote!(
                sixtyfps::re_exports::ItemTreeNode::Item{
                    item: VOffset::new(#component_id::FIELD_OFFSETS.#field_name + sixtyfps::re_exports::Flickable::FIELD_OFFSETS.viewport),
                    chilren_count: #children_count,
                    children_index: #children_index,
                }
            ));
        } else if let Some(repeated) = &item.repeated {
            let base_component = item.base_type.as_component();
            let repeater_index = repeated_element_names.len();
            let repeater_id = format_ident!("repeater_{}", item.id);
            let rep_component_id = self::component_id(&*base_component);

            extra_components.push(generate_component(&*base_component, diag).unwrap_or_else(
                || {
                    assert!(diag.has_error());
                    Default::default()
                },
            ));
            extra_components.push(if repeated.is_conditional_element {
                quote! {
                     impl sixtyfps::re_exports::RepeatedComponent for #rep_component_id {
                        type Data = ();
                        fn update(&self, _: usize, _: Self::Data) { }
                    }
                }
            } else {
                let data_type = rust_type(
                    &Expression::RepeaterModelReference { element: Rc::downgrade(item_rc) }.ty(),
                    &item.node.as_ref().map_or_else(Default::default, |n| n.span()),
                )
                .unwrap_or_else(|err| {
                    diag.push_internal_error(err.into());
                    quote!().into()
                });

                quote! {
                    impl sixtyfps::re_exports::RepeatedComponent for #rep_component_id {
                        type Data = #data_type;
                        fn update(&self, index: usize, data: Self::Data) {
                            self.index.set(index);
                            self.model_data.set(data)
                        }
                    }
                }
            });

            let mut model = compile_expression(&repeated.model, component);
            if repeated.is_conditional_element {
                model = quote!(sixtyfps::re_exports::ModelRc(std::rc::Rc::<bool>::new(#model)))
            }

            // FIXME: there could be an optimization if `repeated.model.is_constant()`, we don't need a binding
            init.push(quote! {
                self_pinned.#repeater_id.set_model_binding({
                    let self_weak = sixtyfps::re_exports::PinWeak::downgrade(self_pinned.clone());
                    move || {
                        let self_pinned = self_weak.upgrade().unwrap();
                        let _self = self_pinned.as_ref();
                        (#model) as _
                    }
                });
            });
            repeated_visit_branch.push(quote!(
                #repeater_index => {
                    #component_id::FIELD_OFFSETS.#repeater_id.apply_pin(self_pinned).ensure_updated(
                            || { #rep_component_id::new(self_pinned.self_weak.get().unwrap().clone()) }
                        );
                    self_pinned.#repeater_id.visit(order, visitor)
                }
            ));

            repeated_input_branch.push(quote!(
                #repeater_index => self.#repeater_id.input_event(rep_index, event),
            ));

            item_tree_array.push(quote!(
                sixtyfps::re_exports::ItemTreeNode::DynamicTree {
                    index: #repeater_index,
                }
            ));

            repeated_element_names.push(repeater_id);
            repeated_element_components.push(rep_component_id);
        } else {
            let field_name = format_ident!("{}", item.id);
            let children_count =
                if super::is_flickable(item_rc) { 1 } else { item.children.len() as u32 };

            item_tree_array.push(quote!(
                sixtyfps::re_exports::ItemTreeNode::Item{
                    item: VOffset::new(#component_id::FIELD_OFFSETS.#field_name),
                    chilren_count: #children_count,
                    children_index: #children_index,
                }
            ));
            for (k, binding_expression) in &item.bindings {
                let rust_property_ident = format_ident!("{}", k);
                let rust_property_accessor_prefix = if item.property_declarations.contains_key(k) {
                    proc_macro2::TokenStream::new()
                } else {
                    quote!(#field_name.)
                };
                let rust_property = quote!(#rust_property_accessor_prefix#rust_property_ident);
                let tokens_for_expression = compile_expression(binding_expression, &component);

                if matches!(item.lookup_property(k.as_str()), Type::Signal{..}) {
                    init.push(quote!(
                        self_pinned.#rust_property.set_handler({
                            let self_weak = sixtyfps::re_exports::PinWeak::downgrade(self_pinned.clone());
                            move |args| {
                                let self_pinned = self_weak.upgrade().unwrap();
                                let _self = self_pinned.as_ref();
                                #tokens_for_expression;
                            }
                        });
                    ));
                } else {
                    let setter = if binding_expression.is_constant() {
                        quote!(set((#tokens_for_expression) as _))
                    } else {
                        property_set_binding_tokens(
                            component,
                            &item_rc,
                            k,
                            quote!({
                                let self_weak = sixtyfps::re_exports::PinWeak::downgrade(self_pinned.clone());
                                move || {
                                    let self_pinned = self_weak.upgrade().unwrap();
                                    let _self = self_pinned.as_ref();
                                    (#tokens_for_expression) as _
                                }
                            }),
                        )
                    };
                    init.push(quote!(
                        self_pinned.#rust_property.#setter;
                    ));
                }
            }
            item_names.push(field_name);
            item_types.push(format_ident!("{}", item.base_type.as_native().class_name));
        }
    });

    let resource_symbols: Vec<proc_macro2::TokenStream> = if component.embed_file_resources.get() {
        component
            .referenced_file_resources
            .borrow()
            .iter()
            .map(|(path, id)| {
                let symbol = format_ident!("SFPS_EMBEDDED_RESOURCE_{}", id);
                quote!(const #symbol: &'static [u8] = ::core::include_bytes!(#path);)
            })
            .collect()
    } else {
        Vec::new()
    };

    let layouts = compute_layout(component, &repeated_element_names);
    let mut visibility = None;
    let mut parent_component_type = None;
    if let Some(parent_element) = component.parent_element.upgrade() {
        if !parent_element.borrow().repeated.as_ref().map_or(false, |r| r.is_conditional_element) {
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
                    quote!().into()
                }),
            );
        }

        parent_component_type = Some(self::component_id(
            &parent_element.borrow().enclosing_component.upgrade().unwrap(),
        ));
    } else {
        // FIXME: This field is public for testing.
        maybe_window_field_decl = Some(quote!(pub window: sixtyfps::re_exports::ComponentWindow));
        maybe_window_field_init = Some(quote!(window: sixtyfps::create_window()));

        let root_elem = component.root_element.borrow();
        let root_item_name = format_ident!("{}", root_elem.id);
        property_and_signal_accessors.push(quote! {
            pub fn run(self : core::pin::Pin<std::rc::Rc<Self>>) {
                use sixtyfps::re_exports::*;
                let root_item = Self::FIELD_OFFSETS.#root_item_name.apply_pin(self.as_ref());
                self.as_ref().window.run(VRef::new_pin(self.as_ref()), VRef::new_pin(root_item));
            }
        });
        property_and_signal_accessors.push(quote! {
            pub fn as_weak(self: core::pin::Pin<std::rc::Rc<Self>>) -> sixtyfps::re_exports::PinWeak<Self> {
                sixtyfps::re_exports::PinWeak::downgrade(self)
            }
        });
        visibility = Some(quote!(pub));
    };

    // Trick so we can use `#()` as a `if let Some` in `quote!`
    let parent_component_type = parent_component_type.iter().collect::<Vec<_>>();

    let item_tree_array_len = item_tree_array.len();

    if diag.has_error() {
        return None;
    }

    let drop_impl = {
        let guarded_window_ref = {
            let mut root_component = component.clone();
            let mut component_rust = quote!(self);
            while let Some(p) = root_component.parent_element.upgrade() {
                root_component = p.borrow().enclosing_component.upgrade().unwrap();
                component_rust = quote!(if let Some(parent) = #component_rust.parent.upgrade() {
                    parent
                } else {
                    return;
                }.as_ref());
            }
            quote!(#component_rust.as_ref().window)
        };

        quote!(impl sixtyfps::re_exports::PinnedDrop for #component_id {
            fn drop(self: core::pin::Pin<&mut #component_id>) {
                use sixtyfps::re_exports::*;
                #guarded_window_ref.free_graphics_resources(VRef::new_pin(self.as_ref()));
            }
        })
    };

    Some(quote!(
        #(#resource_symbols)*

        #[derive(sixtyfps::re_exports::FieldOffsets)]
        #[const_field_offset(sixtyfps::re_exports::const_field_offset)]
        #[repr(C)]
        #[pin_drop]
        #visibility struct #component_id {
            #(#item_names : sixtyfps::re_exports::#item_types,)*
            #(#declared_property_vars : sixtyfps::re_exports::Property<#declared_property_types>,)*
            #(#declared_signals : sixtyfps::re_exports::Signal<(#(#declared_signals_types,)*)>,)*
            #(#repeated_element_names : sixtyfps::re_exports::Repeater<#repeated_element_components>,)*
            self_weak: sixtyfps::re_exports::OnceCell<sixtyfps::re_exports::PinWeak<#component_id>>,
            #(parent : sixtyfps::re_exports::PinWeak<#parent_component_type>,)*
            mouse_grabber: ::core::cell::Cell<sixtyfps::re_exports::VisitChildrenResult>,
            #maybe_window_field_decl
        }

        impl sixtyfps::re_exports::Component for #component_id {
            fn visit_children_item(self: ::core::pin::Pin<&Self>, index: isize, order: sixtyfps::re_exports::TraversalOrder, visitor: sixtyfps::re_exports::ItemVisitorRefMut)
                -> sixtyfps::re_exports::VisitChildrenResult
            {
                use sixtyfps::re_exports::*;
                return sixtyfps::re_exports::visit_item_tree(self, VRef::new_pin(self), Self::item_tree(), index, order, visitor, visit_dynamic);
                #[allow(unused)]
                fn visit_dynamic(self_pinned: ::core::pin::Pin<&#component_id>, order: sixtyfps::re_exports::TraversalOrder, visitor: ItemVisitorRefMut, dyn_index: usize) -> VisitChildrenResult  {
                    match dyn_index {
                        #(#repeated_visit_branch)*
                        _ => panic!("invalid dyn_index {}", dyn_index),
                    }
                }
            }

            fn input_event(self: ::core::pin::Pin<&Self>, mouse_event : sixtyfps::re_exports::MouseEvent) -> sixtyfps::re_exports::InputEventResult {
                use sixtyfps::re_exports::*;
                let mouse_grabber = self.mouse_grabber.get();
                #[allow(unused)]
                let (status, new_grab) = if let Some((item_index, rep_index)) = mouse_grabber.aborted_indexes() {
                    let tree = Self::item_tree();
                    let offset = item_offset(self, tree, item_index);
                    let mut event = mouse_event.clone();
                    event.pos -= offset.to_vector();
                    let res = match tree[item_index] {
                        ItemTreeNode::Item { item, .. } => {
                            item.apply_pin(self).as_ref().input_event(event)
                        }
                        ItemTreeNode::DynamicTree { index } => {
                            match index {
                                #(#repeated_input_branch)*
                                _ => panic!("invalid index {}", index),
                            }
                        }
                    };
                    match res {
                        InputEventResult::GrabMouse => (res, mouse_grabber),
                        _ => (res, VisitChildrenResult::CONTINUE),
                    }
                } else {
                    process_ungrabbed_mouse_event(VRef::new_pin(self), mouse_event)
                };
                self.mouse_grabber.set(new_grab);
                status
            }

            #layouts
        }

        impl #component_id{
            pub fn new(#(parent: sixtyfps::re_exports::PinWeak::<#parent_component_type>)*)
                -> core::pin::Pin<std::rc::Rc<Self>>
            {
                #![allow(unused)]
                use sixtyfps::re_exports::*;
                ComponentVTable_static!(static VT for #component_id);
                let mut self_ = Self {
                    #(#item_names : ::core::default::Default::default(),)*
                    #(#declared_property_vars : ::core::default::Default::default(),)*
                    #(#declared_signals : ::core::default::Default::default(),)*
                    #(#repeated_element_names : ::core::default::Default::default(),)*
                    self_weak : ::core::default::Default::default(),
                    #(parent : parent as sixtyfps::re_exports::PinWeak::<#parent_component_type>,)*
                    mouse_grabber: ::core::cell::Cell::new(sixtyfps::re_exports::VisitChildrenResult::CONTINUE),
                    #maybe_window_field_init
                };
                let self_pinned = std::rc::Rc::pin(self_);
                self_pinned.self_weak.set(PinWeak::downgrade(self_pinned.clone())).map_err(|_|())
                    .expect("Can only be pinned once");
                #(#init)*
                self_pinned
            }
            #(#property_and_signal_accessors)*

            fn item_tree() -> &'static [sixtyfps::re_exports::ItemTreeNode<Self>] {
                use sixtyfps::re_exports::*;
                // FIXME: ideally this should be a const
                static ITEM_TREE : Lazy<[sixtyfps::re_exports::ItemTreeNode<#component_id>; #item_tree_array_len]>  =
                    Lazy::new(|| [#(#item_tree_array),*]);
                &*ITEM_TREE
            }
        }

        #drop_impl

        #(#extra_components)*
    ))
}

/// Return an identifier suitable for this component
fn component_id(component: &Component) -> proc_macro2::Ident {
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
    element: &ElementRc,
    property_name: &str,
) -> Option<TokenStream> {
    if let Some(animation) = element.borrow().property_animations.get(property_name) {
        let bindings: Vec<TokenStream> = animation
            .borrow()
            .bindings
            .iter()
            .map(|(prop, initializer)| {
                let prop_ident = format_ident!("{}", prop);
                let initializer = compile_expression(initializer, component);
                quote!(#prop_ident: #initializer as _)
            })
            .collect();

        Some(quote!(&sixtyfps::re_exports::PropertyAnimation{
            #(#bindings, )*
            ..::core::default::Default::default()
        }))
    } else {
        None
    }
}

fn property_set_value_tokens(
    component: &Rc<Component>,
    element: &ElementRc,
    property_name: &str,
    value_tokens: TokenStream,
) -> TokenStream {
    if let Some(animation_tokens) = property_animation_tokens(component, element, property_name) {
        quote!(set_animated_value(#value_tokens, #animation_tokens))
    } else {
        quote!(set(#value_tokens))
    }
}

fn property_set_binding_tokens(
    component: &Rc<Component>,
    element: &ElementRc,
    property_name: &str,
    binding_tokens: TokenStream,
) -> TokenStream {
    if let Some(animation_tokens) = property_animation_tokens(component, element, property_name) {
        quote!(set_animated_binding(#binding_tokens, #animation_tokens))
    } else {
        quote!(set_binding(#binding_tokens))
    }
}

/// Returns the code that can access the given property or signal (but without the set or get)
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
        let component_id = component_id(&enclosing_component);
        let name_ident = format_ident!("{}", name);
        if e.property_declarations.contains_key(name) || is_special {
            quote!(#component_id::FIELD_OFFSETS.#name_ident.apply_pin(#component_rust))
        } else {
            let elem_ident = format_ident!("{}", e.id);
            let elem_ty = format_ident!("{}", e.base_type.as_native().class_name);

            quote!((#component_id::FIELD_OFFSETS.#elem_ident + #elem_ty::FIELD_OFFSETS.#name_ident)
                .apply_pin(#component_rust)
            )
        }
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
            quote!(#component_rust.parent.upgrade().unwrap().as_ref()),
            is_special,
        )
    }
}

/// Return an expression that gets the window
fn window_ref_expression(component: &Rc<Component>) -> TokenStream {
    let mut root_component = component.clone();
    let mut component_rust = quote!(_self);
    while let Some(p) = root_component.parent_element.upgrade() {
        root_component = p.borrow().enclosing_component.upgrade().unwrap();
        component_rust = quote!(#component_rust.parent.upgrade().unwrap().as_ref());
    }
    quote!(#component_rust.as_ref().window)
}

fn compile_expression(e: &Expression, component: &Rc<Component>) -> TokenStream {
    match e {
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
                    quote!(sixtyfps::re_exports::ModelRc(std::rc::Rc::<usize>::new(#f as usize)))
                }
                (Type::Float32, Type::Color) => {
                    quote!(sixtyfps::re_exports::Color::from_argb_encoded(#f as u32))
                }
                _ => f,
            }
        }
        Expression::PropertyReference(NamedReference { element, name }) => {
            let access = access_member(
                &element.upgrade().unwrap(),
                name.as_str(),
                component,
                quote!(_self),
                false,
            );
            quote!(#access.get())
        }
        Expression::BuiltinFunctionReference(funcref) => match funcref {
            BuiltinFunction::GetWindowScaleFactor => {
                let window_ref = window_ref_expression(component);
                quote!(#window_ref.scale_factor)
            }
            BuiltinFunction::Debug => quote!((|x| println!("{:?}", x))),
        },
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
        Expression::ObjectAccess { base, name } => {
            let index = if let Type::Object(ty) = base.ty() {
                ty.keys()
                    .position(|k| k == name)
                    .expect("Expression::ObjectAccess: Cannot find a key in an object")
            } else {
                panic!("Expression::ObjectAccess's base expression is not an Object type")
            };
            let index = proc_macro2::Literal::usize_unsuffixed(index);
            let base_e = compile_expression(base, component);
            quote!((#base_e).#index )
        }
        Expression::CodeBlock(sub) => {
            let map = sub.iter().map(|e| compile_expression(e, &component));
            quote!({ #(#map);* })
        }
        Expression::SignalReference(NamedReference { element, name, .. }) => access_member(
            &element.upgrade().unwrap(),
            name.as_str(),
            component,
            quote!(_self),
            false,
        ),
        Expression::FunctionCall { function, arguments } => {
            let f = compile_expression(function, &component);
            let a = arguments.iter().map(|a| compile_expression(a, &component));
            if let Type::Signal { args } = function.ty() {
                let cast = args.iter().map(|ty| match ty {
                    Type::Bool => quote!(as bool),
                    Type::Int32 => quote!(as i32),
                    Type::Float32 => quote!(as f32),
                    _ => quote!(.clone()),
                });
                quote! { #f.emit(&(#((#a)#cast,)*).into())}
            } else {
                quote! { #f(#(#a.clone()),*)}
            }
        }
        Expression::SelfAssignment { lhs, rhs, op } => match &**lhs {
            Expression::PropertyReference(NamedReference { element, name }) => {
                let lhs = access_member(
                    &element.upgrade().unwrap(),
                    name.as_str(),
                    component,
                    quote!(_self),
                    false,
                );
                let rhs = compile_expression(&*rhs, &component);
                if *op == '=' {
                    quote!( #lhs.set((#rhs) as _) )
                } else {
                    let op = proc_macro2::Punct::new(*op, proc_macro2::Spacing::Alone);
                    quote!( #lhs.set(((#lhs.get() as f64) #op (#rhs as f64)) as _) )
                }
            }
            _ => panic!("typechecking should make sure this was a PropertyReference"),
        },
        Expression::BinaryExpression { lhs, rhs, op } => {
            let conv = match crate::expression_tree::operator_class(*op) {
                OperatorClass::ArithmeticOp => Some(quote!(as f64)),
                OperatorClass::ComparisonOp
                    if matches!(
                        lhs.ty(),
                        Type::Int32
                            | Type::Float32
                            | Type::Duration
                            | Type::Length
                            | Type::LogicalLength
                    ) =>
                {
                    Some(quote!(as f64))
                }
                _ => None,
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
            quote!( ((#lhs #conv ) #op (#rhs #conv )) )
        }
        Expression::UnaryOp { sub, op } => {
            let sub = compile_expression(&*sub, &component);
            let op = proc_macro2::Punct::new(*op, proc_macro2::Spacing::Alone);
            quote!( #op #sub )
        }
        Expression::ResourceReference { absolute_source_path } => {
            if let Some(id) = component
                .referenced_file_resources
                .borrow()
                .get(absolute_source_path)
                .filter(|_| component.embed_file_resources.get())
            {
                let symbol = format_ident!("SFPS_EMBEDDED_RESOURCE_{}", id);
                quote!(sixtyfps::re_exports::Resource::EmbeddedData(#symbol.into()))
            } else {
                quote!(sixtyfps::re_exports::Resource::AbsoluteFilePath(sixtyfps::re_exports::SharedString::from(#absolute_source_path)))
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
        Expression::Invalid | Expression::Uncompiled(_) => {
            let error = format!("unsupported expression {:?}", e);
            quote!(compile_error! {#error})
        }
        Expression::Array { values, element_ty } => {
            let rust_element_ty = rust_type(&element_ty, &Default::default()).unwrap();
            let val = values.iter().map(|e| compile_expression(e, component));
            quote!(sixtyfps::re_exports::ArrayModel::<#rust_element_ty>::from_slice(&[#(#val as _),*]))
        }
        Expression::Object { ty, values } => {
            if let Type::Object(ty) = ty {
                let elem = ty.iter().map(|(k, t)| {
                    values.get(k).map(|e| {
                        let ce = compile_expression(e, component);
                        let t = rust_type(t, &Default::default()).unwrap_or_default();
                        quote!(#ce as #t)
                    })
                });
                // This will produce a tuple
                quote!((#(#elem,)*))
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
        Expression::EnumerationValue(value) => {
            let base_ident = format_ident!("{}", value.enumeration.name);
            let value_ident = format_ident!("{}", value.to_string());
            quote!(sixtyfps::re_exports::#base_ident::#value_ident)
        }
    }
}

struct RustLanguageLayoutGen;
impl crate::layout::gen::Language for RustLanguageLayoutGen {
    type CompiledCode = TokenStream;
}

type LayoutTreeItem<'a> = crate::layout::gen::LayoutTreeItem<'a, RustLanguageLayoutGen>;

impl LayoutItemCodeGen<RustLanguageLayoutGen> for Layout {
    fn get_property_ref(&self, name: &str) -> TokenStream {
        let moved_property_name = match self.rect().mapped_property_name(name) {
            Some(name) => name,
            None => return quote!(None),
        };
        let n = format_ident!("{}", moved_property_name);
        quote! {Some(&self.#n)}
    }
    fn get_layout_info_ref<'a, 'b>(
        &'a self,
        layout_tree: &'b mut Vec<LayoutTreeItem<'a>>,
        component: &Rc<Component>,
    ) -> TokenStream {
        let self_as_layout_tree_item = collect_layouts_recursively(layout_tree, &self, component);
        match self_as_layout_tree_item {
            LayoutTreeItem::GridLayout { cell_ref_variable, spacing, padding, .. } => {
                quote!(grid_layout_info(&Slice::from_slice(&#cell_ref_variable), #spacing, #padding))
            }
            LayoutTreeItem::PathLayout(_) => todo!(),
        }
    }
}

impl LayoutItemCodeGen<RustLanguageLayoutGen> for LayoutElement {
    fn get_property_ref(&self, name: &str) -> TokenStream {
        let e = format_ident!("{}", self.element.borrow().id);
        if self.element.borrow().lookup_property(name) == Type::Length {
            let n = format_ident!("{}", name);
            quote! {Some(&self.#e.#n)}
        } else {
            quote! {None}
        }
    }
    fn get_layout_info_ref<'a, 'b>(
        &'a self,
        layout_tree: &'b mut Vec<LayoutTreeItem<'a>>,
        component: &Rc<Component>,
    ) -> TokenStream {
        let e = format_ident!("{}", self.element.borrow().id);
        let element_info = quote!(Self::FIELD_OFFSETS.#e.apply_pin(self).layouting_info(&window));
        match &self.layout {
            Some(layout) => {
                let layout_info = layout.get_layout_info_ref(layout_tree, component);
                quote!(#layout_info.merge(&#element_info))
            }
            None => element_info,
        }
    }
}

fn collect_layouts_recursively<'a, 'b>(
    layout_tree: &'b mut Vec<LayoutTreeItem<'a>>,
    layout: &'a Layout,
    component: &Rc<Component>,
) -> &'b LayoutTreeItem<'a> {
    let get_property_ref = LayoutItemCodeGen::<RustLanguageLayoutGen>::get_property_ref;
    match layout {
        Layout::GridLayout(grid_layout) => {
            let cells: Vec<TokenStream> = grid_layout
                .elems
                .iter()
                .map(|cell| {
                    let width = get_property_ref(&cell.item, "width");
                    let height = get_property_ref(&cell.item, "height");
                    let x = get_property_ref(&cell.item, "x");
                    let y = get_property_ref(&cell.item, "y");
                    let (col, row, colspan, rowspan) =
                        (cell.col, cell.row, cell.colspan, cell.rowspan);
                    let mut layout_info = cell.item.get_layout_info_ref(layout_tree, component);
                    if cell.has_explicit_restrictions() {
                        let (name, expr): (Vec<_>, Vec<_>) = cell
                            .for_each_restrictions()
                            .iter()
                            .filter_map(|(e, s)| {
                                e.as_ref().map(|e| {
                                    (format_ident!("{}", s), compile_expression(&e, component))
                                })
                            })
                            .unzip();
                        layout_info = quote!({
                            let mut layout_info = #layout_info;
                             #(layout_info.#name = #expr;)*
                            layout_info });
                    }

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
                })
                .collect();

            let cell_ref_variable = format_ident!("cells_{}", layout_tree.len());
            let cell_creation_code = quote!(let #cell_ref_variable = [#( #cells ),*];);
            let (spacing, spacing_creation_code) = if let Some(spacing) = &grid_layout.spacing {
                let variable = format_ident!("spacing_{}", layout_tree.len());
                let spacing_code = compile_expression(spacing, component);
                (quote!(#variable), Some(quote!(let #variable = #spacing_code;)))
            } else {
                (quote!(0.), None)
            };
            let padding = {
                let padding_prop = |expr| {
                    if let Some(expr) = expr {
                        compile_expression(expr, component)
                    } else {
                        quote!(0.)
                    }
                };
                let left = padding_prop(grid_layout.padding.left.as_ref());
                let right = padding_prop(grid_layout.padding.right.as_ref());
                let top = padding_prop(grid_layout.padding.top.as_ref());
                let bottom = padding_prop(grid_layout.padding.bottom.as_ref());
                quote!(&sixtyfps::re_exports::Padding {
                    left: #left,
                    right: #right,
                    top: #top,
                    bottom: #bottom,
                })
            };
            layout_tree.push(
                LayoutTreeItem::GridLayout {
                    grid: grid_layout,
                    var_creation_code: quote!(#cell_creation_code #spacing_creation_code),
                    cell_ref_variable: quote!(#cell_ref_variable),
                    spacing,
                    padding,
                }
                .into(),
            );
        }
        Layout::PathLayout(layout) => layout_tree.push(layout.into()),
    }
    layout_tree.last().unwrap()
}

impl<'a> LayoutTreeItem<'a> {
    fn emit_solve_calls(&self, component: &Rc<Component>, code_stream: &mut Vec<TokenStream>) {
        match self {
            LayoutTreeItem::GridLayout { grid, cell_ref_variable, spacing, padding, .. } => {
                let x_pos = compile_expression(&*grid.rect.x_reference, component);
                let y_pos = compile_expression(&*grid.rect.y_reference, component);
                let width = compile_expression(&*grid.rect.width_reference, component);
                let height = compile_expression(&*grid.rect.height_reference, component);

                code_stream.push(quote! {
                    solve_grid_layout(&GridLayoutData {
                        width: #width,
                        height: #height,
                        x: #x_pos as _,
                        y: #y_pos as _,
                        cells: Slice::from_slice(&#cell_ref_variable),
                        spacing: #spacing,
                        padding: #padding,
                    });
                });
            }
            LayoutTreeItem::PathLayout(path_layout) => {
                let path_layout_item_data =
                    |elem: &ElementRc, elem_rs: TokenStream, component_rust: TokenStream| {
                        let prop_ref = |n: &str| {
                            if elem.borrow().lookup_property(n) == Type::Length {
                                let n = format_ident!("{}", n);
                                quote! {Some(& #elem_rs.#n)}
                            } else {
                                quote! {None}
                            }
                        };
                        let prop_value = |n: &str| {
                            if elem.borrow().lookup_property(n) == Type::Length {
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
                                quote!(sub_comp.as_ref()),
                            );
                            push_code = quote! {
                                #push_code
                                let internal_vec = self.#repeater_id.borrow_item_vec();
                                for sub_comp in &*internal_vec {
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

                let x_pos = compile_expression(&*path_layout.rect.x_reference, &component);
                let y_pos = compile_expression(&*path_layout.rect.y_reference, &component);
                let width = compile_expression(&*path_layout.rect.width_reference, &component);
                let height = compile_expression(&*path_layout.rect.width_reference, &component);
                let offset = compile_expression(&*path_layout.offset_reference, &component);

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

fn compute_layout(component: &Rc<Component>, repeated_element_names: &[Ident]) -> TokenStream {
    let mut layouts = vec![];
    component.layout_constraints.borrow().iter().for_each(|layout| {
        let mut inverse_layout_tree = Vec::new();

        collect_layouts_recursively(&mut inverse_layout_tree, layout, component);

        layouts.extend(inverse_layout_tree.iter().filter_map(|layout| match layout {
            LayoutTreeItem::GridLayout { var_creation_code, .. } => Some(var_creation_code.clone()),
            LayoutTreeItem::PathLayout(_) => None,
        }));

        inverse_layout_tree
            .iter()
            .rev()
            .for_each(|layout| layout.emit_solve_calls(component, &mut layouts));
    });

    let window_ref = window_ref_expression(component);

    quote! {
        fn layout_info(self: ::core::pin::Pin<&Self>) -> sixtyfps::re_exports::LayoutInfo {
            todo!("Implement in rust.rs")
        }
        fn compute_layout(self: ::core::pin::Pin<&Self>) {
            #![allow(unused)]
            use sixtyfps::re_exports::*;
            let dummy = Property::<f32>::default();
            let _self = self;
            let window = #window_ref.clone();

            #(#layouts)*

            #(self.#repeated_element_names.compute_layout();)*
        }
    }
}

fn compile_path_events(events: &crate::expression_tree::PathEvents) -> TokenStream {
    use lyon::path::Event;

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
            Event::End { last, first, close } => {
                debug_assert_eq!(coordinates.first(), Some(&first));
                debug_assert_eq!(coordinates.last(), Some(&last));
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

    quote!(sixtyfps::re_exports::SharedArray::<sixtyfps::re_exports::PathEvent>::from_slice(&[#(#converted_events),*]),
           sixtyfps::re_exports::SharedArray::<sixtyfps::re_exports::Point>::from_slice(&[#(#coordinates),*]))
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
                sixtyfps::re_exports::SharedArray::<sixtyfps::re_exports::PathElement>::from_slice(&[#(#converted_elements),*])
            ))
        }
        Path::Events(events) => {
            let events = compile_path_events(events);
            quote!(sixtyfps::re_exports::PathData::Events(#events))
        }
    }
}
