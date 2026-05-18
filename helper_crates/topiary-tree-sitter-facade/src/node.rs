#[cfg(not(target_arch = "wasm32"))]
mod native {
    use crate::{
        input_edit::InputEdit, language::LanguageRef, point::Point, range::Range,
        tree_cursor::TreeCursor,
    };
    use std::{borrow::Cow, convert::TryFrom};

    #[derive(Clone, Copy, Eq, Hash, PartialEq)]
    pub struct Node<'tree> {
        pub(crate) inner: tree_sitter::Node<'tree>,
    }

    impl<'tree> Node<'tree> {
        #[inline]
        pub fn byte_range(&self) -> std::ops::Range<u32> {
            let range = self.inner.byte_range();
            let start = u32::try_from(range.start).unwrap();
            let end = u32::try_from(range.end).unwrap();
            start..end
        }

        #[inline]
        pub fn child(&self, i: u32) -> Option<Self> {
            self.inner.child(i).map(Into::into)
        }

        #[inline]
        pub fn child_by_field_id(&self, field_id: u16) -> Option<Self> {
            self.inner.child_by_field_id(field_id).map(Into::into)
        }

        #[inline]
        pub fn child_by_field_name(&self, field_name: impl AsRef<[u8]>) -> Option<Self> {
            self.inner.child_by_field_name(field_name).map(Into::into)
        }

        #[inline]
        pub fn child_count(&self) -> u32 {
            u32::try_from(self.inner.child_count()).unwrap()
        }

        #[inline]
        pub fn children<'a>(
            &self,
            cursor: &'a mut TreeCursor<'tree>,
        ) -> impl ExactSizeIterator<Item = Node<'tree>> + 'a {
            self.inner.children(&mut cursor.inner).map(Into::into)
        }

        #[inline]
        pub fn children_by_field_id<'a>(
            &self,
            field_id: std::num::NonZeroU16,
            cursor: &'a mut TreeCursor<'tree>,
        ) -> impl Iterator<Item = Node<'tree>> + 'a {
            self.inner
                .children_by_field_id(field_id, &mut cursor.inner)
                .map(Into::into)
        }

        #[inline]
        pub fn children_by_field_name<'a>(
            &self,
            field_name: &str,
            cursor: &'a mut TreeCursor<'tree>,
        ) -> impl Iterator<Item = Node<'tree>> + 'a {
            self.inner
                .children_by_field_name(field_name, &mut cursor.inner)
                .map(Into::into)
        }

        #[inline]
        pub fn descendant_for_byte_range(&self, start: u32, end: u32) -> Option<Self> {
            self.inner
                .descendant_for_byte_range(start as usize, end as usize)
                .map(Into::into)
        }

        #[inline]
        pub fn descendant_for_point_range(&self, start: Point, end: Point) -> Option<Self> {
            self.inner
                .descendant_for_point_range(start.inner, end.inner)
                .map(Into::into)
        }

        #[inline]
        pub fn edit(&mut self, edit: &InputEdit) {
            self.inner.edit(&edit.inner);
        }

        #[inline]
        pub fn end_byte(&self) -> u32 {
            u32::try_from(self.inner.end_byte()).unwrap()
        }

        #[inline]
        pub fn end_position(&self) -> Point {
            self.inner.end_position().into()
        }

        #[inline]
        pub fn has_changes(&self) -> bool {
            self.inner.has_changes()
        }

        #[inline]
        pub fn has_error(&self) -> bool {
            self.inner.has_error()
        }

        #[inline]
        pub fn id(&self) -> usize {
            self.inner.id()
        }

        #[inline]
        pub fn is_error(&self) -> bool {
            self.inner.is_error()
        }

        #[inline]
        pub fn is_extra(&self) -> bool {
            self.inner.is_named()
        }

        #[inline]
        pub fn is_missing(&self) -> bool {
            self.inner.is_missing()
        }

        #[inline]
        pub fn is_named(&self) -> bool {
            self.inner.is_named()
        }

        #[inline]
        pub fn kind(&self) -> Cow<'_, str> {
            self.inner.kind().into()
        }

        #[inline]
        pub fn kind_id(&self) -> u16 {
            self.inner.kind_id()
        }

        #[inline]
        pub fn language(&self) -> LanguageRef<'_> {
            self.inner.language().into()
        }

        #[inline]
        pub fn language_name(&self) -> Option<&'static str> {
            self.inner.language().name()
        }

        #[inline]
        pub fn named_child(&self, i: u32) -> Option<Self> {
            self.inner.named_child(i).map(Into::into)
        }

        #[inline]
        pub fn named_child_count(&self) -> u32 {
            u32::try_from(self.inner.named_child_count()).unwrap()
        }

        #[inline]
        pub fn named_children<'a>(
            &self,
            cursor: &'a mut TreeCursor<'tree>,
        ) -> impl ExactSizeIterator<Item = Node<'tree>> + 'a {
            self.inner.named_children(&mut cursor.inner).map(Into::into)
        }

        #[inline]
        pub fn named_descendant_for_byte_range(&self, start: u32, end: u32) -> Option<Self> {
            self.inner
                .named_descendant_for_byte_range(start as usize, end as usize)
                .map(Into::into)
        }

        #[inline]
        pub fn named_descendant_for_point_range(&self, start: Point, end: Point) -> Option<Self> {
            self.inner
                .named_descendant_for_point_range(start.inner, end.inner)
                .map(Into::into)
        }

        #[inline]
        pub fn next_named_sibling(&self) -> Option<Self> {
            self.inner.next_named_sibling().map(Into::into)
        }

        #[inline]
        pub fn next_sibling(&self) -> Option<Self> {
            self.inner.next_sibling().map(Into::into)
        }

        #[inline]
        pub fn parent(&self) -> Option<Self> {
            self.inner.parent().map(Into::into)
        }

        #[inline]
        pub fn prev_named_sibling(&self) -> Option<Self> {
            self.inner.prev_named_sibling().map(Into::into)
        }

        #[inline]
        pub fn prev_sibling(&self) -> Option<Self> {
            self.inner.prev_sibling().map(Into::into)
        }

        #[inline]
        pub fn range(&self) -> Range {
            self.inner.range().into()
        }

        #[inline]
        pub fn start_byte(&self) -> u32 {
            u32::try_from(self.inner.start_byte()).unwrap()
        }

        #[inline]
        pub fn start_position(&self) -> Point {
            self.inner.start_position().into()
        }

        #[inline]
        #[allow(clippy::wrong_self_convention)]
        pub fn to_sexp(&self) -> Cow<'_, str> {
            self.inner.to_sexp().into()
        }

        #[inline]
        pub fn utf8_text<'a>(&self, source: &'a [u8]) -> Result<Cow<'a, str>, std::str::Utf8Error> {
            self.inner.utf8_text(source).map(Into::into)
        }

        #[inline]
        pub fn utf16_text<'a>(&self, source: &'a [u16]) -> &'a [u16] {
            self.inner.utf16_text(source)
        }

        #[inline]
        pub fn walk(&self) -> TreeCursor<'tree> {
            self.inner.walk().into()
        }
    }

    impl std::fmt::Debug for Node<'_> {
        fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
            std::fmt::Debug::fmt(&self.inner, fmt)
        }
    }

    impl<'tree> From<tree_sitter::Node<'tree>> for Node<'tree> {
        #[inline]
        fn from(inner: tree_sitter::Node<'tree>) -> Self {
            Node { inner }
        }
    }

    impl Ord for Node<'_> {
        fn cmp(&self, that: &Self) -> std::cmp::Ordering {
            let this = self.id();
            let that = that.id();
            this.cmp(&that)
        }
    }

    impl<'a> PartialOrd for Node<'a> {
        fn partial_cmp(&self, that: &Node<'a>) -> Option<std::cmp::Ordering> {
            Some(self.cmp(that))
        }
    }

    impl std::panic::RefUnwindSafe for Node<'_> {}

    impl Unpin for Node<'_> {}

    impl std::panic::UnwindSafe for Node<'_> {}
}

