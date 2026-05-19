// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Stub code generator for the Slint SC (safety-critical) runtime.

use crate::CompilerConfiguration;
use crate::object_tree::Document;
use proc_macro2::TokenStream;

/// Public entry point called from `generator::generate`.
pub fn generate(
    _doc: &Document,
    _compiler_config: &CompilerConfiguration,
) -> std::io::Result<TokenStream> {
    Ok(TokenStream::new())
}
