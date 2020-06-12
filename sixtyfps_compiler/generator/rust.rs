/*! module for the Rust code generator
*/

use crate::diagnostics::{CompilerDiagnostic, Diagnostics};
use crate::expression_tree::Expression;
use crate::object_tree::{Component, ElementRc, PropertyDeclaration};
use crate::typeregister::Type;
use proc_macro2::TokenStream;
use quote::quote;

trait RustType {
    fn rust_type(&self) -> Result<proc_macro2::TokenStream, CompilerDiagnostic>;
}

impl RustType for PropertyDeclaration {
    fn rust_type(&self) -> Result<proc_macro2::TokenStream, CompilerDiagnostic> {
        match self.property_type {
            Type::Int32 => Ok(quote!(i32)),
            Type::Float32 => Ok(quote!(f32)),
            Type::String => Ok(quote!(sixtyfps::re_exports::SharedString)),
            Type::Color => Ok(quote!(u32)),
            Type::Bool => Ok(quote!(bool)),
            _ => Err(CompilerDiagnostic {
                message: "Cannot map property type to Rust".into(),
                span: self.type_location.clone(),
            }),
        }
    }
}

/// Generate the rust code for the given component.
///
/// Fill the diagnostic in case of error.
pub fn generate(component: &Component, diag: &mut Diagnostics) -> Option<TokenStream> {
    let mut declared_property_var_names = vec![];
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
                        fn #emitter_ident(&self) {
                            let eval_context = sixtyfps::re_exports::EvaluationContext{ component: sixtyfps::re_exports::ComponentRef::new(self) };
                            self.#prop_ident.emit(&eval_context, ())
                        }
                    )
                    .into(),
                );
            }
        } else {
            declared_property_var_names.push(prop_name.clone());
            declared_property_vars.push(prop_ident.clone());
            let rust_property_type = property_decl.rust_type().unwrap_or_else(|err| {
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
                        fn #getter_ident(&self) -> #rust_property_type {
                            let eval_context = sixtyfps::re_exports::EvaluationContext{ component: sixtyfps::re_exports::ComponentRef::new(self) };
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

    // Fixme! Ideally we would still have the spans available
    let component_id = quote::format_ident!("{}", component.id);

    let mut item_tree_array = Vec::new();
    let mut item_names = Vec::new();
    let mut item_types = Vec::new();
    let mut init = Vec::new();
    super::build_array_helper(component, |item, children_index| {
        let item = item.borrow();
        let field_name = quote::format_ident!("{}", item.id);
        let vtable = quote::format_ident!("{}", item.base_type.as_builtin().vtable_symbol);
        let children_count = item.children.len() as u32;
        item_tree_array.push(quote!(
            sixtyfps::re_exports::ItemTreeNode::Item{
                offset: #component_id::field_offsets().#field_name.get_byte_offset() as isize,
                vtable: &#vtable as *const _,
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
                        let _self = context.component.downcast::<#component_id>().unwrap();
                        #tokens_for_expression;
                    });
                ));
            } else {
                if binding_expression.is_constant() {
                    init.push(quote!(
                        self_.#rust_property.set((#tokens_for_expression) as _);
                    ));
                } else {
                    init.push(quote!(
                        self_.#rust_property.set_binding(|context| {
                            let _self = context.component.downcast::<#component_id>().unwrap();
                            (#tokens_for_expression) as _
                        });
                    ));
                }
            }
        }
        item_names.push(field_name);
        item_types.push(quote::format_ident!("{}", item.base_type.as_builtin().class_name));
    });

    let item_tree_array_len = item_tree_array.len();

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

    Some(quote!(
        #(#resource_symbols)*

        use sixtyfps::re_exports::const_field_offset;
        #[derive(sixtyfps::re_exports::FieldOffsets)]
        #[repr(C)]
        struct #component_id {
            #(#item_names : sixtyfps::re_exports::#item_types,)*
            #(#declared_property_vars : sixtyfps::re_exports::Property<#declared_property_types>,)*
            #(#declared_signals : sixtyfps::re_exports::Signal<()>,)*
        }

        impl core::default::Default for #component_id {
            fn default() -> Self {
                #![allow(unused_braces)] // The generated code may contain unused braces
                let mut self_ = Self {
                    #(#item_names : Default::default(),)*
                    #(#declared_property_vars : Default::default(),)*
                    #(#declared_signals : Default::default(),)*
                };
                #(#init)*
                self_
            }

        }
        impl sixtyfps::re_exports::Component for #component_id {
            fn visit_children_item(&self, index: isize, visitor: sixtyfps::re_exports::ItemVisitorRefMut) {
                use sixtyfps::re_exports::*;
                static TREE : [ItemTreeNode; #item_tree_array_len] = [#(#item_tree_array),*];
                unsafe { visit_item_tree(VRef::new(self), &TREE, index, visitor) };
            }
            fn create() -> Self {
                Default::default()
            }

            #layouts
        }

        impl #component_id{
            fn run(self) {
                use sixtyfps::re_exports::*;
                sixtyfps::re_exports::ComponentVTable_static!(static VT for #component_id);
                sixtyfps_runtime_run_component_with_gl_renderer(VRef::new(&self));
            }

            #(#property_and_signal_accessors)*
        }
    ))
}

