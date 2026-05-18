#[cfg(not(target_arch = "wasm32"))]
mod native {
    use crate::{
        input_edit::InputEdit, language::LanguageRef, node::Node, range::Range,
        tree_cursor::TreeCursor,
    };

    #[derive(Clone)]
    pub struct Tree {
        pub(crate) inner: tree_sitter::Tree,
    }

    impl Tree {
        pub fn edit(&mut self, edit: &InputEdit) {
            self.inner.edit(&edit.inner);
        }

        pub fn changed_ranges(&self, other: &Tree) -> impl ExactSizeIterator<Item = Range> {
            self.inner
                .changed_ranges(&other.inner)
                .map(|inner| Range { inner })
        }

        pub fn language(&self) -> LanguageRef<'_> {
            Into::into(self.inner.language())
        }

        pub fn root_node(&self) -> Node<'_> {
            self.inner.root_node().into()
        }

        pub fn walk(&self) -> TreeCursor<'_> {
            self.inner.walk().into()
        }
    }

    impl std::fmt::Debug for Tree {
        fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
            std::fmt::Debug::fmt(&self.inner, fmt)
        }
    }

    impl From<tree_sitter::Tree> for Tree {
        #[inline]
        fn from(inner: tree_sitter::Tree) -> Self {
            Self { inner }
        }
    }

    impl std::panic::RefUnwindSafe for Tree {}

    unsafe impl Send for Tree {}

    unsafe impl Sync for Tree {}

    impl Unpin for Tree {}

    impl std::panic::UnwindSafe for Tree {}
}

#[cfg(not(target_arch = "wasm32"))]
pub use native::*;

#[cfg(target_arch = "wasm32")]
mod wasm {
    use crate::{
        input_edit::InputEdit, language::Language, node::Node, range::Range,
        tree_cursor::TreeCursor,
    };
    use wasm_bindgen::JsCast;

    pub struct Tree {
        pub(crate) inner: topiary_web_tree_sitter_sys::Tree,
    }

    impl Tree {
        pub fn edit(&mut self, edit: &InputEdit) {
            let edit = {
                let start_index = edit.start_byte();
                let old_end_index = edit.old_end_byte();
                let new_end_index = edit.new_end_byte();
                let start_position = edit.start_position().inner;
                let old_end_position = edit.old_end_position().inner;
                let new_end_position = edit.new_end_position().inner;
                topiary_web_tree_sitter_sys::Edit::new(
                    start_index,
                    old_end_index,
                    new_end_index,
                    &start_position,
                    &old_end_position,
                    &new_end_position,
                )
            };
            self.inner.edit(&edit);
        }

        // FIXME: implement bindings upstream first
        pub fn changed_ranges(&self, other: &Tree) -> impl ExactSizeIterator<Item = Range> {
            self.inner
                .get_changed_ranges(&other.inner)
                .into_vec()
                .into_iter()
                .map(|value| {
                    value
                        .unchecked_into::<topiary_web_tree_sitter_sys::Range>()
                        .into()
                })
        }

        pub fn language(&self) -> Language {
            self.inner.get_language().into()
        }

        pub fn root_node(&self) -> Node<'_> {
            self.inner.root_node().into()
        }

        pub fn walk(&self) -> TreeCursor<'_> {
            self.inner.walk().into()
        }
    }

    impl Clone for Tree {
        fn clone(&self) -> Tree {
            self.inner.copy().into()
        }
    }

    impl std::fmt::Debug for Tree {
        fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
            std::fmt::Debug::fmt(&self.inner, fmt)
        }
    }

    impl Drop for Tree {
        fn drop(&mut self) {
            self.inner.delete();
        }
    }

    impl From<topiary_web_tree_sitter_sys::Tree> for Tree {
        #[inline]
        fn from(inner: topiary_web_tree_sitter_sys::Tree) -> Self {
            Self { inner }
        }
    }

    impl std::panic::RefUnwindSafe for Tree {}

    unsafe impl Send for Tree {}

    unsafe impl Sync for Tree {}

    impl Unpin for Tree {}

    impl std::panic::UnwindSafe for Tree {}
}

#[cfg(target_arch = "wasm32")]
pub use wasm::*;
