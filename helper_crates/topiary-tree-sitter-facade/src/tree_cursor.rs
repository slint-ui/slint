#[cfg(not(target_arch = "wasm32"))]
mod native {
    use crate::node::Node;
    use std::{borrow::Cow, convert::TryFrom};

    #[derive(Clone)]
    pub struct TreeCursor<'a> {
        pub(crate) inner: tree_sitter::TreeCursor<'a>,
    }

    impl<'a> TreeCursor<'a> {
        #[inline]
        pub fn field_id(&self) -> Option<std::num::NonZeroU16> {
            self.inner.field_id()
        }

        #[inline]
        pub fn field_name(&self) -> Option<Cow<'_, str>> {
            self.inner.field_name().map(Into::into)
        }

        #[inline]
        pub fn goto_first_child(&mut self) -> bool {
            self.inner.goto_first_child()
        }

        #[inline]
        pub fn goto_first_child_for_byte(&mut self, index: u32) -> Option<u32> {
            let index = index as usize;
            self.inner
                .goto_first_child_for_byte(index)
                .map(|i| u32::try_from(i).unwrap())
        }

        #[inline]
        pub fn goto_next_sibling(&mut self) -> bool {
            self.inner.goto_next_sibling()
        }

        #[inline]
        pub fn goto_parent(&mut self) -> bool {
            self.inner.goto_parent()
        }

        #[inline]
        pub fn node(&self) -> Node<'a> {
            let inner = self.inner.node();
            Node { inner }
        }

        #[inline]
        pub fn reset(&mut self, node: Node<'a>) {
            self.inner.reset(node.inner);
        }
    }

    impl<'a> From<tree_sitter::TreeCursor<'a>> for TreeCursor<'a> {
        #[inline]
        fn from(inner: tree_sitter::TreeCursor<'a>) -> Self {
            Self { inner }
        }
    }

    impl std::panic::RefUnwindSafe for TreeCursor<'_> {}

    impl Unpin for TreeCursor<'_> {}

    impl std::panic::UnwindSafe for TreeCursor<'_> {}
}

#[cfg(not(target_arch = "wasm32"))]
pub use native::*;

#[cfg(target_arch = "wasm32")]
mod wasm {
    use crate::node::Node;
    use std::borrow::Cow;

    #[derive(Clone)]
    pub struct TreeCursor<'a> {
        pub(crate) inner: topiary_web_tree_sitter_sys::TreeCursor,
        pub(crate) phantom: std::marker::PhantomData<&'a ()>,
    }

    impl<'a> TreeCursor<'a> {
        #[inline]
        pub fn field_id(&self) -> Option<u16> {
            self.inner.current_field_id()
        }

        #[inline]
        pub fn field_name(&self) -> Option<Cow<str>> {
            self.inner
                .current_field_name()
                .map(|name| From::<String>::from(name.into()))
        }

        #[inline]
        pub fn goto_first_child(&mut self) -> bool {
            self.inner.goto_first_child()
        }

        #[inline]
        pub fn goto_next_sibling(&mut self) -> bool {
            self.inner.goto_next_sibling()
        }

        #[inline]
        pub fn goto_parent(&mut self) -> bool {
            self.inner.goto_parent()
        }

        #[inline]
        pub fn node(&self) -> Node<'a> {
            let inner = self.inner.current_node();
            let phantom = std::marker::PhantomData;
            Node { inner, phantom }
        }

        #[inline]
        pub fn reset(&mut self, node: Node<'a>) {
            self.inner.reset(&node.inner);
        }
    }

    impl<'a> Drop for TreeCursor<'a> {
        fn drop(&mut self) {
            self.inner.delete();
        }
    }

    impl<'a> From<topiary_web_tree_sitter_sys::TreeCursor> for TreeCursor<'a> {
        #[inline]
        fn from(inner: topiary_web_tree_sitter_sys::TreeCursor) -> Self {
            let phantom = std::marker::PhantomData;
            Self { inner, phantom }
        }
    }

    impl<'a> std::panic::RefUnwindSafe for TreeCursor<'a> {}

    impl<'a> Unpin for TreeCursor<'a> {}

    impl<'a> std::panic::UnwindSafe for TreeCursor<'a> {}
}

#[cfg(target_arch = "wasm32")]
pub use wasm::*;
