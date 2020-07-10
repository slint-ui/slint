//! Datastructures used to represent layouts in the compiler

use crate::expression_tree::{Expression, Path};
use crate::object_tree::ElementRc;
use crate::passes::ExpressionFieldsVisitor;

#[derive(Default, Debug)]
pub struct LayoutConstraints {
    pub grids: Vec<GridLayout>,
    pub paths: Vec<PathLayout>,
}

impl ExpressionFieldsVisitor for LayoutConstraints {
    fn visit_expressions(&mut self, mut visitor: impl FnMut(&mut Expression)) {
        self.paths.iter_mut().for_each(|l| l.visit_expressions(&mut visitor))
    }
}

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

/// Internal representation of a path layout
#[derive(Debug)]
pub struct PathLayout {
    pub path: Path,
    pub elements: Vec<ElementRc>,
    pub x_reference: Box<Expression>,
    pub y_reference: Box<Expression>,
}

impl ExpressionFieldsVisitor for PathLayout {
    fn visit_expressions(&mut self, mut visitor: impl FnMut(&mut Expression)) {
        visitor(&mut self.x_reference);
        visitor(&mut self.y_reference);
    }
}
