/*!
This crate allow to get the offset of a field of a structure in a const or static context.

To be used re-exported from the `const_field_offset` crate

*/
extern crate proc_macro;

use proc_macro::TokenStream;
use quote::{format_ident, quote, quote_spanned};
use syn::{parse_macro_input, spanned::Spanned, DeriveInput};

/**

The macro FieldOffsets adds a `const fn field_offsets()` associated function to the struct, that
returns an object which has fields with the same name as the fields of the original struct,
each field is of type `const_field_offset::FieldOffset`

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

## limitations

Only work with named #[repr(C)] structures.

## Attributes

### `pin`

Add a `PinnedFlag` to the FieldOffset.

In order for this to be safe, the macro will add code to prevent a
custom `Drop` or `Unpin` implementation.

```rust
use const_field_offset::*;
#[repr(C)]
#[derive(FieldOffsets)]
#[pin]
struct Foo {
    field_1 : u8,
    field_2 : u32,
}

const FIELD_2 : FieldOffset<Foo, u32, PinnedFlag> = Foo::field_offsets().field_2;
let pin_box = Box::pin(Foo{field_1: 1, field_2: 2});
assert_eq!(*FIELD_2.apply_pin(pin_box.as_ref()), 2);
```

### `const-field-offset`

In case the `const-field-offset` crate is re-exported, it is possible to
specify the crate name using the `const_field_offset` attribute.

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
#[proc_macro_derive(FieldOffsets, attributes(const_field_offset, pin))]
pub fn const_field_offset(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let mut has_repr_c = false;
    let mut crate_ = quote!(const_field_offset);
    let mut pin = false;
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
            } else if i == "const_field_offset" {
                let mut token_it = a.tokens.clone().into_iter();
                if let (Some(proc_macro2::TokenTree::Group(g)), None) =
                    (token_it.next(), token_it.next())
                {
                    if g.delimiter() == proc_macro2::Delimiter::Parenthesis {
                        crate_ = g.stream();
                        continue;
                    }
                }
                return TokenStream::from(
                    quote_spanned! {a.span() => compile_error!{"const_field_offset attreibute must be a crate name"}},
                );
            } else if i == "pin" {
                pin = true;
            }
        }
    }
    if !has_repr_c {
        return TokenStream::from(
            quote! {compile_error!{"FieldOffsets inly work if the structure repr(C)"}},
        );
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

    let doc = format!(
        "Helper struct containing the offsets of the fields of the struct `{}`",
        struct_name
    );

    let (ensure_pin_safe, pin_flag, new_from_offset) = if !pin {
        (None, None, quote!(new_from_offset))
    } else {
        let drop_trait_ident = format_ident!("{}MustNotImplDrop", struct_name);
        (
            Some(quote! {
                /// Make sure that Drop is not implemented
                trait #drop_trait_ident {}
                impl<T: ::core::ops::Drop> #drop_trait_ident for T {}
                impl #drop_trait_ident for #struct_name {}

                /// Make sure that Unpin is not implemented
                pub struct __MustNotImplUnpin<'__dummy_lifetime> (
                    #(#types, )*
                    ::core::marker::PhantomData<&'__dummy_lifetime ()>
                );
                impl<'__dummy_lifetime> Unpin for #struct_name where __MustNotImplUnpin<'__dummy_lifetime> : Unpin {};
            }),
            Some(quote!(#crate_::PinnedFlag)),
            quote!(new_from_offset_pinned),
        )
    };
    // Build the output, possibly using quasi-quotation
    let expanded = quote! {
        #[doc = #doc]
        ///
        /// Generated from the derive macro `const-field-offset::FieldOffsets`
        #[allow(missing_docs, non_camel_case_types)]
        pub struct #field_struct_name {
            #(#vis #fields : #crate_::FieldOffset<#struct_name, #types, #pin_flag>,)*
        }

        impl #struct_name {
            /// Return a struct containing the offset of for the fields of this struct
            pub const fn field_offsets() -> #field_struct_name {
                #ensure_pin_safe;
                let mut len = 0usize;
                #field_struct_name {
                    #( #fields : {
                        let align = ::core::mem::align_of::<#types>();
                        // from Layout::padding_needed_for which is not yet stable
                        let len_rounded_up  = len.wrapping_add(align).wrapping_sub(1) & !align.wrapping_sub(1);
                        len = len_rounded_up + ::core::mem::size_of::<#types>();
                        /// Safety: According to the rules of repr(C), this is the right offset
                        unsafe { #crate_::FieldOffset::<#struct_name, #types, _>::#new_from_offset(len_rounded_up) }
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

/**
```compile_fail
use const_field_offset::*;
#[derive(FieldOffsets)]
#[repr(C)]
#[pin]
struct Foo {
    x: u32,
}

impl Drop for Foo {
    fn drop(&mut self) {}
}
```
*/
#[cfg(doctest)]
const _PIN_NO_DROP: u32 = 0;

/**
```compile_fail
use const_field_offset::*;
#[derive(FieldOffsets)]
#[repr(C)]
#[pin]
struct Foo {
    q: std::marker::PhantomPinned,
    x: u32,
}

impl Unpin for Foo {}
```
*/
#[cfg(doctest)]
const _PIN_NO_UNPIN: u32 = 0;
