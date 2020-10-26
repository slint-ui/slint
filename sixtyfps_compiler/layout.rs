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
use crate::langtype::Type;
use crate::object_tree::{ElementRc, PropertyDeclaration};
use std::{borrow::Cow, rc::Rc};

#[derive(Debug, derive_more::From)]
pub enum Layout {
    GridLayout(GridLayout),
    PathLayout(PathLayout),
    BoxLayout(BoxLayout),
}

impl Layout {
    pub fn rect(&self) -> &LayoutRect {
        match self {
            Layout::GridLayout(g) => &g.geometry.rect,
            Layout::BoxLayout(g) => &g.geometry.rect,
            Layout::PathLayout(p) => &p.rect,
        }
    }
}

impl Layout {
    /// Call the visitor for each NamedReference stored in the layout
    pub fn visit_named_references(&mut self, visitor: &mut impl FnMut(&mut NamedReference)) {
        match self {
            Layout::GridLayout(grid) => grid.visit_named_references(visitor),
            Layout::BoxLayout(l) => l.visit_named_references(visitor),
            Layout::PathLayout(path) => path.visit_named_references(visitor),
        }
    }
}

pub type LayoutVec = Vec<Layout>;

/// An Item in the layout tree
#[derive(Debug, Default)]
pub struct LayoutItem {
    pub element: Option<ElementRc>,
    pub layout: Option<Layout>,
}

impl LayoutItem {
    pub fn rect(&self) -> Cow<LayoutRect> {
        if let Some(e) = &self.element {
            let prop = |name: &str| {
                if e.borrow().lookup_property(name) == Type::Length {
                    Some(NamedReference::new(e, name))
                } else {
                    None
                }
            };
            Cow::Owned(LayoutRect {
                x_reference: prop("x"),
                y_reference: prop("y"),
                width_reference: prop("width"),
                height_reference: prop("height"),
            })
        } else if let Some(l) = &self.layout {
            Cow::Borrowed(l.rect())
        } else {
            Cow::Owned(LayoutRect::default())
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct LayoutRect {
    pub width_reference: Option<NamedReference>,
    pub height_reference: Option<NamedReference>,
    pub x_reference: Option<NamedReference>,
    pub y_reference: Option<NamedReference>,
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
            Some(NamedReference::new(element, name))
        };

        Self {
            x_reference: install_prop("x"),
            y_reference: install_prop("y"),
            width_reference: install_prop("width"),
            height_reference: install_prop("height"),
        }
    }

    fn visit_named_references(&mut self, mut visitor: &mut impl FnMut(&mut NamedReference)) {
        self.width_reference.as_mut().map(&mut visitor);
        self.height_reference.as_mut().map(&mut visitor);
        self.x_reference.as_mut().map(&mut visitor);
        self.y_reference.as_mut().map(&mut visitor);
    }
}

#[derive(Debug)]
pub struct LayoutConstraints {
    pub minimum_width: Option<NamedReference>,
    pub maximum_width: Option<NamedReference>,
    pub minimum_height: Option<NamedReference>,
    pub maximum_height: Option<NamedReference>,
}

impl LayoutConstraints {
    pub fn new(element: &ElementRc) -> Self {
        Self {
            minimum_width: binding_reference(&element, "minimum_width"),
            maximum_width: binding_reference(&element, "maximum_width"),
            minimum_height: binding_reference(&element, "minimum_height"),
            maximum_height: binding_reference(&element, "maximum_height"),
        }
    }

    pub fn has_explicit_restrictions(&self) -> bool {
        self.minimum_width.is_some()
            || self.maximum_width.is_some()
            || self.minimum_height.is_some()
            || self.maximum_height.is_some()
    }

