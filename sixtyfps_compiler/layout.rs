//! Datastructures used to represent layouts in the compiler

use crate::object_tree::ElementRc;

#[derive(Default, Debug)]
pub struct LayoutConstraints(pub Vec<GridLayout>);

/// Internal representation of a grid layout
#[derive(Debug)]
pub struct GridLayout {
    /// All the elements will be layout within that element.
    ///
    /// FIXME: This should not be implemented like that instead there should be
    pub within: ElementRc,
    /// This is like a matrix of elements.
    pub elems: Vec<Vec<Option<ElementRc>>>,
}

impl GridLayout {
    pub fn col_count(&self) -> usize {
        self.elems.iter().map(|x| x.len()).max().unwrap_or(0)
    }
    pub fn row_count(&self) -> usize {
        self.elems.len()
    }
}
