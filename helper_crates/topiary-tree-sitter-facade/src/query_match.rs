#[cfg(not(target_arch = "wasm32"))]
mod native {
    use crate::query_capture::QueryCapture;
    pub use tree_sitter::QueryMatches;

    pub trait QueryMatch<'tree> {
        fn pattern_index(&self) -> usize;

        fn captures(&self) -> impl ExactSizeIterator<Item = QueryCapture<'tree>>;
    }

    impl<'tree> QueryMatch<'tree> for tree_sitter::QueryMatch<'tree, 'tree> {
        #[inline]
        fn pattern_index(&self) -> usize {
            self.pattern_index
        }

        #[inline]
        fn captures(&self) -> impl ExactSizeIterator<Item = QueryCapture<'tree>> {
            self.captures.iter().map(Into::into)
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub use native::*;

#[cfg(target_arch = "wasm32")]
mod wasm {
    use crate::query_capture::QueryCapture;
    use std::convert::TryInto;
    use wasm_bindgen::JsCast;

    #[derive(Clone)]
    pub struct QueryMatch<'tree> {
        pub(crate) inner: topiary_web_tree_sitter_sys::QueryMatch,
        pub(crate) phantom: std::marker::PhantomData<&'tree ()>,
    }

    impl<'tree> QueryMatch<'tree> {
        #[inline]
        pub fn pattern_index(&self) -> usize {
            // On WASM32, usize is the same as u32, so the unwrap is safe
            self.inner.pattern().try_into().unwrap()
        }

        #[inline]
        pub fn captures(&self) -> impl ExactSizeIterator<Item = QueryCapture<'tree>> + 'tree {
            self.inner.captures().into_vec().into_iter().map(|value| {
                value
                    .unchecked_into::<topiary_web_tree_sitter_sys::QueryCapture>()
                    .into()
            })
        }
    }

    impl<'tree> From<topiary_web_tree_sitter_sys::QueryMatch> for QueryMatch<'tree> {
        #[inline]
        fn from(inner: topiary_web_tree_sitter_sys::QueryMatch) -> Self {
            let phantom = std::marker::PhantomData;
            Self { inner, phantom }
        }
    }

    impl<'tree> std::panic::RefUnwindSafe for QueryMatch<'tree> {}

    impl<'tree> Unpin for QueryMatch<'tree> {}

    impl<'tree> std::panic::UnwindSafe for QueryMatch<'tree> {}
}

#[cfg(target_arch = "wasm32")]
pub use wasm::*;
