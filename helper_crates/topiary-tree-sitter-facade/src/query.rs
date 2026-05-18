#[cfg(not(target_arch = "wasm32"))]
mod native {
    use crate::{
        error::QueryError, language::Language, node::Node, query_cursor::QueryCursor,
        query_predicate::QueryPredicate,
    };
    use streaming_iterator::StreamingIterator;

    pub struct Query {
        pub(crate) inner: tree_sitter::Query,
    }

    impl Query {
        #[inline]
        pub fn new(language: &Language, source: &str) -> Result<Self, QueryError> {
            let inner = tree_sitter::Query::new(&language.inner, source)?;
            Ok(Self { inner })
        }

        #[inline]
        pub fn matches<
            'query,
            'cursor: 'query,
            'tree: 'query,
            T: tree_sitter::TextProvider<I> + 'query,
            I: AsRef<[u8]> + 'query,
        >(
            &'query self,
            node: &Node<'tree>,
            source: T,
            cursor: &'cursor mut QueryCursor,
        ) -> impl StreamingIterator<Item = tree_sitter::QueryMatch<'query, 'tree>> {
            cursor.inner.matches(&self.inner, node.inner, source)
        }

        #[inline]
        pub fn capture_names(&self) -> Vec<&str> {
            self.inner.capture_names().to_vec()
        }

        #[inline]
        pub fn general_predicates(&self, index: usize) -> Vec<QueryPredicate<'_>> {
            self.inner
                .general_predicates(index)
                .iter()
                .map(Into::into)
                .collect()
        }

        #[inline]
        pub fn pattern_count(&self) -> usize {
            self.inner.pattern_count()
        }

        #[inline]
        pub fn disable_pattern(&mut self, index: usize) {
            self.inner.disable_pattern(index)
        }

        #[inline]
        pub fn start_byte_for_pattern(&self, pattern_index: usize) -> usize {
            self.inner.start_byte_for_pattern(pattern_index)
        }

        #[inline]
        pub fn end_byte_for_pattern(&self, pattern_index: usize) -> usize {
            self.inner.end_byte_for_pattern(pattern_index)
        }
    }

    impl std::fmt::Debug for Query {
        fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
            std::fmt::Debug::fmt(&self.inner, fmt)
        }
    }

    impl From<tree_sitter::Query> for Query {
        #[inline]
        fn from(inner: tree_sitter::Query) -> Self {
            Self { inner }
        }
    }

    impl std::panic::RefUnwindSafe for Query {}

    unsafe impl Send for Query {}

    unsafe impl Sync for Query {}

    impl Unpin for Query {}

    impl std::panic::UnwindSafe for Query {}
}

#[cfg(not(target_arch = "wasm32"))]
pub use native::*;

#[cfg(target_arch = "wasm32")]
mod wasm {
    use crate::{
        error::QueryError, language::Language, node::Node, query_cursor::QueryCursor,
        query_match::QueryMatch, query_predicate::QueryPredicate,
    };
    use wasm_bindgen::JsCast;

    pub struct Query {
        pub(crate) inner: topiary_web_tree_sitter_sys::Query,
    }

    impl Query {
        #[inline]
        pub fn new(language: &Language, source: &str) -> Result<Self, QueryError> {
            let inner = language.inner.query(&source.into())?;
            Ok(Self { inner })
        }

        #[inline]
        pub fn matches<'a, 'tree: 'a>(
            &'a self,
            node: &Node<'tree>,
            _source: &'a [u8],
            _cursor: &'a mut QueryCursor,
        ) -> impl ExactSizeIterator<Item = QueryMatch<'tree>> + 'tree {
            self.inner
                .matches(&node.inner, None, None)
                .into_vec()
                .into_iter()
                .map(|value| {
                    value
                        .unchecked_into::<topiary_web_tree_sitter_sys::QueryMatch>()
                        .into()
                })
        }

        #[inline]
        pub fn capture_names(&self) -> Vec<&str> {
            // The Wasm code does not use this when looking up
            // QueryCapture::name, the way the native code needs to.
            vec![]
        }

        #[inline]
        pub fn general_predicates(&self, index: usize) -> Vec<QueryPredicate> {
            let predicates: Vec<_> = self
                .inner
                .predicates_for_pattern(index)
                .into_vec()
                .into_iter()
                .map(|value| {
                    value
                        .unchecked_into::<topiary_web_tree_sitter_sys::QueryPredicate>()
                        .into()
                })
                .collect();

            predicates
        }
    }

    impl std::fmt::Debug for Query {
        fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
            std::fmt::Debug::fmt(&self.inner, fmt)
        }
    }

    impl Drop for Query {
        #[inline]
        fn drop(&mut self) {
            self.inner.delete();
        }
    }

    impl From<topiary_web_tree_sitter_sys::Query> for Query {
        #[inline]
        fn from(inner: topiary_web_tree_sitter_sys::Query) -> Self {
            Self { inner }
        }
    }

    impl std::panic::RefUnwindSafe for Query {}

    unsafe impl Send for Query {}

    unsafe impl Sync for Query {}

    impl Unpin for Query {}

    impl std::panic::UnwindSafe for Query {}
}

#[cfg(target_arch = "wasm32")]
pub use wasm::*;
