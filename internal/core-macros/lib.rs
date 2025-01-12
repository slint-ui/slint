// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0
// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// Copyright © SixtyFPS GmbH <info@slint.dev>

#![doc = include_str!("README.md")]
#![doc(html_logo_url = "https://slint.dev/logo/slint-logo-square-light.svg")]

extern crate proc_macro;
use proc_macro::TokenStream;
use quote::quote;

mod slint_doc;

/// This derive macro is used with structures in the run-time library that are meant
/// to be exposed to the language. The structure is introspected for properties and fields
/// marked with the `rtti_field` attribute and generates run-time type information for use
/// with the interpreter.
/// In addition all `Property<T> foo` fields get a convenient getter function generated
/// that works on a `Pin<&Self>` receiver.
#[proc_macro_derive(SlintElement, attributes(rtti_field))]
pub fn slint_element(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);

    let fields = match &input.data {
        syn::Data::Struct(syn::DataStruct { fields: f @ syn::Fields::Named(..), .. }) => f,
        _ => {
            return syn::Error::new(
                input.ident.span(),
                "Only `struct` with named field are supported",
            )
            .to_compile_error()
            .into()
        }
    };

    let mut pub_prop_field_names = Vec::new();
    let mut pub_prop_field_names_normalized = Vec::new();
    let mut pub_prop_field_types = Vec::new();
    let mut property_names = Vec::new();
    let mut property_visibility = Vec::new();
    let mut property_types = Vec::new();

    for field in fields {
        if let Some(property_type) = property_type(&field.ty) {
            let name = field.ident.as_ref().unwrap();
            if matches!(field.vis, syn::Visibility::Public(_)) {
                pub_prop_field_names_normalized.push(normalize_identifier(name));
                pub_prop_field_names.push(name);
                pub_prop_field_types.push(&field.ty);
            }

            property_names.push(name);
            property_visibility.push(field.vis.clone());
            property_types.push(property_type);
        }
    }

    let (plain_field_names, plain_field_types): (Vec<_>, Vec<_>) = fields
        .iter()
        .filter(|f| {
            f.attrs.iter().any(|attr| {
                matches!(&attr.meta, syn::Meta::Path(path) if path.get_ident().map(|ident| *ident == "rtti_field").unwrap_or(false))
            })
        })
        .map(|f| (f.ident.as_ref().unwrap(), &f.ty))
        .unzip();
    let plain_field_names_normalized =
        plain_field_names.iter().map(|f| normalize_identifier(f)).collect::<Vec<_>>();

    let mut callback_field_names = Vec::new();
    let mut callback_field_names_normalized = Vec::new();
    let mut callback_args = Vec::new();
    let mut callback_rets = Vec::new();
    for field in fields {
        if let Some((arg, ret)) = callback_arg(&field.ty) {
            if matches!(field.vis, syn::Visibility::Public(_)) {
                let name = field.ident.as_ref().unwrap();
                callback_field_names_normalized.push(normalize_identifier(name));
                callback_field_names.push(name);
                callback_args.push(arg);
                callback_rets.push(ret);
            }
        }
    }

    let item_name = &input.ident;

    quote!(
        #[allow(clippy::nonstandard_macro_braces)]
        #[cfg(feature = "rtti")]
        impl BuiltinItem for #item_name {
            fn name() -> &'static str {
                stringify!(#item_name)
            }
            fn properties<Value: ValueType>() -> ::alloc::vec::Vec<(&'static str, &'static dyn PropertyInfo<Self, Value>)> {
                ::alloc::vec![#( {
                    const O : MaybeAnimatedPropertyInfoWrapper<#item_name, #pub_prop_field_types> =
                        MaybeAnimatedPropertyInfoWrapper(#item_name::FIELD_OFFSETS.#pub_prop_field_names);
                    (#pub_prop_field_names_normalized, (&O).as_property_info())
                } ),*]
            }
            fn fields<Value: ValueType>() -> ::alloc::vec::Vec<(&'static str, &'static dyn FieldInfo<Self, Value>)> {
                ::alloc::vec![#( {
                    const O : const_field_offset::FieldOffset<#item_name, #plain_field_types, const_field_offset::AllowPin> =
                        #item_name::FIELD_OFFSETS.#plain_field_names;
                    (#plain_field_names_normalized, &O as &'static dyn FieldInfo<Self, Value>)
                } ),*]
            }
            fn callbacks<Value: ValueType>() -> ::alloc::vec::Vec<(&'static str, &'static dyn CallbackInfo<Self, Value>)> {
                ::alloc::vec![#( {
                    const O : const_field_offset::FieldOffset<#item_name, Callback<#callback_args, #callback_rets>, const_field_offset::AllowPin> =
                         #item_name::FIELD_OFFSETS.#callback_field_names;
                    (#callback_field_names_normalized, &O as  &'static dyn CallbackInfo<Self, Value>)
                } ),*]
            }
        }

        impl #item_name {
            #(
                #property_visibility fn #property_names(self: core::pin::Pin<&Self>) -> #property_types {
                    Self::FIELD_OFFSETS.#property_names.apply_pin(self).get()
                }
            )*
        }
    )
    .into()
}

