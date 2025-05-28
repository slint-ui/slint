// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::rust::{ident, rust_primitive_type};
use crate::langtype::{Struct, Type};
use crate::llr;
use crate::object_tree::Document;
use crate::CompilerConfiguration;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

/// Generate the rust code for the given component.
pub fn generate(
    doc: &Document,
    compiler_config: &CompilerConfiguration,
) -> std::io::Result<TokenStream> {
    let (structs_and_enums_ids, inner_module) =
        super::rust::generate_types(&doc.used_types.borrow().structs_and_enums);

    let type_value_conversions =
        generate_value_conversions(&doc.used_types.borrow().structs_and_enums);

    let llr = crate::llr::lower_to_item_tree::lower_to_item_tree(doc, compiler_config)?;

    if llr.public_components.is_empty() {
        return Ok(Default::default());
    }

    let main_file = doc
        .node
        .as_ref()
        .ok_or_else(|| std::io::Error::other("Cannot determine path of the main file"))?
        .source_file
        .path()
        .to_string_lossy();

    let public_components = llr
        .public_components
        .iter()
        .map(|p| generate_public_component(p, compiler_config, &main_file));

    let globals = llr
        .globals
        .iter_enumerated()
        .filter(|(_, glob)| glob.must_generate())
        .map(|(_, glob)| generate_global(glob, &llr));
    let globals_ids = llr.globals.iter().filter(|glob| glob.exported).flat_map(|glob| {
        std::iter::once(ident(&glob.name)).chain(glob.aliases.iter().map(|x| ident(x)))
    });
    let compo_ids = llr.public_components.iter().map(|c| ident(&c.name));

    let named_exports = super::rust::generate_named_exports(&doc.exports);
    // The inner module was meant to be internal private, but projects have been reaching into it
    // so we can't change the name of this module
    let generated_mod = doc
        .last_exported_component()
        .map(|c| format_ident!("slint_generated{}", ident(&c.id)))
        .unwrap_or_else(|| format_ident!("slint_generated"));

    Ok(quote! {
        mod #generated_mod {
            #inner_module
            #(#globals)*
            #(#public_components)*
            #type_value_conversions
        }
        #[allow(unused_imports)]
        pub use #generated_mod::{#(#compo_ids,)* #(#structs_and_enums_ids,)* #(#globals_ids,)* #(#named_exports,)*};
        #[allow(unused_imports)]
        pub use slint::{ComponentHandle as _, Global as _, ModelExt as _};
    })
}

