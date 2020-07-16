/*! module for the Rust code generator
*/

use crate::diagnostics::{CompilerDiagnostic, Diagnostics};
use crate::expression_tree::{Expression, NamedReference, OperatorClass, Path};
use crate::object_tree::{Component, ElementRc};
use crate::parser::Spanned;
use crate::typeregister::Type;
use proc_macro2::TokenStream;
use quote::quote;
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
        Type::Object(o) => {
            let elem = o.values().map(|v| rust_type(v, span)).collect::<Result<Vec<_>, _>>()?;
            // This will produce a tuple
            Ok(quote!((#(#elem,)*)))
        }
        _ => Err(CompilerDiagnostic {
            message: "Cannot map property type to Rust".into(),
            span: span.clone(),
        }),
    }
}

/// Generate the rust code for the given component.
///
/// Fill the diagnostic in case of error.
pub fn generate(component: &Rc<Component>, diag: &mut Diagnostics) -> Option<TokenStream> {
    let mut extra_components = vec![];
    let mut declared_property_vars = vec![];
    let mut declared_property_types = vec![];
    let mut declared_signals = vec![];
    let mut property_and_signal_accessors: Vec<TokenStream> = vec![];
    for (prop_name, property_decl) in component.root_element.borrow().property_declarations.iter() {
        let prop_ident = quote::format_ident!("{}", prop_name);
        if property_decl.property_type == Type::Signal {
            declared_signals.push(prop_ident.clone());
            if property_decl.expose_in_public_api {
                let emitter_ident = quote::format_ident!("emit_{}", prop_name);

                property_and_signal_accessors.push(
                    quote!(
                        #[allow(dead_code)]
                        fn #emitter_ident(self: ::core::pin::Pin<&Self>) {
                            Self::field_offsets().#prop_ident.apply_pin(self).emit(())
                        }
                    )
                    .into(),
                );
            }
        } else {
            declared_property_vars.push(prop_ident.clone());
            let rust_property_type =
                rust_type(&property_decl.property_type, &property_decl.type_location)
                    .unwrap_or_else(|err| {
                        diag.push_compiler_error(err);
                        quote!().into()
                    });
            declared_property_types.push(rust_property_type.clone());

            if property_decl.expose_in_public_api {
                let getter_ident = quote::format_ident!("get_{}", prop_name);
                let setter_ident = quote::format_ident!("set_{}", prop_name);

                property_and_signal_accessors.push(
                    quote!(
                        #[allow(dead_code)]
                        fn #getter_ident(self: ::core::pin::Pin<&Self>) -> #rust_property_type {
                            Self::field_offsets().#prop_ident.apply_pin(self).get()
                        }
                    )
                    .into(),
                );

                property_and_signal_accessors.push(
                    quote!(
                        #[allow(dead_code)]
                        fn #setter_ident(&self, value: #rust_property_type) {
                            Self::field_offsets().#prop_ident.apply(self).set(value)
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
    let mut repeated_dynmodel_names = Vec::new();
    let mut repeated_visit_branch = Vec::new();
    let mut init = Vec::new();
    super::build_array_helper(component, |item_rc, children_index| {
        let item = item_rc.borrow();
        if let Some(repeated) = &item.repeated {
            let base_component = match &item.base_type {
                Type::Component(c) => c,
                _ => panic!("should be a component because of the repeater_component pass"),
            };

            let repeater_index = repeated_element_names.len();
            let repeater_id = quote::format_ident!("repeater_{}", repeater_index);
            let rep_component_id = self::component_id(&*base_component);

            extra_components.push(generate(&*base_component, diag).unwrap_or_else(|| {
                assert!(diag.has_error());
                Default::default()
            }));
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
                    diag.push_compiler_error(err);
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
                model = quote!((if #model {Some(())} else {None}).iter().cloned())
            }

            if repeated.model.is_constant() {
                init.push(quote! {
                    self_pinned.#repeater_id.update_model(#model, || {
                        #rep_component_id::new(self_pinned.self_weak.get().unwrap().clone())
                    });
                });
                repeated_visit_branch.push(quote!(
                    #repeater_index => self_pinned.#repeater_id.visit(visitor),
                ));
            } else {
                let model_name = quote::format_ident!("model_{}", repeater_index);
                repeated_visit_branch.push(quote!(
                    #repeater_index => {
                        if self_pinned.#model_name.is_dirty() {
                            #component_id::field_offsets().#model_name.apply_pin(self_pinned).evaluate(|| {
                                let _self = self_pinned.clone();
                                self_pinned.#repeater_id.update_model(#model, || {
                                    #rep_component_id::new(self_pinned.self_weak.get().unwrap().clone())
                                });
                            });
                        }
                        self_pinned.#repeater_id.visit(visitor)
                    }
                ));
                repeated_dynmodel_names.push(model_name);
            }

            item_tree_array.push(quote!(
                sixtyfps::re_exports::ItemTreeNode::DynamicTree {
                    index: #repeater_index,
                }
            ));

            repeated_element_names.push(repeater_id);
            repeated_element_components.push(rep_component_id);
        } else {
            let field_name = quote::format_ident!("{}", item.id);
            let children_count = item.children.len() as u32;
            item_tree_array.push(quote!(
                sixtyfps::re_exports::ItemTreeNode::Item{
                    item: VOffset::new(#component_id::field_offsets().#field_name),
                    chilren_count: #children_count,
                    children_index: #children_index,
                }
            ));
            for (k, binding_expression) in &item.bindings {
                let rust_property_ident = quote::format_ident!("{}", k);
                let rust_property_accessor_prefix = if item.property_declarations.contains_key(k) {
                    proc_macro2::TokenStream::new()
                } else {
                    quote!(#field_name.)
                };
                let rust_property = quote!(#rust_property_accessor_prefix#rust_property_ident);
                let tokens_for_expression = compile_expression(binding_expression, &component);

                if matches!(item.lookup_property(k.as_str()), Type::Signal) {
                    init.push(quote!(
                        self_pinned.#rust_property.set_handler({
                            let self_weak = sixtyfps::re_exports::WeakPin::downgrade(self_pinned.clone());
                            move |()| {
                                let self_pinned = self_weak.upgrade().unwrap();
                                let _self = self_pinned.as_ref();
                                #tokens_for_expression;
                            }
                        });
                    ));
                } else {
                    let setter = if binding_expression.is_constant() {
                        property_set_value_tokens(
                            component,
                            &item_rc,
                            k,
                            quote!((#tokens_for_expression) as _),
                        )
                    } else {
                        property_set_binding_tokens(
                            component,
                            &item_rc,
                            k,
                            quote!({
                                let self_weak = sixtyfps::re_exports::WeakPin::downgrade(self_pinned.clone());
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
            item_types.push(quote::format_ident!("{}", item.base_type.as_builtin().class_name));
        }
    });

    let resource_symbols: Vec<proc_macro2::TokenStream> = component
        .embedded_file_resources
        .borrow()
        .iter()
        .map(|(path, id)| {
            let symbol = quote::format_ident!("SFPS_EMBEDDED_RESOURCE_{}", id);
            quote!(const #symbol: &'static [u8] = ::core::include_bytes!(#path);)
        })
        .collect();

    let layouts = compute_layout(component);

    let mut parent_component_type = None;
    if let Some(parent_element) = component.parent_element.upgrade() {
        if !parent_element.borrow().repeated.as_ref().map_or(false, |r| r.is_conditional_element) {
            declared_property_vars.push(quote::format_ident!("index"));
            declared_property_types.push(quote!(usize));
            declared_property_vars.push(quote::format_ident!("model_data"));
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
                    diag.push_compiler_error(err);
                    quote!().into()
                }),
            );
        }

        parent_component_type = Some(self::component_id(
            &parent_element.borrow().enclosing_component.upgrade().unwrap(),
        ));
    } else {
        declared_property_vars.push(quote::format_ident!("dpi"));
        declared_property_types.push(quote!(f32));
        init.push(quote!(self_pinned.dpi.set(1.0);));
        let window_props = |name| {
            let root_elem = component.root_element.borrow();

            if root_elem.lookup_property(name) == Type::Length {
                let root_item_name = quote::format_ident!("{}", root_elem.id);
                let name = quote::format_ident!("{}", name);
                quote!(Some(&self.#root_item_name.#name))
            } else {
                quote!(None)
            }
        };
        let width_prop = window_props("width");
        let height_prop = window_props("height");
        property_and_signal_accessors.push(quote! {
            fn run(self : core::pin::Pin<std::rc::Rc<Self>>) {
                use sixtyfps::re_exports::*;
                let window = sixtyfps::create_window();
                let window_props = WindowProperties {width: #width_prop, height: #height_prop, dpi: Some(&self.dpi)};
                window.run(VRef::new_pin(self.as_ref()), &window_props);
            }
        });
    };

    // Trick so we can use `#()` as a `if let Some` in `quote!`
    let parent_component_type = parent_component_type.iter().collect::<Vec<_>>();

    if diag.has_error() {
        return None;
    }

    Some(quote!(
        #(#resource_symbols)*

        #[derive(sixtyfps::re_exports::FieldOffsets)]
        #[const_field_offset(sixtyfps::re_exports::const_field_offset)]
        #[repr(C)]
        #[pin]
        struct #component_id {
            #(#item_names : sixtyfps::re_exports::#item_types,)*
            #(#declared_property_vars : sixtyfps::re_exports::Property<#declared_property_types>,)*
            #(#declared_signals : sixtyfps::re_exports::Signal<()>,)*
            #(#repeated_element_names : sixtyfps::re_exports::Repeater<#repeated_element_components>,)*
            #(#repeated_dynmodel_names : sixtyfps::re_exports::PropertyListenerScope,)*
            self_weak: sixtyfps::re_exports::OnceCell<sixtyfps::re_exports::WeakPin<#component_id>>,
            #(parent : sixtyfps::re_exports::WeakPin<#parent_component_type>,)*
        }

        impl sixtyfps::re_exports::Component for #component_id {
            fn visit_children_item(self: ::core::pin::Pin<&Self>, index: isize, visitor: sixtyfps::re_exports::ItemVisitorRefMut) {
                use sixtyfps::re_exports::*;
                let tree = &[#(#item_tree_array),*];
                sixtyfps::re_exports::visit_item_tree(self, VRef::new_pin(self), tree, index, visitor, visit_dynamic);
                #[allow(unused)]
                fn visit_dynamic(self_pinned: ::core::pin::Pin<&#component_id>, visitor: ItemVisitorRefMut, dyn_index: usize) {
                    match dyn_index {
                        #(#repeated_visit_branch)*
                        _ => panic!("invalid dyn_index {}", dyn_index),
                    }
                }
            }

            #layouts
        }

        impl #component_id{
            fn new(#(parent: sixtyfps::re_exports::WeakPin::<#parent_component_type>)*)
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
                    #(#repeated_dynmodel_names : ::core::default::Default::default(),)*
                    self_weak : ::core::default::Default::default(),
                    #(parent : parent as sixtyfps::re_exports::WeakPin::<#parent_component_type>,)*
                };
                let self_pinned = std::rc::Rc::pin(self_);
                self_pinned.self_weak.set(WeakPin::downgrade(self_pinned.clone())).map_err(|_|())
                    .expect("Can only be pinned once");
                #(#init)*
                self_pinned
            }
            #(#property_and_signal_accessors)*
        }

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
        quote::format_ident!("{}", id)
    } else {
        quote::format_ident!("{}", component.id)
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
                let prop_ident = quote::format_ident!("{}", prop);
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
) -> TokenStream {
    let e = element.borrow();

    let enclosing_component = e.enclosing_component.upgrade().unwrap();
    if Rc::ptr_eq(component, &enclosing_component) {
        let component_id = component_id(&enclosing_component);
        let name_ident = quote::format_ident!("{}", name);
        if e.property_declarations.contains_key(name) {
            quote!(#component_id::field_offsets().#name_ident.apply_pin(#component_rust))
        } else {
            let elem_ident = quote::format_ident!("{}", e.id);
            let elem_ty = quote::format_ident!("{}", e.base_type.as_builtin().class_name);

            quote!((#component_id::field_offsets().#elem_ident + #elem_ty::field_offsets().#name_ident)
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
        )
    }
}

/// Return an expression that gets the DPI property
fn dpi_expression(component: &Rc<Component>) -> TokenStream {
    let mut root_component = component.clone();
    let mut component_rust = quote!(_self);
    while let Some(p) = root_component.parent_element.upgrade() {
        root_component = p.borrow().enclosing_component.upgrade().unwrap();
        component_rust = quote!(#component_rust.parent.upgrade().unwrap().as_ref());
    }
    let component_id = component_id(&root_component);
    quote!(#component_id::field_offsets().dpi.apply_pin(#component_rust).get())
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
                (Type::Float32, Type::Model) | (Type::Int32, Type::Model) => quote!((0..#f as i32)),
                (Type::Array(_), Type::Model) => quote!(#f.iter().cloned()),
                (Type::Float32, Type::Color) => {
                    quote!(sixtyfps::re_exports::Color::from(#f as u32))
                }
                (Type::LogicalLength, Type::Length) => {
                    let dpi_expression = dpi_expression(component);
                    quote!((#f as f64) * #dpi_expression as f64)
                }
                (Type::Length, Type::LogicalLength) => {
                    let dpi_expression = dpi_expression(component);
                    quote!((#f as f64) / #dpi_expression as f64)
                }
                _ => f,
            }
        }
        Expression::PropertyReference(NamedReference { element, name }) => {
            let access =
                access_member(&element.upgrade().unwrap(), name.as_str(), component, quote!(_self));
            quote!(#access.get())
        }
        Expression::RepeaterIndexReference { element } => {
            if element.upgrade().unwrap().borrow().base_type == Type::Component(component.clone()) {
                let component_id = component_id(&component);
                quote!({ #component_id::field_offsets().index.apply_pin(_self).get() })
            } else {
                todo!();
            }
        }
        Expression::RepeaterModelReference { element } => {
            if element.upgrade().unwrap().borrow().base_type == Type::Component(component.clone()) {
                let component_id = component_id(&component);
                quote!({ #component_id::field_offsets().model_data.apply_pin(_self).get() })
            } else {
                todo!();
            }
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
        Expression::SignalReference(NamedReference { element, name, .. }) => {
            let access =
                access_member(&element.upgrade().unwrap(), name.as_str(), component, quote!(_self));
            quote!(#access.emit(()))
        }
        Expression::FunctionCall { function } => {
            if matches!(function.ty(), Type::Signal) {
                compile_expression(function, &component)
            } else {
                let error = format!("the function {:?} is not a signal", e);
                quote!(compile_error! {#error})
            }
        }
        Expression::SelfAssignment { lhs, rhs, op } => match &**lhs {
            Expression::PropertyReference(NamedReference { element, name }) => {
                let lhs = access_member(
                    &element.upgrade().unwrap(),
                    name.as_str(),
                    component,
                    quote!(_self),
                );
                let rhs = compile_expression(&*rhs, &component);
                let op = proc_macro2::Punct::new(*op, proc_macro2::Spacing::Alone);
                quote!( #lhs.set(#lhs.get() #op &((#rhs) as _) ))
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
            if let Some(id) = component.embedded_file_resources.borrow().get(absolute_source_path) {
                let symbol = quote::format_ident!("SFPS_EMBEDDED_RESOURCE_{}", id);
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
        Expression::Array { values, .. } => {
            //let rust_element_ty = rust_type(&element_ty, &Default::default());
            let val = values.iter().map(|e| compile_expression(e, component));
            quote!([#(#val as _),*])
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
    }
}

fn compute_layout(component: &Rc<Component>) -> TokenStream {
    let mut layouts = vec![];
    for grid_layout in component.layout_constraints.borrow().grids.iter() {
        let within = quote::format_ident!("{}", grid_layout.within.borrow().id);
        let within_ty = quote::format_ident!(
            "{}",
            grid_layout.within.borrow().base_type.as_builtin().class_name
        );
        let row_constraint = vec![quote!(Constraint::default()); grid_layout.row_count()];
        let col_constraint = vec![quote!(Constraint::default()); grid_layout.col_count()];
        let cells = grid_layout
            .elems
            .iter()
            .map(|x| {
                x.iter()
                    .map(|y| {
                        y.as_ref()
                            .map(|elem| {
                                let e = quote::format_ident!("{}", elem.borrow().id);
                                let p = |n: &str| {
                                    if elem.borrow().lookup_property(n) == Type::Length {
                                        let n = quote::format_ident!("{}", n);
                                        quote! {Some(&self.#e.#n)}
                                    } else {
                                        quote! {None}
                                    }
                                };
                                let width = p("width");
                                let height = p("height");
                                let x = p("x");
                                let y = p("y");
                                quote!(GridLayoutCellData {
                                    x: #x,
                                    y: #y,
                                    width: #width,
                                    height: #height,
                                })
                            })
                            .unwrap_or_else(|| quote!(GridLayoutCellData::default()))
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();

        let x_pos = compile_expression(&*grid_layout.x_reference, &component);
        let y_pos = compile_expression(&*grid_layout.y_reference, &component);

        layouts.push(quote! {
            solve_grid_layout(&GridLayoutData {
                row_constraint: Slice::from_slice(&[#(#row_constraint),*]),
                col_constraint: Slice::from_slice(&[#(#col_constraint),*]),
                width: (Self::field_offsets().#within + #within_ty::field_offsets().width)
                    .apply_pin(self).get(),
                height: (Self::field_offsets().#within + #within_ty::field_offsets().height)
                    .apply_pin(self).get(),
                x: #x_pos,
                y: #y_pos,
                cells: Slice::from_slice(&[#( Slice::from_slice(&[#( #cells ),*])),*]),
            });
        });
    }

    for path_layout in component.layout_constraints.borrow().paths.iter() {
        let items = path_layout
            .elements
            .iter()
            .map(|elem| {
                let e = quote::format_ident!("{}", elem.borrow().id);
                let prop_ref = |n: &str| {
                    if elem.borrow().lookup_property(n) == Type::Length {
                        let n = quote::format_ident!("{}", n);
                        quote! {Some(&self.#e.#n)}
                    } else {
                        quote! {None}
                    }
                };
                let prop_value = |n: &str| {
                    if elem.borrow().lookup_property(n) == Type::Length {
                        let accessor = access_member(&elem, n, component, quote!(self));
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
            })
            .collect::<Vec<_>>();

        let path = compile_path(&path_layout.path, &component);

        let x_pos = compile_expression(&*path_layout.x_reference, &component);
        let y_pos = compile_expression(&*path_layout.y_reference, &component);
        let width = compile_expression(&*path_layout.width_reference, &component);
        let height = compile_expression(&*path_layout.width_reference, &component);
        let offset = compile_expression(&*path_layout.offset_reference, &component);

        layouts.push(quote! {
            solve_path_layout(&PathLayoutData {
                items: Slice::from_slice(&[#( #items ),*]),
                elements: &#path,
                x: #x_pos,
                y: #y_pos,
                width: #width,
                height: #height,
                offset: #offset,
            });
        });
    }

    quote! {
        fn layout_info(self: ::core::pin::Pin<&Self>) -> sixtyfps::re_exports::LayoutInfo {
            todo!("Implement in rust.rs")
        }
        fn compute_layout(self: ::core::pin::Pin<&Self>) {
            #![allow(unused)]
            use sixtyfps::re_exports::*;
            let dummy = Property::<f32>::default();
            let _self = self;

            #(#layouts)*
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

    quote!(sixtyfps::re_exports::SharedArray::<sixtyfps::re_exports::PathEvent>::from(&[#(#converted_events),*]),
           sixtyfps::re_exports::SharedArray::<sixtyfps::re_exports::Point>::from(&[#(#coordinates),*]))
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
                            let prop_ident = quote::format_ident!("{}", property);
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
                        .rust_type_constructor
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
                sixtyfps::re_exports::SharedArray::<sixtyfps::re_exports::PathElement>::from(&[#(#converted_elements),*])
            ))
        }
        Path::Events(events) => {
            let events = compile_path_events(events);
            quote!(sixtyfps::re_exports::PathData::Events(#events))
        }
    }
}
