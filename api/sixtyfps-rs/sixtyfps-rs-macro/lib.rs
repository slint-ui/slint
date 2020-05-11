extern crate proc_macro;
use proc_macro::TokenStream;
use quote::quote;

#[proc_macro]
pub fn sixtyfps(_item: TokenStream) -> TokenStream {
    quote!(
        #[derive(Default)]
        struct SuperSimple;
        impl SuperSimple {
            fn run(&self) {
                println!("Hello world");
            }

        }
    )
    .into()
}
