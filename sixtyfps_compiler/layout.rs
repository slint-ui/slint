/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
//! Datastructures used to represent layouts in the compiler

use crate::expression_tree::{Expression, NamedReference, Path};
use crate::object_tree::{ElementRc, PropertyDeclaration};
use crate::{passes::ExpressionFieldsVisitor, typeregister::Type};
use std::rc::Rc;

#[derive(Debug, derive_more::From)]
pub enum Layout {
    GridLayout(GridLayout),
    PathLayout(PathLayout),
}

impl Layout {
    pub fn rect(&self) -> &LayoutRect {
        match self {
            Layout::GridLayout(g) => &g.rect,
            Layout::PathLayout(p) => &p.rect,
        }
    }
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
pub struct LayoutElement {
    pub element: ElementRc,
    pub layout: Option<Layout>,
}

#[derive(Debug, derive_more::From)]
pub enum LayoutItem {
    Element(LayoutElement),
    Layout(Box<Layout>),
}

#[derive(Debug, Clone)]
pub struct LayoutRect {
    pub width_reference: Box<Expression>,
    pub height_reference: Box<Expression>,
    pub x_reference: Box<Expression>,
    pub y_reference: Box<Expression>,
}

impl LayoutRect {
    pub fn install_on_element(element: &ElementRc) -> Self {
        let install_prop = |name: &str| {
            element.borrow_mut().property_declarations.insert(
                name.to_string(),
                PropertyDeclaration {
                    property_type: Type::Length,
                    type_node: None,
                    ..Default::default()
                },
            );

            Box::new(Expression::PropertyReference(NamedReference {
                element: Rc::downgrade(&element.clone()),
                name: name.into(),
            }))
        };

        Self {
            x_reference: install_prop("x"),
            y_reference: install_prop("y"),
            width_reference: install_prop("width"),
            height_reference: install_prop("height"),
        }
    }

    pub fn mapped_property_name(&self, name: &str) -> Option<&str> {
        let expr = match name {
            "x" => &self.x_reference,
            "y" => &self.y_reference,
            "width" => &self.width_reference,
            "height" => &self.height_reference,
            _ => return None,
        };
        match expr.as_ref() {
            Expression::PropertyReference(NamedReference { name, .. }) => Some(name),
            _ => None,
        }
    }
}

impl ExpressionFieldsVisitor for LayoutRect {
    fn visit_expressions(&mut self, visitor: &mut impl FnMut(&mut Expression)) {
        visitor(&mut self.width_reference);
        visitor(&mut self.height_reference);
        visitor(&mut self.x_reference);
        visitor(&mut self.y_reference);
    }
}

/// An element in a GridLayout
#[derive(Debug)]
pub struct GridLayoutElement {
    pub col: u16,
    pub row: u16,
    pub colspan: u16,
    pub rowspan: u16,
    pub item: LayoutItem,
    pub minimum_width: Option<Box<Expression>>,
    pub maximum_width: Option<Box<Expression>>,
    pub minimum_height: Option<Box<Expression>>,
    pub maximum_height: Option<Box<Expression>>,
}

impl GridLayoutElement {
    pub fn has_explicit_restrictions(&self) -> bool {
        self.minimum_width.is_some()
            || self.maximum_width.is_some()
            || self.minimum_height.is_some()
            || self.maximum_height.is_some()
    }

    /*pub fn for_each_restrictions(&self, mut f: impl FnMut(&str, &Expression)) {
        self.minimum_width.map(|e| f("minimum_width", &e));
        self.maximum_width.map(|e| f("maximum_width", &e));
        self.minimum_height.map(|e| f("minimum_height", &e));
        self.maximum_height.map(|e| f("maximum_height", &e));
    }*/
    pub fn for_each_restrictions<'a>(&'a self) -> [(&Option<Box<Expression>>, &'static str); 4] {
        [
            (&self.minimum_width, "min_width"),
            (&self.maximum_width, "max_width"),
            (&self.minimum_height, "min_height"),
            (&self.maximum_height, "max_height"),
        ]
    }
}

#[derive(Debug)]
pub struct Padding {
    pub left: Option<Expression>,
    pub right: Option<Expression>,
    pub top: Option<Expression>,
    pub bottom: Option<Expression>,
}

impl ExpressionFieldsVisitor for Padding {
    fn visit_expressions(&mut self, visitor: &mut impl FnMut(&mut Expression)) {
        self.left.as_mut().map(|e| visitor(&mut *e));
        self.right.as_mut().map(|e| visitor(&mut *e));
        self.top.as_mut().map(|e| visitor(&mut *e));
        self.bottom.as_mut().map(|e| visitor(&mut *e));
    }
}

/// Internal representation of a grid layout
#[derive(Debug)]
pub struct GridLayout {
    /// All the elements will be layout within that element.
    ///
    pub elems: Vec<GridLayoutElement>,
    pub rect: LayoutRect,