    pub fn for_each_restrictions<'a>(&'a self) -> [(&Option<NamedReference>, &'static str); 4] {
        [
            (&self.minimum_width, "min_width"),
            (&self.maximum_width, "max_width"),
            (&self.minimum_height, "min_height"),
            (&self.maximum_height, "max_height"),
        ]
    }
}

impl LayoutConstraints {
    fn visit_named_references(&mut self, visitor: &mut impl FnMut(&mut NamedReference)) {
        self.maximum_width.as_mut().map(|e| visitor(&mut *e));
        self.minimum_width.as_mut().map(|e| visitor(&mut *e));
        self.maximum_height.as_mut().map(|e| visitor(&mut *e));
        self.minimum_height.as_mut().map(|e| visitor(&mut *e));
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
    pub constraints: LayoutConstraints,
}

#[derive(Debug)]
pub struct Padding {
    pub left: Option<NamedReference>,
    pub right: Option<NamedReference>,
    pub top: Option<NamedReference>,
    pub bottom: Option<NamedReference>,
}

impl Padding {
    fn visit_named_references(&mut self, visitor: &mut impl FnMut(&mut NamedReference)) {
        self.left.as_mut().map(|e| visitor(&mut *e));
        self.right.as_mut().map(|e| visitor(&mut *e));
        self.top.as_mut().map(|e| visitor(&mut *e));
        self.bottom.as_mut().map(|e| visitor(&mut *e));
    }
}

#[derive(Debug)]
pub struct LayoutGeometry {
    pub rect: LayoutRect,
    pub spacing: Option<NamedReference>,
    pub padding: Padding,
}

impl LayoutGeometry {
    fn visit_named_references(&mut self, visitor: &mut impl FnMut(&mut NamedReference)) {
        self.rect.visit_named_references(visitor);
        self.spacing.as_mut().map(|e| visitor(&mut *e));
        self.padding.visit_named_references(visitor);
    }
}

fn binding_reference(element: &ElementRc, name: &str) -> Option<NamedReference> {
    if element.borrow().bindings.contains_key(name) {
        Some(NamedReference { element: Rc::downgrade(element), name: name.into() })
    } else {
        None
    }
}

fn init_fake_property(
    grid_layout_element: &ElementRc,
    name: &str,
    lazy_default: impl Fn() -> Option<NamedReference>,
) {
    if grid_layout_element.borrow().property_declarations.contains_key(name)
        && !grid_layout_element.borrow().bindings.contains_key(name)
    {
        if let Some(e) = lazy_default() {
            grid_layout_element
                .borrow_mut()
                .bindings
                .insert(name.to_owned(), Expression::PropertyReference(e).into());
        }
    }
}

impl LayoutGeometry {
    pub fn new(rect: LayoutRect, layout_element: &ElementRc) -> Self {
        let padding = || binding_reference(layout_element, "padding");
        let spacing = binding_reference(layout_element, "spacing");

        init_fake_property(layout_element, "width", || rect.width_reference.clone());
        init_fake_property(layout_element, "height", || rect.height_reference.clone());
        init_fake_property(layout_element, "x", || rect.x_reference.clone());
        init_fake_property(layout_element, "y", || rect.y_reference.clone());
        init_fake_property(layout_element, "padding_left", padding);
        init_fake_property(layout_element, "padding_right", padding);
        init_fake_property(layout_element, "padding_top", padding);
        init_fake_property(layout_element, "padding_bottom", padding);

        let padding = Padding {
            left: binding_reference(layout_element, "padding_left").or_else(padding),
            right: binding_reference(layout_element, "padding_right").or_else(padding),
            top: binding_reference(layout_element, "padding_top").or_else(padding),
            bottom: binding_reference(layout_element, "padding_bottom").or_else(padding),
        };

        Self { rect, spacing, padding }
    }
}

/// Internal representation of a grid layout
#[derive(Debug)]
pub struct GridLayout {
    /// All the elements will be layout within that element.
    pub elems: Vec<GridLayoutElement>,