fn normalize_identifier(name: &syn::Ident) -> String {
    name.to_string().replace('_', "-")
}

// Try to match `Property<Foo>` on the syn tree and return Foo if found
fn property_type(ty: &syn::Type) -> Option<&syn::Type> {
    if let syn::Type::Path(syn::TypePath { path: syn::Path { segments, .. }, .. }) = ty {
        if let Some(syn::PathSegment {
            ident,
            arguments:
                syn::PathArguments::AngleBracketed(syn::AngleBracketedGenericArguments { args, .. }),
        }) = segments.first()
        {
            match args.first() {
                Some(syn::GenericArgument::Type(property_type)) if *ident == "Property" => {
                    return Some(property_type)
                }
                _ => {}
            }
        }
    }
    None
}

// Try to match `Callback<Args, Ret>` on the syn tree and return Args and Ret if found
fn callback_arg(ty: &syn::Type) -> Option<(&syn::Type, Option<&syn::Type>)> {
    if let syn::Type::Path(syn::TypePath { path: syn::Path { segments, .. }, .. }) = ty {
        if let Some(syn::PathSegment {
            ident,
            arguments:
                syn::PathArguments::AngleBracketed(syn::AngleBracketedGenericArguments { args, .. }),
        }) = segments.first()
        {
            if ident != "Callback" {
                return None;
            }
            let mut it = args.iter();
            let first = match it.next() {
                Some(syn::GenericArgument::Type(ty)) => ty,
                _ => return None,
            };
            let sec = match it.next() {
                Some(syn::GenericArgument::Type(ty)) => Some(ty),
                _ => None,
            };
            return Some((first, sec));
        }
    }
    None
}

/// An attribute macro that simply return its input and ignore any arguments
#[proc_macro_attribute]
pub fn identity(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// To be applied on any item that has documentation comment, it will convert link to `slint:Foo` to the link from the
/// documentation map from link-data.json
#[proc_macro_attribute]
pub fn slint_doc(_attr: TokenStream, item: TokenStream) -> TokenStream {
    use syn::visit_mut::VisitMut;
    let mut visitor = slint_doc::Visitor::new();
    let mut item = syn::parse_macro_input!(item as syn::Item);
    visitor.visit_item_mut(&mut item);
    assert!(visitor.1, "No slint link found");
    quote!(#item).into()
}

/// Same as `slint_doc` but for string literals instead of doc comments (useful for crate level documentation that cannot have an attribute)
#[proc_macro]
pub fn slint_doc_str(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::LitStr);
    let mut doc = input.value();
    let mut visitor = slint_doc::Visitor::new();
    visitor.process_string(&mut doc);
    assert!(visitor.1, "No slint link found");
    quote!(#doc).into()
}
