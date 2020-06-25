/*! module for the Rust code generator
*/

use crate::diagnostics::{CompilerDiagnostic, Diagnostics};
use crate::expression_tree::{Expression, NamedReference};
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
        Type::Color => Ok(quote!(u32)),
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
                            let eval_context = sixtyfps::re_exports::EvaluationContext::for_root_component(
                                    sixtyfps::re_exports::ComponentRef::new_pin(self)
                                );
                            self.#prop_ident.emit(&eval_context, ())
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
                            let eval_context = sixtyfps::re_exports::EvaluationContext::for_root_component(
                                   sixtyfps::re_exports::ComponentRef::new_pin(self)
                                );
                            self.#prop_ident.get(&eval_context)
                        }
                    )
                    .into(),
                );

                property_and_signal_accessors.push(
                    quote!(
                        #[allow(dead_code)]
                        fn #setter_ident(&self, value: #rust_property_type) {
                            self.#prop_ident.set(value)
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
    let mut init = Vec::new();
    super::build_array_helper(component, |item_rc, children_index| {
        let item = item_rc.borrow();
        if let Some(repeated) = &item.repeated {
            let base_component = match &item.base_type {
                Type::Component(c) => c,
                _ => panic!("should be a component because of the repeater_component pass"),
            };

            let repeater_index = repeated_element_names.len();
            let data_type = rust_type(
                &Expression::RepeaterModelReference { element: Rc::downgrade(item_rc) }.ty(),
                &item.node.as_ref().map_or_else(Default::default, |n| n.span()),
            )
            .unwrap_or_else(|err| {
                diag.push_compiler_error(err);
                quote!().into()
            });

            let repeater_id = quote::format_ident!("repeater_{}", repeater_index);
            let rep_component_id = self::component_id(&*base_component);

            extra_components.push(generate(&*base_component, diag).unwrap_or_default());
            extra_components.push(quote! {
                impl sixtyfps::re_exports::RepeatedComponent for #rep_component_id {
                    type Data = #data_type;
                    fn update(&self, index: usize, data: Self::Data) {
                        self.index.set(index);
                        self.model_data.set(data)
                    }
                }
            });

            assert!(repeated.model.is_constant(), "TODO: currently model can only be const");
            let model = compile_expression(&repeated.model, component);
            init.push(quote! {
                self_.#repeater_id.update_model(#model);
            });

            item_tree_array.push(quote!(
                sixtyfps::re_exports::ItemTreeNode::DynamicTree {
                    index: #repeater_index,
                }
            ));
            repeated_visit_branch.push(quote!(
                #repeater_index => base.#repeater_id.visit(visitor),
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
                        self_.#rust_property.set_handler(|context, ()| {
                            let _self = context.get_component::<#component_id>().unwrap();
                            #tokens_for_expression;
                        });
                    ));
                } else {
                    if binding_expression.is_constant() {
                        let setter = property_set_value_tokens(
                            component,
                            &item_rc,
                            k,
                            quote!((#tokens_for_expression) as _),
                        );
                        init.push(quote!(
                            self_.#rust_property.#setter;
                        ));
                    } else {
                        let setter = property_set_binding_tokens(
                            component,
                            &item_rc,
                            k,
                            quote!(
                                |context| {
                                    let _self = context.get_component::<#component_id>().unwrap();
                                    (#tokens_for_expression) as _
                                }
                            ),
                        );
                        init.push(quote!(
                            self_.#rust_property.#setter;
                        ));
                    }
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
            quote!(const #symbol: &'static [u8] = std::include_bytes!(#path);)
        })
        .collect();

    let layouts = compute_layout(component);

    if let Some(parent_element) = component.parent_element.upgrade() {
        declared_property_vars.push(quote::format_ident!("index"));
        declared_property_types.push(quote!(usize));
        declared_property_vars.push(quote::format_ident!("model_data"));
        declared_property_types.push(
            rust_type(
                &Expression::RepeaterModelReference { element: component.parent_element.clone() }
                    .ty(),
                &parent_element.borrow().node.as_ref().map_or_else(Default::default, |n| n.span()),
            )
            .unwrap_or_else(|err| {
                diag.push_compiler_error(err);
                quote!().into()
            }),
        );
    } else {
        property_and_signal_accessors.push(quote! {
            fn run(self) {
                use sixtyfps::re_exports::*;
                let window = sixtyfps::create_window();
                let self_pined = self;
                pin_mut!(self_pined);
                window.run(VRef::new_pin(self_pined.as_ref()));
            }
        });
    };

    Some(quote!(
        #(#resource_symbols)*

        #[derive(sixtyfps::re_exports::FieldOffsets)]
        #[const_field_offset(sixtyfps::re_exports::const_field_offset)]
        #[repr(C)]
        struct #component_id {
            #(#item_names : sixtyfps::re_exports::#item_types,)*
            #(#declared_property_vars : sixtyfps::re_exports::Property<#declared_property_types>,)*
            #(#declared_signals : sixtyfps::re_exports::Signal<()>,)*
            #(#repeated_element_names : sixtyfps::re_exports::Repeater<#repeated_element_components>,)*
        }

        impl core::default::Default for #component_id {
            fn default() -> Self {
                #![allow(unused)]
                use sixtyfps::re_exports::*;
                ComponentVTable_static!(static VT for #component_id);
                let mut self_ = Self {
                    #(#item_names : Default::default(),)*
                    #(#declared_property_vars : Default::default(),)*
                    #(#declared_signals : Default::default(),)*
                    #(#repeated_element_names : Default::default(),)*
                };
                #(#init)*
                self_
            }

        }
        impl sixtyfps::re_exports::Component for #component_id {
            fn visit_children_item(self: core::pin::Pin<&Self>, index: isize, visitor: sixtyfps::re_exports::ItemVisitorRefMut) {
                use sixtyfps::re_exports::*;
                let tree = &[#(#item_tree_array),*];
                sixtyfps::re_exports::visit_item_tree(self.get_ref(), VRef::new_pin(self), tree, index, visitor, visit_dynamic);
                #[allow(unused)]
                fn visit_dynamic(base: &#component_id, visitor: ItemVisitorRefMut, dyn_index: usize) {
                    match dyn_index {
                        #(#repeated_visit_branch)*
                        _ => panic!("invalid dyn_index {}", dyn_index),
                    }
                }
            }

            #layouts
        }

        impl #component_id{
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

        Some(quote!(&sixtyfps::re_exports::PropertyAnimation{#(#bindings)*, ..Default::default()}))
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

/// Returns the code that can access the given property or signal from the context (but without the set or get)
///
/// to be used like:
/// ```ignore
/// let (access, context) = access_member(...)
/// quote!(#access.get(#context))
/// ```
fn access_member(
    element: &ElementRc,
    name: &str,
    component: &Rc<Component>,
    context: TokenStream,
) -> (TokenStream, TokenStream) {
    let e = element.borrow();

    let enclosing_component = e.enclosing_component.upgrade().unwrap();
    let component_id = component_id(&enclosing_component);
    if Rc::ptr_eq(component, &enclosing_component) {
        let name_ident = quote::format_ident!("{}", name);
        let comp = quote!(#context.get_component::<#component_id>().unwrap());
        if e.property_declarations.contains_key(name) {
            (quote!(#comp.#name_ident), context)
        } else {
            let elem_ident = quote::format_ident!("{}", e.id);
            (quote!(#comp.#elem_ident.#name_ident), context)
        }
    } else {
        access_member(element, name, &enclosing_component, quote!(#context.parent_context.unwrap()))
    }
}

fn compile_expression(e: &Expression, component: &Rc<Component>) -> TokenStream {
    match e {
        Expression::StringLiteral(s) => quote!(sixtyfps::re_exports::SharedString::from(#s)),
        Expression::NumberLiteral(n) => quote!(#n),
        Expression::Cast { from, to } => {
            let f = compile_expression(&*from, &component);
            match (from.ty(), to) {
                (Type::Float32, Type::String) | (Type::Int32, Type::String) => {
                    quote!(sixtyfps::re_exports::SharedString::from(format!("{}", #f).as_str()))
                }
                (Type::Float32, Type::Model) | (Type::Int32, Type::Model) => quote!((0..#f as i32)),
                (Type::Array(_), Type::Model) => quote!(#f.iter().cloned()),
                _ => f,
            }
        }
        Expression::PropertyReference(NamedReference { element, name }) => {
            let (access, context) = access_member(
                &element.upgrade().unwrap(),
                name.as_str(),
                component,
                quote!(context),
            );
            quote!(#access.get(#context))
        }
        Expression::RepeaterIndexReference { element } => {
            if element.upgrade().unwrap().borrow().base_type == Type::Component(component.clone()) {
                quote!({ _self.index.get(context) })
            } else {
                todo!();
            }
        }
        Expression::RepeaterModelReference { element } => {
            if element.upgrade().unwrap().borrow().base_type == Type::Component(component.clone()) {
                quote!({ _self.model_data.get(context) })
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
            let (access, context) = access_member(
                &element.upgrade().unwrap(),
                name.as_str(),
                component,
                quote!(context),
            );
            quote!(#access.emit(#context, ()))
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
                let (lhs, context) = access_member(
                    &element.upgrade().unwrap(),
                    name.as_str(),
                    component,
                    quote!(context),
                );
                let rhs = compile_expression(&*rhs, &component);
                let op = proc_macro2::Punct::new(*op, proc_macro2::Spacing::Alone);
                quote!( #lhs.set(#lhs.get(#context) #op &((#rhs) as _) ))
            }
            _ => panic!("typechecking should make sure this was a PropertyReference"),
        },
        Expression::BinaryExpression { lhs, rhs, op } => {
            let lhs = compile_expression(&*lhs, &component);
            let rhs = compile_expression(&*rhs, &component);
            let op = proc_macro2::Punct::new(*op, proc_macro2::Spacing::Alone);
            quote!( ((#lhs as f64) #op (#rhs as f64)) )
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
    }
}

fn compute_layout(component: &Component) -> TokenStream {
    let mut layouts = vec![];
    for x in component.layout_constraints.borrow().0.iter() {
        let within = quote::format_ident!("{}", x.within.borrow().id);
        let row_constraint = vec![quote!(Constraint::default()); x.row_count()];
        let col_constraint = vec![quote!(Constraint::default()); x.col_count()];
        let cells = x
            .elems
            .iter()
            .map(|x| {
                x.iter()
                    .map(|y| {
                        y.as_ref()
                            .map(|elem| {
                                let e = quote::format_ident!("{}", elem.borrow().id);
                                let p = |n: &str| {
                                    if elem.borrow().lookup_property(n) == Type::Float32 {
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

        layouts.push(quote! {
            solve_grid_layout(&GridLayoutData {
                row_constraint: Slice::from_slice(&[#(#row_constraint),*]),
                col_constraint: Slice::from_slice(&[#(#col_constraint),*]),
                width: self.#within.width.get(eval_context),
                height: self.#within.height.get(eval_context),
                x: 0.,
                y: 0.,
                cells: Slice::from_slice(&[#( Slice::from_slice(&[#( #cells ),*])),*]),
            });
        });
    }

    quote! {
        fn layout_info(self: core::pin::Pin<&Self>) -> sixtyfps::re_exports::LayoutInfo {
            todo!("Implement in rust.rs")
        }
        fn compute_layout(self: core::pin::Pin<&Self>, eval_context: &sixtyfps::re_exports::EvaluationContext) {
            #![allow(unused)]
            use sixtyfps::re_exports::*;
            let dummy = Property::<f32>::default();

            #(#layouts)*
        }
    }
}
