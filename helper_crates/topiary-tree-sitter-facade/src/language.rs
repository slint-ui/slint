#[cfg(not(target_arch = "wasm32"))]
mod native {
    use std::{borrow::Cow, convert::TryFrom};

    pub struct LanguageRef<'a> {
        pub(crate) inner: tree_sitter::LanguageRef<'a>,
    }

    impl LanguageRef<'_> {
        pub fn field_count(&self) -> usize {
            self.inner.field_count()
        }
    }

    impl<'a> From<tree_sitter::LanguageRef<'a>> for LanguageRef<'a> {
        fn from(inner: tree_sitter::LanguageRef<'a>) -> Self {
            LanguageRef { inner }
        }
    }

    #[derive(Clone, Eq, PartialEq)]
    pub struct Language {
        pub(crate) inner: tree_sitter::Language,
    }

    impl Language {
        #[inline]
        pub fn field_count(&self) -> u16 {
            u16::try_from(self.inner.field_count()).unwrap()
        }

        #[inline]
        pub fn field_id_for_name(
            &self,
            field_name: impl AsRef<[u8]>,
        ) -> Option<std::num::NonZeroU16> {
            let field_name = field_name.as_ref();
            self.inner.field_id_for_name(field_name)
        }

        #[inline]
        pub fn field_name_for_id(&self, field_id: u16) -> Option<Cow<'_, str>> {
            self.inner.field_name_for_id(field_id).map(Into::into)
        }

        #[inline]
        pub fn id_for_node_kind(&self, kind: &str, named: bool) -> u16 {
            self.inner.id_for_node_kind(kind, named)
        }

        #[inline]
        pub fn node_kind_count(&self) -> u16 {
            u16::try_from(self.inner.node_kind_count()).unwrap()
        }

        #[inline]
        pub fn node_kind_for_id(&self, id: u16) -> Option<Cow<'_, str>> {
            self.inner.node_kind_for_id(id).map(Into::into)
        }

        #[inline]
        pub fn node_kind_is_named(&self, id: u16) -> bool {
            self.inner.node_kind_is_named(id)
        }

        #[inline]
        pub fn node_kind_is_visible(&self, id: u16) -> bool {
            self.inner.node_kind_is_visible(id)
        }

        #[inline]
        pub fn version(&self) -> u32 {
            u32::try_from(self.inner.abi_version()).unwrap()
        }
    }

    impl std::fmt::Debug for Language {
        fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
            std::fmt::Debug::fmt(&self.inner, fmt)
        }
    }

    impl From<tree_sitter::Language> for Language {
        #[inline]
        fn from(inner: tree_sitter::Language) -> Self {
            Self { inner }
        }
    }

    impl From<tree_sitter_language::LanguageFn> for Language {
        #[inline]
        fn from(inner: tree_sitter_language::LanguageFn) -> Self {
            Language::from(tree_sitter::Language::new(inner))
        }
    }

    impl std::panic::RefUnwindSafe for Language {}

    unsafe impl Send for Language {}

    unsafe impl Sync for Language {}

    impl Unpin for Language {}

    impl std::panic::UnwindSafe for Language {}
}

#[cfg(not(target_arch = "wasm32"))]
pub use native::*;

#[cfg(target_arch = "wasm32")]
mod wasm {
    use std::borrow::Cow;

    #[derive(Clone, PartialEq)]
    pub struct Language {
        pub(crate) inner: topiary_web_tree_sitter_sys::Language,
    }

    impl Language {
        #[inline]
        pub fn field_count(&self) -> u16 {
            self.inner.field_count()
        }

        #[inline]
        pub fn field_id_for_name(&self, field_name: impl AsRef<[u8]>) -> Option<u16> {
            let field_name = field_name.as_ref();
            let field_name = unsafe { std::str::from_utf8_unchecked(field_name) };
            self.inner.field_id_for_name(field_name)
        }

        #[inline]
        pub fn field_name_for_id(&self, field_id: u16) -> Option<Cow<str>> {
            self.inner.field_name_for_id(field_id).map(Into::into)
        }

        #[inline]
        pub fn id_for_node_kind(&self, kind: &str, named: bool) -> u16 {
            self.inner.id_for_node_kind(kind, named)
        }

        #[inline]
        pub fn node_kind_count(&self) -> u16 {
            self.inner.node_kind_count()
        }

        #[inline]
        pub fn node_kind_for_id(&self, id: u16) -> Option<Cow<str>> {
            self.inner.node_kind_for_id(id).map(Into::into)
        }

        #[inline]
        pub fn node_kind_is_named(&self, id: u16) -> bool {
            self.inner.node_kind_is_named(id)
        }

        #[inline]
        pub fn node_kind_is_visible(&self, id: u16) -> bool {
            self.inner.node_kind_is_visible(id)
        }

        #[inline]
        pub fn version(&self) -> u32 {
            self.inner.version()
        }
    }

    impl std::fmt::Debug for Language {
        fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
            std::fmt::Debug::fmt(&self.inner, fmt)
        }
    }

    impl From<topiary_web_tree_sitter_sys::Language> for Language {
        #[inline]
        fn from(inner: topiary_web_tree_sitter_sys::Language) -> Self {
            Self { inner }
        }
    }

    impl std::panic::RefUnwindSafe for Language {}

    unsafe impl Send for Language {}

    unsafe impl Sync for Language {}

    impl Unpin for Language {}

    impl std::panic::UnwindSafe for Language {}
}

#[cfg(target_arch = "wasm32")]
pub use wasm::*;
