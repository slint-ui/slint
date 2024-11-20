// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/*! module for the TypeScript code generator
*/

/// This module contains some data structure that helps represent a TypeScript code.
/// It is then rendered into an actual TypeScript text using the Display trait
mod typescript_ast {
    use std::fmt::Display;

    /// A full TypeScript file
    #[derive(Default, Debug)]
    pub struct File {}

    impl Display for File {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            writeln!(f, "// This file is auto-generated")?;
            Ok(())
        }
    }
}

use crate::object_tree::Document;
use crate::CompilerConfiguration;
use typescript_ast::*;

pub fn generate(
    doc: &Document,
    compiler_config: &CompilerConfiguration,
) -> std::io::Result<impl std::fmt::Display> {
    let file = File {};

    Ok(file)
}
