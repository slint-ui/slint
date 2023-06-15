// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.0 OR LicenseRef-Slint-commercial

//! The Low Level Representation module

mod expression;
pub use expression::*;
mod item_tree;
pub use item_tree::*;
pub mod lower_expression;
pub mod lower_to_item_tree;
pub mod pretty_print;

/// The optimisation passes over the LLR
pub mod optim_passes {
    pub mod count_property_use;
    mod inline_expressions;

    pub fn run_passes(root: &super::PublicComponent) {
        inline_expressions::inline_simple_expressions(root);
        count_property_use::count_property_use(root);
    }
}
