//! Datastructures used to represent layouts in the compiler

use crate::expression_tree::{Expression, Path};
use crate::object_tree::ElementRc;
use crate::passes::ExpressionFieldsVisitor;

#[derive(Debug, derive_more::From)]
pub enum Layout {
    GridLayout(GridLayout),
    PathLayout(PathLayout),
}

impl ExpressionFieldsVisitor for Layout {
    fn visit_expressions(&mut self, visitor: &mut impl FnMut(&mut Expression)) {
        match self {
            Layout::GridLayout(grid) => grid.visit_expressions(visitor),
            Layout::PathLayout(path) => path.visit_expressions(visitor),
        }
    }
}

#[derive(Default, Debug, derive_more::Deref, derive_more::DerefMut)]
pub struct LayoutConstraints(Vec<Layout>);

impl ExpressionFieldsVisitor for LayoutConstraints {
    fn visit_expressions(&mut self, mut visitor: &mut impl FnMut(&mut Expression)) {
        self.0.iter_mut().for_each(|l| l.visit_expressions(&mut visitor));
    }
}

#[derive(Debug, derive_more::From)]
pub enum LayoutItem {
    Element(ElementRc),
    Layout(Box<Layout>),
}

/// An element in a GridLayout
#[derive(Debug)]
pub struct GridLayoutElement {
    pub col: u16,
    pub row: u16,
    pub colspan: u16,
    pub rowspan: u16,
    pub item: LayoutItem,
}

/// Internal representation of a grid layout
#[derive(Debug)]
pub struct GridLayout {
    /// All the elements will be layout within that element.
    ///
    /// FIXME: This should not be implemented like that instead there should be
    pub within: ElementRc,
    pub elems: Vec<GridLayoutElement>,
    pub x_reference: Box<Expression>,
    pub y_reference: Box<Expression>,
}

impl ExpressionFieldsVisitor for GridLayout {
    fn visit_expressions(&mut self, visitor: &mut impl FnMut(&mut Expression)) {
        visitor(&mut self.x_reference);
        visitor(&mut self.y_reference);
        for cell in &mut self.elems {
            match &mut cell.item {
                LayoutItem::Element(_) => {
                    // These expressions are traversed through the regular element tree traversal
                }
                LayoutItem::Layout(layout) => layout.visit_expressions(visitor),
            }
        }
    }
}

/// Internal representation of a path layout
#[derive(Debug)]
pub struct PathLayout {
    pub path: Path,
    pub elements: Vec<ElementRc>,
    pub x_reference: Box<Expression>,
    pub y_reference: Box<Expression>,
    pub width_reference: Box<Expression>,
    pub height_reference: Box<Expression>,
    pub offset_reference: Box<Expression>,
}

impl ExpressionFieldsVisitor for PathLayout {
    fn visit_expressions(&mut self, visitor: &mut impl FnMut(&mut Expression)) {
        visitor(&mut self.x_reference);
        visitor(&mut self.y_reference);
        visitor(&mut self.width_reference);
        visitor(&mut self.height_reference);
        visitor(&mut self.offset_reference);
    }
}
