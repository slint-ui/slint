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
    use tree_sitter::{Node, Parser, Tree};

    fn parse(source: &str) -> Tree {
        let mut parser = Parser::new();
        parser
            .set_language(&super::LANGUAGE.into())
            .expect("Error loading the Slint tree-sitter grammar");
        parser.parse(source, None).expect("source should parse")
    }

    fn find_first<'tree>(node: Node<'tree>, kind: &str) -> Option<Node<'tree>> {
        if node.kind() == kind {
            return Some(node);
        }

        for index in 0..node.child_count() {
            if let Some(child) = node.child(index as u32)
                && let Some(found) = find_first(child, kind)
            {
                return Some(found);
            }
        }

        None
    }

    fn count_kind(node: Node<'_>, kind: &str) -> usize {
        usize::from(node.kind() == kind)
            + (0..node.child_count())
                .filter_map(|index| node.child(index as u32))
                .map(|child| count_kind(child, kind))
                .sum::<usize>()
    }

    #[test]
    fn can_load_language() {
        let mut parser = Parser::new();
        parser
            .set_language(&super::LANGUAGE.into())
            .expect("Error loading the Slint tree-sitter grammar");
    }

    #[test]
    fn parses_changed_as_callback_name_and_function_call() {
        let source = r#"
export component Demo inherits Window {
    callback changed(int);
    changed => {
    }
    changed(delta) => {
        root.changed(+1);
        root.changed;
        changed(+2);
    }
    changed value => {
    }
}
"#;

        let tree = parse(source);
        let root = tree.root_node();
        assert!(!root.has_error());

        let callback = find_first(root, "callback").expect("callback declaration");
        let callback_name = callback.child_by_field_name("name").expect("callback name");
        assert_eq!(callback_name.kind(), "simple_identifier");
        assert_eq!(callback_name.utf8_text(source.as_bytes()).expect("utf-8"), "changed");

        let member_access = find_first(root, "member_access").expect("member access");
        let member_call = find_first(member_access, "function_call").expect("member call");
        assert_eq!(
            member_call.child_by_field_name("arguments").expect("member arguments").kind(),
            "arguments"
        );

        assert_eq!(count_kind(root, "callback_event"), 2);
        assert_eq!(count_kind(root, "changed_event"), 1);
        assert_eq!(count_kind(root, "function_call"), 2);
        assert_eq!(count_kind(root, "member_access"), 2);
    }
}
