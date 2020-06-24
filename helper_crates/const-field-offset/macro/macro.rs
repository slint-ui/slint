/*!

This crate allow to get the offset of a field of a structure in a const or static context.

```rust
use const_field_offset::FieldOffsets;
#[repr(C)]
#[derive(FieldOffsets)]
struct Foo {
    field_1 : u8,
    field_2 : u32,
}

const FOO : usize = Foo::field_offsets().field_2.get_byte_offset();
assert_eq!(FOO, 4);

// This would not work on stable rust at the moment (rust 1.43)
// const FOO : usize = memofsets::offsetof!(Foo, field_2);
```

The macro FieldOffsets adds a `const fn field_offsets()` associated function to the struct, that
returns an object which has a bunch of usize fields with the same name as the fields of the
original struct.

## limitations

Only work with named #[repr(C)] structures.

## Attributes

It is possible to specify the crate name using the `const_field_offset` attribute.

```rust
// suppose you re-export the const_field_offset create from a different module
mod xxx { pub use const_field_offset as cfo; }
#[repr(C)]
#[derive(xxx::cfo::FieldOffsets)]
#[const_field_offset(xxx::cfo)]
struct Foo {
    field_1 : u8,
    field_2 : u32,
}
```

*/

extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, spanned::Spanned, DeriveInput};

#[proc_macro_derive(FieldOffsets, attributes(const_field_offset))]
pub fn const_field_offset(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let mut has_repr_c = false;
    for a in &input.attrs {
        if let Some(i) = a.path.get_ident() {
            if i == "repr" {
                match a.tokens.to_string().as_str() {
                    "(C)" => has_repr_c = true,
                    "(packed)" => {
                        return TokenStream::from(quote!(
                            compile_error! {"FieldOffsets does not work on #[repr(packed)]"}
                        ))
                    }
                    _ => (),
                }
            }
        }
    }
    if !has_repr_c {
        return TokenStream::from(
            quote! {compile_error!{"FieldOffsets inly work if the structure repr(C)"}},
        );
    }

    if input.attrs.iter().any(|a| {
        if let Some(i) = a.path.get_ident() {
            i == "repr" && a.tokens.to_string() == "(packed)"
        } else {
            false
        }
    }) {
        return TokenStream::from(quote! {compile_error!{"Does not work if "}});
    }

    let struct_name = input.ident;
    let field_struct_name = quote::format_ident!("{}FieldsOffsets", struct_name);

    let (fields, types, vis) = if let syn::Data::Struct(s) = &input.data {
        if let syn::Fields::Named(n) = &s.fields {
            let (f, tv): (Vec<_>, Vec<_>) =
                n.named.iter().map(|f| (&f.ident, (&f.ty, &f.vis))).unzip();
            let (t, v): (Vec<_>, Vec<_>) = tv.into_iter().unzip();
            (f, t, v)
        } else {
            return TokenStream::from(quote! {compile_error!{"Only work for named fields"}});
        }
    } else {
        return TokenStream::from(quote! {compile_error!("Only work for struct")});
    };

    let crate_ = input
        .attrs
        .iter()
        .find(|a| a.path.get_ident().map_or(false, |i| i == "const_field_offset"))
        .map(|a| {
            a.tokens
                .clone()
                .into_iter()
                .next()
                .and_then(|tt| match tt {
                    proc_macro2::TokenTree::Group(g) => {
                        if g.delimiter() == proc_macro2::Delimiter::Parenthesis {
                            Some(g.stream())
                        } else {
                            None
                        }
                    }
                    _ => None,
                })
                .ok_or_else(|| {
                    syn::Error::new(a.span(), "The argument must be a path to the crate")
                })
        })
        .unwrap_or_else(|| Ok(quote!(const_field_offset)));
    let crate_ = match crate_ {
        Ok(crate_) => crate_,
        Err(e) => return e.to_compile_error().into(),
    };

    let doc = format!(
        "Helper struct containing the offsets of the fields of the struct `{}`",
        struct_name
    );

    // Build the output, possibly using quasi-quotation
    let expanded = quote! {
        #[doc = #doc]
        ///
        /// Generated from the derive macro `const-field-offset::FieldOffsets`
        #[allow(missing_docs, non_camel_case_types)]
        pub struct #field_struct_name {
            #(#vis #fields : #crate_::FieldOffset<#struct_name, #types>,)*
        }

        impl #struct_name {
            /// Return a struct containing the offset of for the fields of this struct
            pub const fn field_offsets() -> #field_struct_name {
                let mut len = 0usize;
                #field_struct_name {
                    #( #fields : {
                        let align = ::core::mem::align_of::<#types>();
                        // from Layout::padding_needed_for which is not yet stable
                        let len_rounded_up  = len.wrapping_add(align).wrapping_sub(1) & !align.wrapping_sub(1);
                        len = len_rounded_up + ::core::mem::size_of::<#types>();
                        /// Safety: According to the rules of repr(C), this is the right offset
                        unsafe { #crate_::FieldOffset::<#struct_name, #types>::new_from_offset(len_rounded_up) }
                    }, )*
                }
            }
        }
    };

    // Hand the output tokens back to the compiler
    TokenStream::from(expanded)
}

/**
```compile_fail
use const_field_offset::*;
#[derive(FieldOffsets)]
struct Foo {
    x: u32,
}
```
*/
#[cfg(doctest)]
const _NO_REPR_C: u32 = 0;

/**
```compile_fail
use const_field_offset::*;
#[derive(FieldOffsets)]
#[repr(C)]
#[repr(packed)]
struct Foo {
    x: u32,
}
```
*/
#[cfg(doctest)]
const _REPR_PACKED: u32 = 0;
