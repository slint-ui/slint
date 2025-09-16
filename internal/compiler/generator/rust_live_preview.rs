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
    let module_header = super::rust::generate_module_header();

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
        .path();
    let main_file = std::path::absolute(main_file).unwrap_or_else(|_| main_file.to_path_buf());
    let main_file = main_file.to_string_lossy();

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
            #module_header
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

    let main_file = if main_file.ends_with("Cargo.toml") {
        // We couldn't get the actual .rs file from a slint! macro, so use file!() which will expand to the actual file name
        let current_dir = std::env::current_dir().unwrap_or_default();
        let current_dir = current_dir.to_string_lossy();
        quote!(std::path::Path::new(#current_dir).join(file!()))
    } else {
        quote!(#main_file)
    };

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
                    self.0.borrow().invoke(#prop_name, &[#(#args_name.into(),)*])
                        .try_into().unwrap_or_else(|_| panic!("Invalid return type for callback {}::{}", #component_name, #prop_name))
                }
            ));
            let on_ident = format_ident!("on_{}", prop_ident);
            property_and_callback_accessors.push(quote!(
                #[allow(dead_code)]
                pub fn #on_ident(&self, f: impl FnMut(#(#callback_args),*) -> #return_type + 'static) {
                    let f = ::core::cell::RefCell::new(f);
                    self.0.borrow_mut().set_callback(#prop_name, sp::Rc::new(move |values| {
                        let [#(#args_name,)*] = values else { panic!("invalid number of argument for callback {}::{}", #component_name, #prop_name) };
                        (*f.borrow_mut())(#(#args_name.clone().try_into().unwrap_or_else(|_| panic!("invalid argument for callback {}::{}", #component_name, #prop_name)),)*).into()
                    }))
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
                    self.0.borrow().invoke(#prop_name, &[#(#args_name.into(),)*])
                        .try_into().unwrap_or_else(|_| panic!("Invalid return type for function {}::{}", #component_name, #prop_name))
                }
            ));
        } else {
            let rust_property_type = rust_primitive_type(&p.ty).unwrap();
            let convert_to_value = convert_to_value_fn(&p.ty);
            let convert_from_value = convert_from_value_fn(&p.ty);

            let getter_ident = format_ident!("get_{}", prop_ident);
            property_and_callback_accessors.push(quote!(
                #[allow(dead_code)]
                pub fn #getter_ident(&self) -> #rust_property_type {
                    #convert_from_value(self.0.borrow().get_property(#prop_name))
                        .unwrap_or_else(|_| panic!("Invalid property type for {}::{}", #component_name, #prop_name))
                }
            ));

            let setter_ident = format_ident!("set_{}", prop_ident);
            if !p.read_only {
                property_and_callback_accessors.push(quote!(
                    #[allow(dead_code)]
                    pub fn #setter_ident(&self, value: #rust_property_type) {
                        self.0.borrow_mut().set_property(#prop_name, #convert_to_value(value))
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
        quote!((#n.to_string(), #p.into()))
    });
    let style = compiler_config.style.iter();

    quote!(
        pub struct #public_component_id(sp::Rc<::core::cell::RefCell<sp::live_preview::LiveReloadingComponent>>, sp::Rc<dyn sp::WindowAdapter>);

        impl #public_component_id {
            pub fn new() -> sp::Result<Self, slint::PlatformError> {
                let mut compiler = sp::live_preview::Compiler::default();
                compiler.set_include_paths([#(#include_paths.into()),*].into_iter().collect());
                compiler.set_library_paths([#(#library_paths.into()),*].into_iter().collect());
                #(compiler.set_style(#style.to_string());)*
                let instance = sp::live_preview::LiveReloadingComponent::new(compiler, #main_file.into(), #component_name.into())?;
                let window_adapter = sp::WindowInner::from_pub(slint::ComponentHandle::window(instance.borrow().instance())).window_adapter();
                sp::Ok(Self(instance, window_adapter))
            }

            #(#property_and_callback_accessors)*
        }

        impl slint::ComponentHandle for #public_component_id {
            type WeakInner = sp::Weak<::core::cell::RefCell<sp::live_preview::LiveReloadingComponent>>;
            fn as_weak(&self) -> slint::Weak<Self> {
                slint::Weak::new(sp::Rc::downgrade(&self.0))
            }

            fn clone_strong(&self) -> Self {
                Self(self.0.clone(), self.1.clone())
            }

            fn upgrade_from_weak_inner(inner: &Self::WeakInner) -> sp::Option<Self> {
                let rc = inner.upgrade()?;
                let window_adapter = sp::WindowInner::from_pub(slint::ComponentHandle::window(rc.borrow().instance())).window_adapter();
                sp::Some(Self(rc, window_adapter))
            }

            fn run(&self) -> ::core::result::Result<(), slint::PlatformError> {
                self.show()?;
                slint::run_event_loop()
            }

            fn show(&self) -> ::core::result::Result<(), slint::PlatformError> {
                self.0.borrow().instance().show()
            }

            fn hide(&self) -> ::core::result::Result<(), slint::PlatformError> {
                self.0.borrow().instance().hide()
            }

            fn window(&self) -> &slint::Window {
                self.1.window()
            }

            fn global<'a, T: slint::Global<'a, Self>>(&'a self) -> T {
                T::get(&self)
            }
        }

        /// This is needed for the the internal tests  (eg `slint_testing::send_keyboard_string_sequence`)
        impl<X> ::core::convert::From<#public_component_id> for sp::VRc<sp::ItemTreeVTable, X>
            where Self : ::core::convert::From<sp::live_preview::ComponentInstance>
        {
            fn from(value: #public_component_id) -> Self {
                Self::from(slint::ComponentHandle::clone_strong(value.0.borrow().instance()))
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
                    self.0.borrow().invoke_global(#global_name, #prop_name, &[#(#args_name.into(),)*])
                        .try_into().unwrap_or_else(|_| panic!("Invalid return type for callback {}::{}", #global_name, #prop_name))
                }
            ));
            let on_ident = format_ident!("on_{}", prop_ident);
            property_and_callback_accessors.push(quote!(
                #[allow(dead_code)]
                pub fn #on_ident(&self, f: impl FnMut(#(#callback_args),*) -> #return_type + 'static) {
                    let f = ::core::cell::RefCell::new(f);
                    self.0.borrow_mut().set_global_callback(#global_name, #prop_name, sp::Rc::new(move |values| {
                        let [#(#args_name,)*] = values else { panic!("invalid number of argument for callback {}::{}", #global_name, #prop_name) };
                        (*f.borrow_mut())(#(#args_name.clone().try_into().unwrap_or_else(|_| panic!("invalid argument for callback {}::{}", #global_name, #prop_name)),)*).into()
                    }))
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
                    self.0.borrow().invoke_global(#global_name, #prop_name, &[#(#args_name.into(),)*])
                        .try_into().unwrap_or_else(|_| panic!("Invalid return type for function {}::{}", #global_name, #prop_name))
                }
            ));
        } else {
            let rust_property_type = rust_primitive_type(&p.ty).unwrap();
            let convert_to_value = convert_to_value_fn(&p.ty);
            let convert_from_value = convert_from_value_fn(&p.ty);

            let getter_ident = format_ident!("get_{}", prop_ident);
            property_and_callback_accessors.push(quote!(
                #[allow(dead_code)]
                pub fn #getter_ident(&self) -> #rust_property_type {
                    #convert_from_value(self.0.borrow().get_global_property(#global_name, #prop_name))
                        .unwrap_or_else(|_| panic!("Invalid property type for {}::{}", #global_name, #prop_name))
                }
            ));

            let setter_ident = format_ident!("set_{}", prop_ident);
            if !p.read_only {
                property_and_callback_accessors.push(quote!(
                    #[allow(dead_code)]
                    pub fn #setter_ident(&self, value: #rust_property_type) {
                        self.0.borrow_mut().set_global_property(#global_name, #prop_name, #convert_to_value(value))
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
        pub struct #public_component_id<'a>(&'a ::core::cell::RefCell<sp::live_preview::LiveReloadingComponent>);

        impl<'a> #public_component_id<'a> {
            #(#property_and_callback_accessors)*
        }
        #(pub type #aliases<'a> = #public_component_id<'a>;)*
        #(#getters)*
    )
}

/// returns a function that converts the type to a Value.
/// Normally, that would simply be `xxx.into()`, but for anonymous struct, we need an explicit conversion
fn convert_to_value_fn(ty: &Type) -> TokenStream {
    match ty {
        Type::Struct(s) if s.name.is_none() => {
            // anonymous struct is mapped to a tuple
            let names = s.fields.keys().map(|k| k.as_str()).collect::<Vec<_>>();
            let fields = names.iter().map(|k| ident(k)).collect::<Vec<_>>();
            quote!((|(#(#fields,)*)| {
                sp::live_preview::Value::Struct([#((#names.to_string(), sp::live_preview::Value::from(#fields)),)*].into_iter().collect())
            }))
        }
        Type::Array(a) if matches!(a.as_ref(), Type::Struct(s) if s.name.is_none()) => {
            let conf_fn = convert_to_value_fn(a.as_ref());
            quote!((|model: sp::ModelRc<_>| -> sp::live_preview::Value {
                sp::live_preview::Value::Model(sp::ModelRc::new(model.map(#conf_fn)))
            }))
        }
        _ => quote!(::core::convert::From::from),
    }
}

/// Returns a function that converts a Value to the type.
/// Normally, that would simply be `xxx.try_into()`, but for anonymous struct, we need an explicit conversion
fn convert_from_value_fn(ty: &Type) -> TokenStream {
    match ty {
        Type::Struct(s) if s.name.is_none() => {
            let names = s.fields.keys().map(|k| k.as_str()).collect::<Vec<_>>();
            // anonymous struct is mapped to a tuple
            quote!((|v: sp::live_preview::Value| -> sp::Result<_, ()> {
                let sp::live_preview::Value::Struct(s) = v else { return sp::Err(()) };
                sp::Ok((#(s.get_field(#names).ok_or(())?.clone().try_into().map_err(|_|())?,)*))
            }))
        }
        Type::Array(a) if matches!(a.as_ref(), Type::Struct(s) if s.name.is_none()) => {
            let conf_fn = convert_from_value_fn(a.as_ref());
            quote!((|v: sp::live_preview::Value| -> sp::Result<_, ()> {
                let sp::live_preview::Value::Model(model) = v else { return sp::Err(()) };
                sp::Ok(sp::ModelRc::new(model.map(|x| #conf_fn(x).unwrap_or_default())))
            }))
        }
        _ => quote!(::core::convert::TryFrom::try_from),
    }
}

fn generate_value_conversions(used_types: &[Type]) -> TokenStream {
    let r = used_types
        .iter()
        .filter_map(|ty| match ty {
            Type::Struct(s) => match s.as_ref() {
                Struct { fields, name: Some(name), node: Some(_), .. } => {
                    let ty = ident(name);
                    let convert_to_value = fields.values().map(convert_to_value_fn);
                    let convert_from_value = fields.values().map(convert_from_value_fn);
                    let field_names = fields.keys().map(|k| k.as_str()).collect::<Vec<_>>();
                    let fields = field_names.iter().map(|k| ident(k)).collect::<Vec<_>>();
                    Some(quote!{
                        impl From<#ty> for sp::live_preview::Value {
                            fn from(_value: #ty) -> Self {
                                Self::Struct([#((#field_names.to_string(), #convert_to_value(_value.#fields)),)*].into_iter().collect())
                            }
                        }
                        impl TryFrom<sp::live_preview::Value> for #ty {
                            type Error = ();
                            fn try_from(v: sp::live_preview::Value) -> sp::Result<Self, ()> {
                                match v {
                                    sp::live_preview::Value::Struct(_x) => {
                                        sp::Ok(Self {
                                            #(#fields: #convert_from_value(_x.get_field(#field_names).ok_or(())?.clone()).map_err(|_|())?,)*
                                        })
                                    }
                                    _ => sp::Err(()),
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
                let vals = en.values.iter().map(|v| ident(&crate::generator::to_pascal_case(v))).collect::<Vec<_>>();
                let val_names = en.values.iter().map(|v| v.as_str()).collect::<Vec<_>>();

                Some(quote!{
                    impl From<#ty> for sp::live_preview::Value {
                        fn from(v: #ty) -> Self {
                            fn to_string(v: #ty) -> String {
                                match v {
                                    #(#ty::#vals => #val_names.to_string(),)*
                                }
                            }
                            Self::EnumerationValue(#name.to_owned(), to_string(v))
                        }
                    }
                    impl TryFrom<sp::live_preview::Value> for #ty {
                        type Error = ();
                        fn try_from(v: sp::live_preview::Value) -> sp::Result<Self, ()> {
                            match v {
                                sp::live_preview::Value::EnumerationValue(enumeration, value) => {
                                    if enumeration != #name {
                                        return sp::Err(());
                                    }
                                    fn from_str(value: &str) -> sp::Result<#ty, ()> {
                                        match value {
                                            #(#val_names => Ok(#ty::#vals),)*
                                            _ => sp::Err(()),
                                        }
                                    }
                                    from_str(value.as_str()).map_err(|_| ())
                                }
                                _ => sp::Err(()),
                            }
                        }
                    }
                })
            },
            _ => None,
        });
    quote!(#(#r)*)
}
