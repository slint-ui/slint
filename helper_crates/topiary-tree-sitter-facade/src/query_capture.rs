#[cfg(not(target_arch = "wasm32"))]
mod native {
    use crate::node::Node;
    use std::borrow::Cow;

    #[derive(Clone)]
    pub struct QueryCapture<'a> {
        pub(crate) inner: tree_sitter::QueryCapture<'a>,
    }

    impl QueryCapture<'_> {
        #[inline]
        pub fn node(&self) -> Node<'_> {
            self.inner.node.into()
        }

        #[inline]
        pub fn name<'s>(&self, capture_names: &'s [&str]) -> Cow<'s, str> {
            let index: usize = self.inner.index as usize;
            Cow::Borrowed(capture_names[index])
        }
    }

    impl std::fmt::Debug for QueryCapture<'_> {
        fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
            std::fmt::Debug::fmt(&self.inner, fmt)
        }
    }

    impl<'a> From<&tree_sitter::QueryCapture<'a>> for QueryCapture<'a> {
        #[inline]
        fn from(inner: &tree_sitter::QueryCapture<'a>) -> Self {
            Self { inner: *inner }
        }
    }

    impl<'tree> From<tree_sitter::QueryCapture<'tree>> for QueryCapture<'tree> {
        #[inline]
        fn from(inner: tree_sitter::QueryCapture<'tree>) -> Self {
            Self { inner }
        }
    }

    impl std::panic::RefUnwindSafe for QueryCapture<'_> {}

    impl Unpin for QueryCapture<'_> {}

    impl std::panic::UnwindSafe for QueryCapture<'_> {}
}

#[cfg(not(target_arch = "wasm32"))]
pub use native::*;

#[cfg(target_arch = "wasm32")]
mod wasm {
    use crate::node::Node;
    use std::borrow::Cow;

    #[derive(Clone)]
    pub struct QueryCapture<'a> {
        pub(crate) inner: topiary_web_tree_sitter_sys::QueryCapture,
        pub(crate) phantom: std::marker::PhantomData<&'a ()>,
    }

    impl<'a> QueryCapture<'a> {
        #[inline]
        pub fn node(&self) -> Node {
            self.inner.node().into()
        }

        #[inline]
        pub fn name(&self, _capture_names: &[&str]) -> Cow<str> {
            Cow::Owned(self.inner.name().as_string().unwrap())
        }
    }

    impl<'a> std::fmt::Debug for QueryCapture<'a> {
        fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
            std::fmt::Debug::fmt(&self.inner, fmt)
        }
    }

    impl<'a> From<topiary_web_tree_sitter_sys::QueryCapture> for QueryCapture<'a> {
        #[inline]
        fn from(inner: topiary_web_tree_sitter_sys::QueryCapture) -> Self {
            let phantom = std::marker::PhantomData;
            Self { inner, phantom }
        }
    }

    impl<'a> std::panic::RefUnwindSafe for QueryCapture<'a> {}

    impl<'a> Unpin for QueryCapture<'a> {}

    impl<'a> std::panic::UnwindSafe for QueryCapture<'a> {}
}

#[cfg(target_arch = "wasm32")]
pub use wasm::*;
