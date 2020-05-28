extern crate proc_macro;
use proc_macro::{Spacing, TokenStream};
use quote::{quote, ToTokens};
use sixtyfps_compiler::expression_tree::Expression;
use sixtyfps_compiler::typeregister::Type;
use sixtyfps_compiler::*;

fn fill_token_vec(stream: TokenStream, vec: &mut Vec<parser::Token>) {
    let mut prev_spacing = Spacing::Alone;
    for t in stream {
        use parser::SyntaxKind;
        use proc_macro::TokenTree;

        match t {
            TokenTree::Ident(i) => {
                vec.push(parser::Token {
                    kind: SyntaxKind::Identifier,
                    text: i.to_string().into(),
                    span: Some(i.span()),
                    ..Default::default()
                });
            }
            TokenTree::Punct(p) => {
                let kind = match p.as_char() {
                    ':' => SyntaxKind::Colon,
                    '=' => {
                        if let Some(last) = vec.last_mut() {
                            if last.kind == SyntaxKind::Colon && prev_spacing == Spacing::Joint {
                                last.kind = SyntaxKind::ColonEqual;
                                last.text = ":=".into();
                                continue;
                            }
                        }
                        SyntaxKind::Equal
                    }
                    ';' => SyntaxKind::Semicolon,
                    '!' => SyntaxKind::Bang,
                    '.' => SyntaxKind::Dot,
                    '<' => SyntaxKind::LAngle,
                    '>' => {
                        if let Some(last) = vec.last_mut() {
                            if last.kind == SyntaxKind::Equal && prev_spacing == Spacing::Joint {
                                last.kind = SyntaxKind::FatArrow;
                                last.text = "=>".into();
                                continue;
                            }
                        }
                        SyntaxKind::RAngle
                    }
                    _ => SyntaxKind::Error,
                };
                prev_spacing = p.spacing();
                vec.push(parser::Token {
                    kind,
                    text: p.to_string().into(),
                    span: Some(p.span()),
                    ..Default::default()
                });
            }
            TokenTree::Literal(l) => {
                let s = l.to_string();
                // Why can't the rust API give me the type of the literal
                let f = s.chars().next().unwrap();
                let kind = if f == '"' {
                    SyntaxKind::StringLiteral
                } else if f.is_digit(10) {
                    SyntaxKind::NumberLiteral
                } else {
                    SyntaxKind::Error
                };
                vec.push(parser::Token {
                    kind,
                    text: s.into(),
                    span: Some(l.span()),
                    ..Default::default()
                });
            }
            TokenTree::Group(g) => {
                use proc_macro::Delimiter::*;
                use SyntaxKind::*;
                let (l, r, sl, sr) = match g.delimiter() {
                    Parenthesis => (LParent, RParent, "(", ")"),
                    Brace => (LBrace, RBrace, "{", "}"),
                    Bracket => todo!(),
                    None => todo!(),
                };
                vec.push(parser::Token {
                    kind: l,
                    text: sl.into(),
                    span: Some(g.span()), // span_open is not stable
                    ..Default::default()
                });
                fill_token_vec(g.stream(), vec);
                vec.push(parser::Token {
                    kind: r,
                    text: sr.into(),
                    span: Some(g.span()), // span_clone is not stable
                    ..Default::default()
                });
            }
        }
    }
}

trait RustType {
    fn rust_type(&self) -> Result<proc_macro2::TokenStream, diagnostics::CompilerDiagnostic>;
}

impl RustType for sixtyfps_compiler::object_tree::PropertyDeclaration {
    fn rust_type(&self) -> Result<proc_macro2::TokenStream, diagnostics::CompilerDiagnostic> {
        match self.property_type {
            Type::Int32 => Ok(quote!(i32)),
            Type::Float32 => Ok(quote!(f32)),
            Type::String => Ok(quote!(sixtyfps::re_exports::SharedString)),
            Type::Color => Ok(quote!(u32)),
            Type::Bool => Ok(quote!(bool)),
            _ => Err(diagnostics::CompilerDiagnostic {
                message: "Cannot map property type to Rust".into(),
                span: self.type_location.clone(),
            }),
        }
    }
}

