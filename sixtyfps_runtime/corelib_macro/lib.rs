/*!
    This crate constains the internal procedural macro
    used by the sixtyfps corelib crate
*/

extern crate proc_macro;
use core::iter::IntoIterator;
use core::str::FromStr;
use proc_macro::{TokenStream, TokenTree};

#[proc_macro_derive(BuiltinComponent)]
pub fn parser_test(_attr: TokenStream, item: TokenStream) -> TokenStream {
    result
}
