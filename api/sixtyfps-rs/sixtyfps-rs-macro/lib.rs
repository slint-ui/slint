extern crate proc_macro;
use proc_macro::{Spacing, TokenStream};
use quote::{quote, ToTokens};
use sixtyfps_compiler::expression_tree::Expression;
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
        use sixtyfps_compiler::typeregister::Type;
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

    let (syntax_node, mut diag) = parser::parse_tokens(tokens);

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
        return diag.into_token_stream().into();
    }

    let lower = lower::LoweredComponent::lower(&tree.root_component);

    // FIXME! ideally we would still have the spans available
    let component_id = quote::format_ident!("{}", lower.id);

    let mut declared_property_var_names = vec![];
    let mut declared_property_vars = vec![];
    let mut declared_property_types = vec![];
    for (prop_name, property_decl) in lower.property_declarations.iter() {
        let member_name = prop_name;
        declared_property_var_names.push(member_name.clone());
        declared_property_vars.push(quote::format_ident!("{}", member_name));
        declared_property_types.push(property_decl.rust_type().unwrap_or_else(|err| {
            diag.push_compiler_error(err);
            quote!().into()
        }));
    }

    if diag.has_error() {
        return diag.into_token_stream().into();
    }

    let mut item_tree_array = Vec::new();
    let mut item_names = Vec::new();
    let mut item_types = Vec::new();
    let mut init = Vec::new();
    generator::build_array_helper(&lower, |item, children_index| {
        let field_name = quote::format_ident!("{}", item.id);
        let vtable = quote::format_ident!("{}", item.native_type.vtable);
        let children_count = item.children.len() as u32;
        item_tree_array.push(quote!(
            sixtyfps::re_exports::ItemTreeNode::Item{
                offset: #component_id::field_offsets().#field_name.get_byte_offset() as isize,
                vtable: &#vtable as *const _,
                chilren_count: #children_count,
                children_index: #children_index,
             }
        ));
        for (k, v) in &item.init_properties {
            let rust_property_ident = quote::format_ident!("{}", k);
            let rust_property_accessor_prefix = if item.property_declarations.contains(k) {
                proc_macro2::TokenStream::new()
            } else {
                quote!(#field_name.)
            };
            let rust_property = quote!(#rust_property_accessor_prefix#rust_property_ident);

            let v = match v {
                Expression::StringLiteral(s) => {
                    quote!(sixtyfps::re_exports::SharedString::from(#s))
                }
                Expression::NumberLiteral(n) => quote!(#n),
                _ => quote!(compile_error! {"unsupported expression"}),
            };
            init.push(quote!(self_.#rust_property.set(#v as _);));
        }
        item_names.push(field_name);
        item_types.push(quote::format_ident!("{}", item.native_type.class_name));
    });

    let item_tree_array_len = item_tree_array.len();

    quote!(
        use sixtyfps::re_exports::const_field_offset;
        #[derive(sixtyfps::re_exports::FieldOffsets)]
        #[repr(C)]
        struct #component_id {
            #(#item_names : sixtyfps::re_exports::#item_types,)*
            #(#declared_property_vars : sixtyfps::re_exports::Property<#declared_property_types>,)*
        }

        impl core::default::Default for #component_id {
            fn default() -> Self {
                let mut self_ = Self {
                    #(#item_names : Default::default(),)*
                    #(#declared_property_vars : Default::default(),)*
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
    )
    .into()
}