    pub geometry: LayoutGeometry,
}

impl GridLayout {
    fn visit_named_references(&mut self, visitor: &mut impl FnMut(&mut NamedReference)) {
        for cell in &mut self.elems {
            cell.item.layout.as_mut().map(|x| x.visit_named_references(visitor));
            cell.constraints.visit_named_references(visitor);
        }
        self.geometry.visit_named_references(visitor);
    }
}

/// An element in a BoxLayout
#[derive(Debug)]
pub struct BoxLayoutElement {
    pub item: LayoutItem,
    pub constraints: LayoutConstraints,
}

/// Internal representation of a BoxLayout
#[derive(Debug)]
pub struct BoxLayout {
    /// When true, this is a HorizonalLayout, otherwise a VerticalLayout
    pub is_horizontal: bool,
    pub elems: Vec<BoxLayoutElement>,
    pub geometry: LayoutGeometry,
}

impl BoxLayout {
    fn visit_named_references(&mut self, visitor: &mut impl FnMut(&mut NamedReference)) {
        for cell in &mut self.elems {
            cell.item.layout.as_mut().map(|x| x.visit_named_references(visitor));
            cell.constraints.visit_named_references(visitor);
        }
        self.geometry.visit_named_references(visitor);
    }
}

/// Internal representation of a path layout
#[derive(Debug)]
pub struct PathLayout {
    pub path: Path,
    pub elements: Vec<ElementRc>,
    pub rect: LayoutRect,
    pub offset_reference: NamedReference,
}

impl PathLayout {
    fn visit_named_references(&mut self, visitor: &mut impl FnMut(&mut NamedReference)) {
        self.rect.visit_named_references(visitor);
        visitor(&mut self.offset_reference);
    }
}

pub mod gen {
    use super::*;
    use crate::object_tree::Component;

    pub trait Language: Sized {
        type CompiledCode;

        /// Generate the code that instentiate the runtime struct `GridLayoutCellData` for the given cell parameter
        fn make_grid_layout_cell_data<'a, 'b>(
            item: &'a crate::layout::LayoutItem,
            constraints: &crate::layout::LayoutConstraints,
            col: u16,
            row: u16,
            colspan: u16,
            rowspan: u16,
            layout_tree: &'b mut Vec<LayoutTreeItem<'a, Self>>,
            component: &Rc<Component>,
        ) -> Self::CompiledCode;

        /// Returns a LayoutTree::GridLayout
        ///
        /// `cells` is the list of runtime `GridLayoutCellData`
        fn grid_layout_tree_item<'a, 'b>(
            layout_tree: &'b mut Vec<LayoutTreeItem<'a, Self>>,
            geometry: &'a LayoutGeometry,
            cells: Vec<Self::CompiledCode>,
            component: &Rc<Component>,
        ) -> LayoutTreeItem<'a, Self>;

        fn get_layout_info_ref<'a, 'b>(
            item: &'a LayoutItem,
            layout_tree: &'b mut Vec<LayoutTreeItem<'a, Self>>,
            component: &Rc<Component>,
        ) -> Self::CompiledCode;
    }

    #[derive(derive_more::From)]
    pub enum LayoutTreeItem<'a, L: Language> {
        GridLayout {
            geometry: &'a LayoutGeometry,
            spacing: L::CompiledCode,
            padding: L::CompiledCode,
            var_creation_code: L::CompiledCode,
            cell_ref_variable: L::CompiledCode,
        },
        PathLayout(&'a PathLayout),
    }

    pub fn collect_layouts_recursively<'a, 'b, L: Language>(
        layout_tree: &'b mut Vec<LayoutTreeItem<'a, L>>,
        layout: &'a Layout,
        component: &Rc<Component>,
    ) -> &'b LayoutTreeItem<'a, L> {
        match layout {
            Layout::GridLayout(grid_layout) => {
                let cells: Vec<_> = grid_layout
                    .elems
                    .iter()
                    .map(|cell| {
                        L::make_grid_layout_cell_data(
                            &cell.item,
                            &cell.constraints,
                            cell.col,
                            cell.row,
                            cell.colspan,
                            cell.rowspan,
                            layout_tree,
                            component,
                        )
                    })
                    .collect();

                let i =
                    L::grid_layout_tree_item(layout_tree, &grid_layout.geometry, cells, component);
                layout_tree.push(i);
            }
            Layout::BoxLayout(box_layout) => {
                let cells: Vec<_> = box_layout
                    .elems
                    .iter()
                    .enumerate()
                    .map(|(idx, cell)| {
                        let (c, r) = if box_layout.is_horizontal { (idx, 0) } else { (0, idx) };
                        L::make_grid_layout_cell_data(
                            &cell.item,
                            &cell.constraints,
                            c as u16,
                            r as u16,
                            1,
                            1,
                            layout_tree,
                            component,
                        )
                    })
                    .collect();

                let i =
                    L::grid_layout_tree_item(layout_tree, &box_layout.geometry, cells, component);
                layout_tree.push(i);
            }
            Layout::PathLayout(layout) => layout_tree.push(layout.into()),
        }
        layout_tree.last().unwrap()
    }
}