fn generate_public_component(
    llr: &llr::PublicComponent,
    compiler_config: &CompilerConfiguration,
    main_file: &str,
) -> TokenStream {
    let public_component_id = ident(&llr.name);
    let component_name = llr.name.as_str();

    let mut property_and_callback_accessors: Vec<TokenStream> = vec![];
    for p in &llr.public_properties {
        let prop_name = p.name.as_str();
        let prop_ident = ident(&p.name);

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
                    self.0.invoke(#prop_name, &[#(#args_name.into(),)*])
                        .unwrap_or_else(|e| panic!("Cannot invoke callback {}::{}: {e}", #component_name, #prop_name))
                        .try_into().expect("Invalid return type")
                }
            ));
            let on_ident = format_ident!("on_{}", prop_ident);
            property_and_callback_accessors.push(quote!(
                #[allow(dead_code)]
                pub fn #on_ident(&self, f: impl Fn(#(#callback_args),*) -> #return_type + 'static) {
                    self.0.set_callback(#prop_name, move |values| {
                        let [#(#args_name,)*] = values else { panic!("invalid number of argument for callback {}::{}", #component_name, #prop_name) };
                        f(#(#args_name.clone().try_into().unwrap_or_else(|_| panic!("invalid argument for callback {}::{}", #component_name, #prop_name)),)*).into()
                    }).unwrap_or_else(|e| panic!("Cannot set callback {}::{}: {e}", #component_name, #prop_name))
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
                    self.0.invoke(#prop_name, &[#(#args_name.into(),)*])
                        .unwrap_or_else(|e| panic!("Cannot invoke callback {}::{}: {e}", #component_name, #prop_name))
                        .try_into().expect("Invalid return type")
                }
            ));
        } else {
            let rust_property_type = rust_primitive_type(&p.ty).unwrap();

            let getter_ident = format_ident!("get_{}", prop_ident);
            property_and_callback_accessors.push(quote!(
                #[allow(dead_code)]
                pub fn #getter_ident(&self) -> #rust_property_type {
                    #[allow(unused_imports)]
                    self.0.get_property(#prop_name)
                        .unwrap_or_else(|e| panic!("Cannot get property {}::{} - {e}", #component_name, #prop_name))
                        .try_into().expect("Invalid property type")
                }
            ));

            let setter_ident = format_ident!("set_{}", prop_ident);
            if !p.read_only {
                property_and_callback_accessors.push(quote!(
                    #[allow(dead_code)]
                    pub fn #setter_ident(&self, value: #rust_property_type) {
                        self.0.set_property(#prop_name, value.into())
                            .unwrap_or_else(|e| panic!("Cannot set property {}::{} - {e}", #component_name, #prop_name));
                    }
                ));
            } else {
                property_and_callback_accessors.push(quote!(
                    #[allow(dead_code)] fn #setter_ident(&self, _read_only_property : ()) { }
                ));
            }
        }
    }

    let include_paths = compiler_config.include_paths.iter().map(|p| p.to_string_lossy());
    let library_paths = compiler_config.library_paths.iter().map(|(n, p)| {
        let p = p.to_string_lossy();
        quote!((#n, #p))
    });
    let style = compiler_config.style.iter();

    quote!(
        pub struct #public_component_id(slint_interpreter::ComponentInstance);

        impl #public_component_id {
            pub fn new() -> core::result::Result<Self, slint::PlatformError> {
                let mut compiler = slint_interpreter::Compiler::default();
                compiler.set_include_paths([#(#include_paths),*].into_iter().collect());
                compiler.set_library_paths([#(#library_paths),*].into_iter().collect());
                #(compiler.set_style(#style);)*

                let mut future = ::core::pin::pin!(compiler.build_from_path(#main_file));
                let mut cx = ::std::task::Context::from_waker(::std::task::Waker::noop());
                let ::std::task::Poll::Ready(result) = ::std::future::Future::poll(future.as_mut(), &mut cx) else { unreachable!("Compiler returned Pending") };
                result.print_diagnostics();
                assert!(!result.has_errors(), "Was not able to compile the file");
                let definition = result.component(#component_name).expect("Cannot open component");
                let instance = definition.create()?;
                Ok(Self(instance))
            }

            #(#property_and_callback_accessors)*
        }

        impl slint::ComponentHandle for #public_component_id {
            type Inner = <slint_interpreter::ComponentInstance as slint::ComponentHandle>::Inner;
            fn as_weak(&self) -> slint::Weak<Self> {
                slint::Weak::new(&slint::ComponentHandle::clone_strong(&self.0).into())
            }

            fn clone_strong(&self) -> Self {
                Self(slint::ComponentHandle::clone_strong(&self.0))
            }

            fn from_inner(inner: sp::VRc<sp::ItemTreeVTable, Self::Inner>) -> Self {
                Self(slint::ComponentHandle::from_inner(inner))
            }

            fn run(&self) -> core::result::Result<(), slint::PlatformError> {
                slint::ComponentHandle::run(&self.0)
            }

            fn show(&self) -> core::result::Result<(), slint::PlatformError> {
                slint::ComponentHandle::show(&self.0)
            }

            fn hide(&self) -> core::result::Result<(), slint::PlatformError> {
                slint::ComponentHandle::hide(&self.0)
            }

            fn window(&self) -> &slint::Window {
                slint::ComponentHandle::window(&self.0)
            }

            fn global<'a, T: slint::Global<'a, Self>>(&'a self) -> T {
                T::get(&self)
            }
        }
    )
}

fn generate_global(global: &llr::GlobalComponent, root: &llr::CompilationUnit) -> TokenStream {
    if !global.exported {
        return quote!();
    }
    let global_name = global.name.as_str();
    let mut property_and_callback_accessors: Vec<TokenStream> = vec![];
    for p in &global.public_properties {
        let prop_name = p.name.as_str();
        let prop_ident = ident(&p.name);

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
                    self.0.invoke_global(#global_name, #prop_name, &[#(#args_name.into(),)*])
                        .unwrap_or_else(|e| panic!("Cannot invoke callback {}::{}: {e}", #global_name, #prop_name))
                        .try_into().expect("Invalid return type")
                }
            ));
            let on_ident = format_ident!("on_{}", prop_ident);
            property_and_callback_accessors.push(quote!(
                #[allow(dead_code)]
                pub fn #on_ident(&self, f: impl Fn(#(#callback_args),*) -> #return_type + 'static) {
                    self.0.set_global_callback(#global_name, #prop_name, move |values| {
                        let [#(#args_name,)*] = values else { panic!("invalid number of argument for callback {}::{}", (#global_name, #prop_name)) };
                        f(#(#args_name.clone().try_into().unwrap_or_else(|_| panic!("invalid argument for callback {}::{}", (#global_name, #prop_name))),)*).into()
                    }).unwrap_or_else(|e| panic!("Cannot set callback {}::{}: {e}", #global_name, #prop_name))
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
                    self.0.invoke_global(#global_name, #prop_name, &[#(#args_name.into(),)*])
                        .unwrap_or_else(|e| panic!("Cannot invoke callback {}::{}: {e}", #global_name, #prop_name))
                        .try_into().expect("Invalid return type")
                }
            ));
        } else {
            let rust_property_type = rust_primitive_type(&p.ty).unwrap();

            let getter_ident = format_ident!("get_{}", prop_ident);
            property_and_callback_accessors.push(quote!(
                #[allow(dead_code)]
                pub fn #getter_ident(&self) -> #rust_property_type {
                    #[allow(unused_imports)]
                    self.0.get_global_property(#global_name, #prop_name)
                        .unwrap_or_else(|e| panic("Cannot get property {}::{} - {e}", #global_name, #prop_name))
                        .try_into().expect("Invalid property type")
                }
            ));

            let setter_ident = format_ident!("set_{}", prop_ident);
            if !p.read_only {
                property_and_callback_accessors.push(quote!(
                    #[allow(dead_code)]
                    pub fn #setter_ident(&self, value: #rust_property_type) {
                        self.0.set__global_property(#global_name, #prop_name, value.into())
                            .unwrap_or_else(|e| panic("Cannot set property {}::{} - {e}", #global_name, #prop_name));
                    }
                ));
            } else {
                property_and_callback_accessors.push(quote!(
                    #[allow(dead_code)] fn #setter_ident(&self, _read_only_property : ()) { }
                ));
            }
        }
    }

    let public_component_id = ident(&global.name);
    let aliases = global.aliases.iter().map(|name| ident(name));
    let getters = root.public_components.iter().map(|c| {
        let root_component_id = ident(&c.name);
        quote! {
            impl<'a> slint::Global<'a, #root_component_id> for #public_component_id<'a> {
                fn get(component: &'a #root_component_id) -> Self {
                    Self(&component.0)
                }
            }
        }
    });

    quote!(
        #[allow(unused)]
        pub struct #public_component_id<'a>(&'a slint_interpreter::ComponentInstance);

        impl<'a> #public_component_id<'a> {
            #(#property_and_callback_accessors)*
        }
        #(pub type #aliases<'a> = #public_component_id<'a>;)*
        #(#getters)*
    )
}

fn generate_value_conversions(used_types: &[Type]) -> TokenStream {
    let r = used_types
        .iter()
        .filter_map(|ty| match ty {
            Type::Struct(s) => match s.as_ref() {
                Struct { fields, name: Some(name), node: Some(_), .. } => {
                    let ty = ident(name);
                    let field_names = fields.keys().map(|k| k.as_str()).collect::<Vec<_>>();
                    let fields = field_names.iter().map(|k| ident(k)).collect::<Vec<_>>();
                    Some(quote!{
                        impl From<#ty> for slint_interpreter::Value {
                            fn from(value: #ty) -> Self {
                                let mut struct_ = slint_interpreter::Struct::default();
                                #(struct_.set_field(#field_names.into(), value.#fields.into());)*
                                slint_interpreter::Value::Struct(struct_)
                            }
                        }
                        impl TryFrom<slint_interpreter::Value> for #ty {
                            type Error = ();
                            fn try_from(v: slint_interpreter::Value) -> ::core::result::Result<Self, ()> {
                                match v {
                                    slint_interpreter::Value::Struct(x) => {
                                        ::core::result::Result::Ok(Self {
                                            #(#fields: x.get_field(#field_names).ok_or(())?.clone().try_into().map_err(|_|())?,)*
                                        })
                                    }
                                    _ => Err(()),
                                }
                            }
                        }
                    })
                }
                _ => None,
            },
            Type::Enumeration(en) => {
                let name = en.name.as_str();
                let ty = ident(&en.name);
                Some(quote!{
                    impl From<#ty> for slint_interpreter::Value {
                        fn from(v: #ty) -> Self {
                            Self::EnumerationValue(#name.to_owned(), v.to_string())
                        }
                    }
                    impl TryFrom<slint_interpreter::Value> for #ty {
                        type Error = ();
                        fn try_from(v: slint_interpreter::Value) -> Result<Self, ()> {
                            use ::std::str::FromStr;
                            match v {
                                slint_interpreter::Value::EnumerationValue(enumeration, value) => {
                                    if enumeration != #name {
                                        return Err(());
                                    }
                                    #ty::from_str(value.as_str()).map_err(|_| ())
                                }
                                _ => Err(()),
                            }
                        }
                    }
                })
            },
            _ => None,
        });
        quote!(#(#r)*)
}
