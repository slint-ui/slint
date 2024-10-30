// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The Low Level Representation module

mod expression;
pub use expression::*;
mod item_tree;
pub use item_tree::*;
pub mod lower_expression;
pub mod lower_to_item_tree;
pub mod pretty_print;
#[cfg(feature = "bundle-translations")]
pub mod translations;

/// The optimization passes over the LLR
pub mod optim_passes {
    pub mod count_property_use;
    mod inline_expressions;

    pub fn run_passes(root: &super::CompilationUnit) {
        count_property_use::count_property_use(root);
        inline_expressions::inline_simple_expressions(root);
    }
}
