//! Rust bindings for the tree-sitter parser for the Slint language.
//!
//! ```
//! let mut parser = tree_sitter::Parser::new();
//! parser
//!     .set_language(&i_tree_sitter_slint::LANGUAGE.into())
//!     .expect("Error loading the Slint tree-sitter grammar");
//! ```

use tree_sitter_language::LanguageFn;

unsafe extern "C" {
    fn tree_sitter_slint() -> *const ();
}

/// The tree-sitter language for Slint.
pub const LANGUAGE: LanguageFn = unsafe { LanguageFn::from_raw(tree_sitter_slint) };

/// The contents of the grammar's `node-types.json`.
pub const NODE_TYPES: &str = include_str!("../../src/node-types.json");

#[cfg(test)]
mod tests {
    #[test]
    fn can_load_language() {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&super::LANGUAGE.into())
            .expect("Error loading the Slint tree-sitter grammar");
    }
}