fn access_member(element: &ElementRc, name: &str) -> TokenStream {
    let e = element.borrow();
    let name_ident = quote::format_ident!("{}", name);
    if e.property_declarations.contains_key(name) {
        quote!(#name_ident)
    } else {
        let elem_ident = quote::format_ident!("{}", e.id);
        quote!(#elem_ident.#name_ident )
    }
}

fn compile_expression(e: &Expression, component: &Component) -> TokenStream {
    match e {
        Expression::StringLiteral(s) => quote!(sixtyfps::re_exports::SharedString::from(#s)),
        Expression::NumberLiteral(n) => quote!(#n),
        Expression::Cast { from, to } => {
            let f = compile_expression(&*from, &component);
            match (from.ty(), to) {
                (Type::Float32, Type::String) | (Type::Int32, Type::String) => {
                    quote!(sixtyfps::re_exports::SharedString::from(format!("{}", #f).as_str()))
                }
                _ => f,
            }
        }
        Expression::PropertyReference { component: _, element, name } => {
            let access = access_member(&element.upgrade().unwrap(), name.as_str());
            quote!(_self.#access.get(context))
        }
        Expression::CodeBlock(sub) => {
            let map = sub.iter().map(|e| compile_expression(e, &component));
            quote!({ #(#map);* })
        }
        Expression::SignalReference { element, name, .. } => {
            let access = access_member(&element.upgrade().unwrap(), name.as_str());
            quote!(_self.#access)
        }
        Expression::FunctionCall { function } => {
            if matches!(function.ty(), Type::Signal) {
                let base = compile_expression(function, &component);
                quote!(#base.emit(&context, ()))
            } else {
                let error = format!("the function {:?} is not a signal", e);
                quote!(compile_error! {#error})
            }
        }
        Expression::SelfAssignment { lhs, rhs, op } => match &**lhs {
            Expression::PropertyReference { element, name, .. } => {
                let lhs = access_member(&element.upgrade().unwrap(), name.as_str());
                let rhs = compile_expression(&*rhs, &component);
                let op = proc_macro2::Punct::new(*op, proc_macro2::Spacing::Alone);
                quote!( _self.#lhs.set(_self.#lhs.get(context) #op &((#rhs) as _) ))
            }
            _ => panic!("typechecking should make sure this was a PropertyReference"),
        },
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
        _ => {
            let error = format!("unsupported expression {:?}", e);
            quote!(compile_error! {#error})
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
                width: self.#within.width.get(&eval_context),
                height: self.#within.height.get(&eval_context),
                x: 0.,
                y: 0.,
                cells: Slice::from_slice(&[#( Slice::from_slice(&[#( #cells ),*])),*]),
            });
        });
    }

    quote! {
        fn layout_info(&self) -> sixtyfps::re_exports::LayoutInfo {
                todo!("Implement in rust.rs")
        }
        fn compute_layout(&self) {
            #![allow(unused)]
            use sixtyfps::re_exports::*;
            let eval_context = EvaluationContext{ component: ComponentRef::new(self) };
            let dummy = Property::<f32>::default();

            #(#layouts)*
        }
    }
}
