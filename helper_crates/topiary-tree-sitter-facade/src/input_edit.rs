#[cfg(not(target_arch = "wasm32"))]
mod native {
    use crate::point::Point;
    use std::convert::TryFrom;

    #[derive(Clone, Eq, PartialEq)]
    pub struct InputEdit {
        pub(crate) inner: tree_sitter::InputEdit,
    }

    impl InputEdit {
        #[inline]
        pub fn new(
            start_byte: u32,
            old_end_byte: u32,
            new_end_byte: u32,
            start_position: &Point,
            old_end_position: &Point,
            new_end_position: &Point,
        ) -> Self {
            let start_byte = start_byte as usize;
            let old_end_byte = old_end_byte as usize;
            let new_end_byte = new_end_byte as usize;
            let start_position = start_position.inner;
            let old_end_position = old_end_position.inner;
            let new_end_position = new_end_position.inner;
            tree_sitter::InputEdit {
                start_byte,
                old_end_byte,
                new_end_byte,
                start_position,
                old_end_position,
                new_end_position,
            }
            .into()
        }

        #[inline]
        pub fn new_end_byte(&self) -> u32 {
            u32::try_from(self.inner.new_end_byte).unwrap()
        }

        #[inline]
        pub fn new_end_position(&self) -> Point {
            let inner = self.inner.new_end_position;
            Point { inner }
        }

        #[inline]
        pub fn old_end_byte(&self) -> u32 {
            u32::try_from(self.inner.old_end_byte).unwrap()
        }

        #[inline]
        pub fn old_end_position(&self) -> Point {
            let inner = self.inner.old_end_position;
            Point { inner }
        }

        #[inline]
        pub fn start_byte(&self) -> u32 {
            u32::try_from(self.inner.start_byte).unwrap()
        }

        #[inline]
        pub fn start_position(&self) -> Point {
            let inner = self.inner.start_position;
            Point { inner }
        }
    }

    impl std::fmt::Debug for InputEdit {
        fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
            std::fmt::Debug::fmt(&self.inner, fmt)
        }
    }

    impl Default for InputEdit {
        fn default() -> Self {
            let start_index = Default::default();
            let old_end_index = Default::default();
            let new_end_index = Default::default();
            let start_position = &Default::default();
            let old_end_position = &Default::default();
            let new_end_position = &Default::default();
            Self::new(
                start_index,
                old_end_index,
                new_end_index,
                start_position,
                old_end_position,
                new_end_position,
            )
        }
    }

    impl From<tree_sitter::InputEdit> for InputEdit {
        #[inline]
        fn from(inner: tree_sitter::InputEdit) -> Self {
            Self { inner }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub use native::*;

#[cfg(target_arch = "wasm32")]
mod wasm {
    use crate::point::Point;

    #[derive(Clone, Eq, PartialEq)]
    pub struct InputEdit {
        pub(crate) inner: topiary_web_tree_sitter_sys::Edit,
    }

    impl InputEdit {
        #[inline]
        pub fn new(
            start_byte: u32,
            old_end_byte: u32,
            new_end_byte: u32,
            start_position: &Point,
            old_end_position: &Point,
            new_end_position: &Point,
        ) -> Self {
            let start_position = &start_position.inner;
            let old_end_position = &old_end_position.inner;
            let new_end_position = &new_end_position.inner;
            topiary_web_tree_sitter_sys::Edit::new(
                start_byte,
                old_end_byte,
                new_end_byte,
                start_position,
                old_end_position,
                new_end_position,
            )
            .into()
        }

        #[inline]
        pub fn new_end_byte(&self) -> u32 {
            self.inner.new_end_index()
        }

        #[inline]
        pub fn new_end_position(&self) -> Point {
            let inner = self.inner.new_end_position();
            Point { inner }
        }

        #[inline]
        pub fn old_end_byte(&self) -> u32 {
            self.inner.old_end_index()
        }

        #[inline]
        pub fn old_end_position(&self) -> Point {
            let inner = self.inner.old_end_position();
            Point { inner }
        }

        #[inline]
        pub fn start_byte(&self) -> u32 {
            self.inner.start_index()
        }

        #[inline]
        pub fn start_position(&self) -> Point {
            let inner = self.inner.start_position();
            Point { inner }
        }
    }

    impl std::fmt::Debug for InputEdit {
        fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
            std::fmt::Debug::fmt(&self.inner, fmt)
        }
    }

    impl Default for InputEdit {
        fn default() -> Self {
            let start_index = Default::default();
            let old_end_index = Default::default();
            let new_end_index = Default::default();
            let start_position = &Default::default();
            let old_end_position = &Default::default();
            let new_end_position = &Default::default();
            Self::new(
                start_index,
                old_end_index,
                new_end_index,
                start_position,
                old_end_position,
                new_end_position,
            )
        }
    }

    impl From<topiary_web_tree_sitter_sys::Edit> for InputEdit {
        #[inline]
        fn from(inner: topiary_web_tree_sitter_sys::Edit) -> Self {
            Self { inner }
        }
    }

    unsafe impl Send for InputEdit {}

    unsafe impl Sync for InputEdit {}
}

#[cfg(target_arch = "wasm32")]
pub use wasm::*;
