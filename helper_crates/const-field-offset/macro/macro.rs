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

*/

extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(FieldOffsets)]
pub fn const_field_offset(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    if !input.attrs.iter().any(|a| {
        if let Some(i) = a.path.get_ident() {
            i == "repr" && a.tokens.to_string() == "(C)"
        } else {
            false
        }
    }) {
        return TokenStream::from(quote! {compile_error!{"Only work if repr(C)"}});
    };

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

    let crate_ = quote!(const_field_offset);

    // Build the output, possibly using quasi-quotation
    let expanded = quote! {
        pub struct #field_struct_name {
            #(#vis #fields : #crate_::FieldOffset<#struct_name, #types>,)*
        }

        impl #struct_name {
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
