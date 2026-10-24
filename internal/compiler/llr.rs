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
mod native_class_selection;
pub mod pretty_print;

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

/// Compiles `source` and lowers it to the LLR, which runs the optimization passes.
#[cfg(test)]
pub(crate) fn lower_test_source(source: &str) -> CompilationUnit {
    let mut config = crate::CompilerConfiguration::new(crate::generator::OutputFormat::Interpreter);
    config.style = Some("fluent".into());
    let mut diags = crate::diagnostics::BuildDiagnostics::default();
    let doc_node = crate::parser::parse(
        source.into(),
        Some(crate::source_path::SourcePath::new("t.slint")),
        &mut diags,
    );
    let (doc, diag, _) =
        spin_on::spin_on(crate::compile_syntax_node(doc_node, diags, config.clone()));
    assert!(!diag.has_errors(), "compile error: {:#?}", diag.to_string_vec());
    lower_to_item_tree::lower_to_item_tree(&doc, &config)
}