    pub spacing: Option<Expression>,
    pub padding: Padding,
}

impl ExpressionFieldsVisitor for GridLayout {
    fn visit_expressions(&mut self, visitor: &mut impl FnMut(&mut Expression)) {
        self.rect.visit_expressions(visitor);
        for cell in &mut self.elems {
            match &mut cell.item {
                LayoutItem::Element(element) => {
                    element.layout.as_mut().map(|layout| layout.visit_expressions(visitor));
                    // The expressions of element.element are traversed through the regular element tree traversal
                }
                LayoutItem::Layout(layout) => layout.visit_expressions(visitor),
            }
            cell.maximum_width.as_mut().map(|e| visitor(&mut *e));
            cell.minimum_width.as_mut().map(|e| visitor(&mut *e));
            cell.maximum_height.as_mut().map(|e| visitor(&mut *e));
            cell.minimum_height.as_mut().map(|e| visitor(&mut *e));
        }
        self.spacing.as_mut().map(|e| visitor(&mut *e));
        self.padding.visit_expressions(visitor);
    }
}

/// Internal representation of a path layout
#[derive(Debug)]
pub struct PathLayout {
    pub path: Path,
    pub elements: Vec<ElementRc>,
    pub rect: LayoutRect,
    pub offset_reference: Box<Expression>,
}

impl ExpressionFieldsVisitor for PathLayout {
    fn visit_expressions(&mut self, visitor: &mut impl FnMut(&mut Expression)) {
        self.rect.visit_expressions(visitor);
        visitor(&mut self.offset_reference);
    }
}

pub mod gen {
    use super::*;
    use crate::object_tree::Component;

    pub trait Language {
        type CompiledCode;
    }

    #[derive(derive_more::From)]
    pub enum LayoutTreeItem<'a, L: Language> {
        GridLayout {
            grid: &'a GridLayout,
            spacing: L::CompiledCode,
            padding: L::CompiledCode,
            var_creation_code: L::CompiledCode,
            cell_ref_variable: L::CompiledCode,
        },
        PathLayout(&'a PathLayout),
    }

    pub trait LayoutItemCodeGen<L: Language> {
        fn get_property_ref(&self, name: &str) -> L::CompiledCode;
        fn get_layout_info_ref<'a, 'b>(
            &'a self,
            layout_tree: &'b mut Vec<LayoutTreeItem<'a, L>>,
            component: &Rc<Component>,
        ) -> L::CompiledCode;
    }

    impl<L: Language> LayoutItemCodeGen<L> for LayoutItem
    where
        LayoutElement: LayoutItemCodeGen<L>,
        Layout: LayoutItemCodeGen<L>,
    {
        fn get_property_ref(&self, name: &str) -> L::CompiledCode {
            match self {
                LayoutItem::Element(e) => e.get_property_ref(name),
                LayoutItem::Layout(l) => l.get_property_ref(name),
            }
        }
        fn get_layout_info_ref<'a, 'b>(
            &'a self,
            layout_tree: &'b mut Vec<LayoutTreeItem<'a, L>>,
            component: &Rc<Component>,
        ) -> L::CompiledCode {
            match self {
                LayoutItem::Element(e) => e.get_layout_info_ref(layout_tree, component),
                LayoutItem::Layout(l) => l.get_layout_info_ref(layout_tree, component),
            }
        }
    }
}