#[proc_macro]
pub fn sixtyfps(stream: TokenStream) -> TokenStream {
    let mut tokens = vec![];
    fill_token_vec(stream, &mut tokens);

    let (syntax_node, mut diag) = parser::parse_tokens(tokens.clone());

    if let Ok(cargo_manifest) = std::env::var("CARGO_MANIFEST_DIR") {
        diag.current_path = cargo_manifest.into();
        diag.current_path.push("Cargo.toml");
    }

    //println!("{:#?}", syntax_node);
    let mut tr = typeregister::TypeRegister::builtin();
    let tree = object_tree::Document::from_node(syntax_node, &mut diag, &mut tr);
    run_passes(&tree, &mut diag, &mut tr);
    //println!("{:#?}", tree);
    if diag.has_error() {
        diag.map_offsets_to_span(&tokens);
        return diag.into_token_stream().into();
    }

    // FIXME! ideally we would still have the spans available
    let component_id = quote::format_ident!("{}", tree.root_component.id);

    let mut declared_property_var_names = vec![];
    let mut declared_property_vars = vec![];
    let mut declared_property_types = vec![];
    let mut declared_signals = vec![];
    for (prop_name, property_decl) in
        tree.root_component.root_element.borrow().property_declarations.iter()
    {
        let prop_ident = quote::format_ident!("{}", prop_name);
        if property_decl.property_type == Type::Signal {
            declared_signals.push(prop_ident);
        } else {
            declared_property_var_names.push(prop_name.clone());
            declared_property_vars.push(prop_ident);
            declared_property_types.push(property_decl.rust_type().unwrap_or_else(|err| {
                diag.push_compiler_error(err);
                quote!().into()
            }));
        }
    }

    if diag.has_error() {
        diag.map_offsets_to_span(&tokens);
        return diag.into_token_stream().into();
    }

    let mut item_tree_array = Vec::new();
    let mut item_names = Vec::new();
    let mut item_types = Vec::new();
    let mut init = Vec::new();
    generator::build_array_helper(&tree.root_component, |item, children_index| {
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
            let tokens_for_expression = compile_expression(binding_expression);

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
                        self_.#rust_property.set(#tokens_for_expression);
                    ));
                } else {
                    init.push(quote!(
                        self_.#rust_property.set_binding(|context| {
                            let _self = context.component.downcast::<#component_id>().unwrap();
                            #tokens_for_expression
                        });
                    ));
                }
            }
        }
        item_names.push(field_name);
        item_types.push(quote::format_ident!("{}", item.base_type.as_builtin().class_name));
    });

    let item_tree_array_len = item_tree_array.len();

    let result = quote!(
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
            fn item_tree(&self) -> *const sixtyfps::re_exports::ItemTreeNode {
                use sixtyfps::re_exports::*;
                static TREE : [ItemTreeNode; #item_tree_array_len] = [#(#item_tree_array),*];
                TREE.as_ptr()
            }
            fn create() -> Self {
                Default::default()
            }
        }

        impl #component_id{
            // FIXME: we need a static lifetime for winit run, so this takes the component by value and put it in a leaked box
            // Ideally we would not need a static lifetime to run the engine. (eg: use run_return function of winit)
            fn run(self) {
                use sixtyfps::re_exports::*;
                sixtyfps::re_exports::ComponentVTable_static!(#component_id);
                let static_self = Box::leak(Box::new(self));
                sixtyfps_runtime_run_component_with_gl_renderer(VRefMut::new(static_self));
            }
        }
    );
    result.into()
}

fn compile_expression(e: &Expression) -> proc_macro2::TokenStream {
    match e {
        Expression::StringLiteral(s) => quote!(sixtyfps::re_exports::SharedString::from(#s)),
        Expression::NumberLiteral(n) => quote!(#n as _),
        Expression::Cast { from, to } => {
            let f = compile_expression(&*from);
            match (from.ty(), to) {
                (Type::Float32, Type::String) | (Type::Int32, Type::String) => {
                    quote!(sixtyfps::re_exports::SharedString::from(format!("{}", #f).as_str()))
                }
                _ => f,
            }
        }
        Expression::PropertyReference { component: _, element, name } => {
            let name_ident = quote::format_ident!("{}", name);
            let e = element.upgrade().unwrap();
            if !e.borrow().property_declarations.contains_key(name) {
                let elem_ident = quote::format_ident!("{}", e.borrow().id);
                quote!(_self.#elem_ident.#name_ident.get(context))
            } else {
                quote!(_self.#name_ident.get(context))
            }
        }
        Expression::CodeBlock(sub) => {
            let map = sub.iter().map(|e| compile_expression(e));
            quote!({ #(#map);* })
        }
        // FIXME: signals!
        Expression::SignalReference { element, name, .. } => {
            let name_ident = quote::format_ident!("{}", name);
            let e = element.upgrade().unwrap();
            if !e.borrow().property_declarations.contains_key(name) {
                let elem_ident = quote::format_ident!("{}", e.borrow().id);
                quote!(_self.#elem_ident.#name_ident)
            } else {
                quote!(_self.#name_ident)
            }
        }
        Expression::FunctionCall { function } => {
            if matches!(function.ty(), Type::Signal) {
                let base = compile_expression(function);
                quote!(#base.emit(&context, ()))
            } else {
                let error = format!("the function {:?} is not a signal", e);
                quote!(compile_error! {#error})
            }
        }
        _ => {
            let error = format!("unsupported expression {:?}", e);
            quote!(compile_error! {#error})
        }
    }
}
