// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Code generator for the Slint SC (safety-critical) runtime.

use crate::CompilerConfiguration;
use crate::object_tree::Document;
use itertools::Either;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

/// Public entry point called from `generator::generate`.
pub fn generate(
    doc: &Document,
    _compiler_config: &CompilerConfiguration,
) -> std::io::Result<TokenStream> {
    let mut output = TokenStream::new();

    for (export_name, export) in doc.exports.iter() {
        let Either::Left(component) = export else { continue };
        if component.is_global() {
            continue;
        }
        let name = format_ident!("{}", export_name.name.as_str());
        output.extend(quote! {
            pub struct #name;
            impl #name {
                pub fn new() -> Self {
                    Self
                }
            }
        });
    }

    Ok(output)
}
