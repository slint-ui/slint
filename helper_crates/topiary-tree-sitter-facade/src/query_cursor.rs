#[cfg(not(target_arch = "wasm32"))]
mod native {
    pub struct QueryCursor {
        pub(crate) inner: tree_sitter::QueryCursor,
    }

    impl QueryCursor {
        #[inline]
        pub fn new() -> Self {
            let inner = tree_sitter::QueryCursor::new();
            Self { inner }
        }
    }

    impl Default for QueryCursor {
        fn default() -> Self {
            Self::new()
        }
    }

    impl From<tree_sitter::QueryCursor> for QueryCursor {
        #[inline]
        fn from(inner: tree_sitter::QueryCursor) -> Self {
            Self { inner }
        }
    }

    impl std::panic::RefUnwindSafe for QueryCursor {}

    impl Unpin for QueryCursor {}

    impl std::panic::UnwindSafe for QueryCursor {}
}

#[cfg(not(target_arch = "wasm32"))]
pub use native::*;

#[cfg(target_arch = "wasm32")]
mod wasm {
    #[derive(Clone)]
    pub struct QueryCursor {}

    impl QueryCursor {
        #[inline]
        pub fn new() -> Self {
            Self {}
        }
    }

    impl Default for QueryCursor {
        fn default() -> Self {
            Self::new()
        }
    }

    impl std::panic::RefUnwindSafe for QueryCursor {}

    impl Unpin for QueryCursor {}

    impl std::panic::UnwindSafe for QueryCursor {}
}

#[cfg(target_arch = "wasm32")]
pub use wasm::*;
