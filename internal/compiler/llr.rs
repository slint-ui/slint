// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore optim
//! The Low Level Representation module

pub mod debug_info;
mod expression;
pub use expression::*;
mod item_tree;
pub use item_tree::*;
pub mod lower_expression;
pub mod lower_layout_expression;
pub mod lower_to_item_tree;
pub mod pretty_print;

// The LLR is meant to be shared across threads (compiled on a worker thread,
// interpreted on the UI thread behind an `Arc`), so the whole graph must be
// Send + Sync. Its interior mutability uses `AtomicRefCell`/`AtomicUsize`
// rather than `RefCell`/`Cell` for that reason.
const _: fn() = || {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<CompilationUnit>();
};

/// The optimization passes over the LLR
pub mod optim_passes {
    pub mod count_property_use;
    mod inline_expressions;
    mod remove_unused;

    pub fn run_passes(root: &mut super::CompilationUnit) {
        count_property_use::count_property_use(root);
        inline_expressions::inline_simple_expressions(root);
        remove_unused::remove_unused(root);
    }
}