#[cfg(not(target_arch = "wasm32"))]
pub use native::*;

#[cfg(target_arch = "wasm32")]
mod wasm {
    use crate::{input_edit::InputEdit, point::Point, range::Range, tree_cursor::TreeCursor};
    use std::borrow::Cow;
    use topiary_web_tree_sitter_sys::SyntaxNode;
    use wasm_bindgen::{JsCast, prelude::*};

    #[derive(Clone, Eq, Hash, PartialEq)]
    pub struct Node<'tree> {
        pub(crate) inner: SyntaxNode,
        pub(crate) phantom: std::marker::PhantomData<&'tree ()>,
    }

    impl<'tree> Node<'tree> {
        // FIXME: check that this is correct
        #[inline]
        pub fn byte_range(&self) -> std::ops::Range<u32> {
            let start = self.inner.start_index();
            let end = self.inner.end_index();
            start..end
        }

        #[inline]
        pub fn child(&self, i: u32) -> Option<Self> {
            self.inner.child(i).map(Into::into)
        }

        #[inline]
        pub fn child_by_field_id(&self, field_id: u16) -> Option<Self> {
            self.inner.child_for_field_id(field_id).map(Into::into)
        }

        #[inline]
        pub fn child_by_field_name(&self, field_name: impl AsRef<[u8]>) -> Option<Self> {
            let field_name = field_name.as_ref();
            let field_name = unsafe { std::str::from_utf8_unchecked(field_name) };
            self.inner.child_for_field_name(field_name).map(Into::into)
        }

        #[inline]
        pub fn child_count(&self) -> u32 {
            self.inner.child_count()
        }

        #[inline]
        pub fn children<'a>(
            &self,
            _cursor: &'a mut TreeCursor<'tree>,
        ) -> impl ExactSizeIterator<Item = Node<'tree>> + 'a {
            self.inner
                .children()
                .into_vec()
                .into_iter()
                .map(|value| value.unchecked_into::<SyntaxNode>().into())
        }

        pub fn children_by_field_id<'a>(
            &self,
            field_id: u16,
            cursor: &'a mut TreeCursor<'tree>,
        ) -> impl Iterator<Item = Node<'tree>> + 'a {
            cursor.reset(self.clone());
            cursor.goto_first_child();
            std::iter::from_fn(move || {
                while cursor.field_id() != Some(field_id) {
                    if !cursor.goto_next_sibling() {
                        return None;
                    }
                }
                let result = cursor.node();
                Some(result)
            })
        }

        pub fn children_by_field_name<'a>(
            &self,
            field_name: &'a str,
            cursor: &'a mut TreeCursor<'tree>,
        ) -> impl Iterator<Item = Node<'tree>> + 'a {
            cursor.reset(self.clone());
            cursor.goto_first_child();
            std::iter::from_fn(move || {
                while cursor.field_name() != Some(field_name.into()) {
                    if !cursor.goto_next_sibling() {
                        return None;
                    }
                }
                let result = cursor.node();
                Some(result)
            })
        }

        #[inline]
        pub fn descendant_for_byte_range(&self, start: u32, end: u32) -> Option<Self> {
            self.inner
                .descendant_for_index_range(start, end)
                .map(Into::into)
        }

        #[inline]
        pub fn descendant_for_point_range(&self, start: Point, end: Point) -> Option<Self> {
            self.inner
                .descendant_for_position_range(&start.inner, &end.inner)
                .map(Into::into)
        }

        #[inline]
        pub fn edit(&mut self, _edit: &InputEdit) {
            unimplemented!()
        }

        // FIXME: this returns end character offset instead of byte offset
        #[inline]
        pub fn end_byte(&self) -> u32 {
            self.inner.end_index()
        }

        #[inline]
        pub fn end_position(&self) -> Point {
            self.inner.end_position().into()
        }

        #[inline]
        pub fn has_changes(&self) -> bool {
            self.inner.has_changes()
        }

        #[inline]
        pub fn has_error(&self) -> bool {
            self.inner.has_error()
        }

        #[inline]
        pub fn id(&self) -> usize {
            self.inner.id() as usize
        }

        #[inline]
        pub fn is_error(&self) -> bool {
            self.kind_id() == u16::MAX
        }

        #[inline]
        pub fn is_extra(&self) -> bool {
            unimplemented!()
        }

        #[inline]
        pub fn is_missing(&self) -> bool {
            self.inner.is_missing()
        }

        #[inline]
        pub fn is_named(&self) -> bool {
            self.inner.is_named()
        }

        #[inline]
        pub fn kind(&self) -> Cow<str> {
            From::<String>::from(self.inner.type_().into())
        }

        #[inline]
        pub fn kind_id(&self) -> u16 {
            self.inner.type_id()
        }

        #[inline]
        pub fn named_child(&self, i: u32) -> Option<Self> {
            self.inner.named_child(i).map(Into::into)
        }

        #[inline]
        pub fn named_child_count(&self) -> u32 {
            self.inner.named_child_count()
        }

        #[inline]
        pub fn named_children<'a>(
            &self,
            _cursor: &'a mut TreeCursor<'tree>,
        ) -> impl ExactSizeIterator<Item = Node<'tree>> + 'a {
            self.inner
                .named_children()
                .into_vec()
                .into_iter()
                .map(|value| value.unchecked_into::<SyntaxNode>().into())
        }

        #[inline]
        pub fn named_descendant_for_byte_range(&self, start: u32, end: u32) -> Option<Self> {
            self.inner
                .named_descendant_for_index_range(start, end)
                .map(Into::into)
        }

        #[inline]
        pub fn named_descendant_for_point_range(&self, start: Point, end: Point) -> Option<Self> {
            self.inner
                .named_descendant_for_position_range(&start.inner, &end.inner)
                .map(Into::into)
        }

        #[inline]
        pub fn next_named_sibling(&self) -> Option<Self> {
            self.inner.next_named_sibling().map(Into::into)
        }

        #[inline]
        pub fn next_sibling(&self) -> Option<Self> {
            self.inner.next_sibling().map(Into::into)
        }

        #[inline]
        pub fn parent(&self) -> Option<Self> {
            self.inner.parent().map(Into::into)
        }

        #[inline]
        pub fn prev_named_sibling(&self) -> Option<Self> {
            self.inner.previous_named_sibling().map(Into::into)
        }

        #[inline]
        pub fn prev_sibling(&self) -> Option<Self> {
            self.inner.previous_sibling().map(Into::into)
        }

        // FIXME: check that this is correct
        #[inline]
        pub fn range(&self) -> Range {
            let start_position = self.inner.start_position();
            let end_position = self.inner.end_position();
            let start_index = self.inner.start_index();
            let end_index = self.inner.end_index();
            topiary_web_tree_sitter_sys::Range::new(
                &start_position,
                &end_position,
                start_index,
                end_index,
            )
            .into()
        }

        // FIXME: this returns start character offset instead of byte offset
        #[inline]
        pub fn start_byte(&self) -> u32 {
            self.inner.start_index()
        }

        #[inline]
        pub fn start_position(&self) -> Point {
            self.inner.start_position().into()
        }

        // FIXME: check that this is correct
        #[inline]
        #[allow(clippy::wrong_self_convention)]
        pub fn to_sexp(&self) -> Cow<str> {
            From::<String>::from(self.inner.to_string().into())
        }

        // source should not be used in wasm because start_byte is character offset instead of byte offset
        // this is caused by different string encoding in JS and Rust
        #[inline]
        pub fn utf8_text<'a>(
            &self,
            _source: &'a [u8],
        ) -> Result<Cow<'a, str>, std::str::Utf8Error> {
            Ok(self.inner.text().as_string().unwrap().into())
        }

        #[inline]
        pub fn utf16_text<'a>(&self, _source: &'a [u16]) -> &'a [u16] {
            unimplemented!()
        }

        #[inline]
        pub fn walk(&self) -> TreeCursor<'tree> {
            self.inner.walk().into()
        }
    }

    impl<'tree> std::fmt::Debug for Node<'tree> {
        fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
            std::fmt::Debug::fmt(&self.inner, fmt)
        }
    }

    impl<'tree> From<SyntaxNode> for Node<'tree> {
        #[inline]
        fn from(inner: SyntaxNode) -> Self {
            let phantom = std::marker::PhantomData;
            Node { inner, phantom }
        }
    }

    pub struct NodeIterator<'tree, 'a> {
        pub(crate) inner: Box<dyn Iterator<Item = Node<'tree>> + 'a>,
    }

    impl<'tree, 'a> Iterator for NodeIterator<'tree, 'a> {
        type Item = Node<'tree>;

        fn next(&mut self) -> Option<Self::Item> {
            self.inner.next()
        }
    }

    pub struct NodeExactSizeIterator<'tree> {
        inner: Box<[JsValue]>,
        index: usize,
        phantom: std::marker::PhantomData<&'tree ()>,
    }

    impl<'tree> Iterator for NodeExactSizeIterator<'tree> {
        type Item = Node<'tree>;

        fn next(&mut self) -> Option<Self::Item> {
            let node = self.inner[self.index]
                .clone()
                .unchecked_into::<SyntaxNode>();
            Some(node.into())
        }
    }

    impl<'tree> ExactSizeIterator for NodeExactSizeIterator<'tree> {
        fn len(&self) -> usize {
            self.inner.len()
        }
    }

    impl<'a> Ord for Node<'a> {
        fn cmp(&self, that: &Self) -> std::cmp::Ordering {
            let this = self.id();
            let that = that.id();
            this.cmp(&that)
        }
    }

    impl<'a> PartialOrd for Node<'a> {
        fn partial_cmp(&self, that: &Node<'a>) -> Option<std::cmp::Ordering> {
            Some(self.cmp(that))
        }
    }

    impl<'a> std::panic::RefUnwindSafe for Node<'a> {}

    impl<'a> Unpin for Node<'a> {}

    impl<'a> std::panic::UnwindSafe for Node<'a> {}
}

#[cfg(target_arch = "wasm32")]
pub use wasm::*;
