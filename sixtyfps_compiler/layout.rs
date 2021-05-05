/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
//! Datastructures used to represent layouts in the compiler

use crate::diagnostics::BuildDiagnostics;
use crate::expression_tree::{Expression, NamedReference, Path};
use crate::langtype::{PropertyLookupResult, Type};
use crate::object_tree::{Component, ElementRc};

use std::rc::Rc;

#[derive(Clone, Debug, derive_more::From)]
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

/// An Item in the layout tree
#[derive(Debug, Default, Clone)]
pub struct LayoutItem {
    pub element: ElementRc,
    pub constraints: LayoutConstraints,
}

impl LayoutItem {
    pub fn rect(&self) -> LayoutRect {
        let p = |unresolved_name: &str| {
            let PropertyLookupResult { resolved_name, property_type } =
                self.element.borrow().lookup_property(unresolved_name);
            if property_type == Type::LogicalLength {
                Some(NamedReference::new(&self.element, resolved_name.as_ref()))
            } else {
                None
            }
        };
        LayoutRect {
            x_reference: p("x"),
            y_reference: p("y"),
            width_reference: if !self.constraints.fixed_width { p("width") } else { None },
            height_reference: if !self.constraints.fixed_height { p("height") } else { None },
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
        let install_prop = |name: &str| Some(NamedReference::new(element, name));

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

#[derive(Debug, Default, Clone)]
pub struct LayoutConstraints {
    pub minimum_width: Option<NamedReference>,
    pub maximum_width: Option<NamedReference>,
    pub minimum_height: Option<NamedReference>,
    pub maximum_height: Option<NamedReference>,
    pub preferred_width: Option<NamedReference>,
    pub preferred_height: Option<NamedReference>,
    pub horizontal_stretch: Option<NamedReference>,
    pub vertical_stretch: Option<NamedReference>,
    pub fixed_width: bool,
    pub fixed_height: bool,
}

impl LayoutConstraints {
    pub fn new(element: &ElementRc, diag: &mut BuildDiagnostics) -> Self {
        let mut constraints = Self {
            minimum_width: binding_reference(&element, "minimum_width"),
            maximum_width: binding_reference(&element, "maximum_width"),
            minimum_height: binding_reference(&element, "minimum_height"),
            maximum_height: binding_reference(&element, "maximum_height"),
            preferred_width: binding_reference(&element, "preferred_width"),
            preferred_height: binding_reference(&element, "preferred_height"),
            horizontal_stretch: binding_reference(&element, "horizontal_stretch"),
            vertical_stretch: binding_reference(&element, "vertical_stretch"),
            fixed_width: false,
            fixed_height: false,
        };
        let mut apply_size_constraint = |prop, binding, op: &mut Option<NamedReference>| {
            if let Some(other_prop) = op {
                diag.push_error(
                    format!("Cannot specity both {} and {}.", prop, other_prop.name()),
                    binding,
                )
            }
            *op = Some(NamedReference::new(element, prop))
        };
        let e = element.borrow();
        e.bindings.get("height").map(|s| {
            constraints.fixed_height = true;
            apply_size_constraint("height", s, &mut constraints.minimum_height);
            apply_size_constraint("height", s, &mut constraints.maximum_height);
        });
        e.bindings.get("width").map(|s| {
            if s.expression.ty() == Type::Percent {
                apply_size_constraint("width", s, &mut constraints.minimum_width);
                return;
            }
            constraints.fixed_width = true;
            apply_size_constraint("width", s, &mut constraints.minimum_width);
            apply_size_constraint("width", s, &mut constraints.maximum_width);
        });

        constraints
    }

    pub fn has_explicit_restrictions(&self) -> bool {
        self.minimum_width.is_some()
            || self.maximum_width.is_some()
            || self.minimum_height.is_some()
            || self.maximum_height.is_some()
            || self.horizontal_stretch.is_some()
            || self.vertical_stretch.is_some()
    }

    // Iterate over the constraint with a reference to a property, and the corresponding member in the sixtyfps_corelib::layout::LayoutInfo struct
    pub fn for_each_restrictions<'a>(
        &'a self,
    ) -> impl Iterator<Item = (&NamedReference, &'static str)> {
        std::iter::empty()
            .chain(self.minimum_width.as_ref().map(|x| {
                if Expression::PropertyReference(x.clone()).ty() != Type::Percent {
                    (x, "min_width")
                } else {
                    (x, "min_width_percent")
                }
            }))
            .chain(self.maximum_width.as_ref().map(|x| {
                if Expression::PropertyReference(x.clone()).ty() != Type::Percent {
                    (x, "max_width")
                } else {
                    (x, "max_width_percent")
                }
            }))
            .chain(self.minimum_height.as_ref().map(|x| {
                if Expression::PropertyReference(x.clone()).ty() != Type::Percent {
                    (x, "min_height")
                } else {
                    (x, "min_height_percent")
                }
            }))
            .chain(self.maximum_height.as_ref().map(|x| {
                if Expression::PropertyReference(x.clone()).ty() != Type::Percent {
                    (x, "max_height")
                } else {
                    (x, "max_height_percent")
                }
            }))
            .chain(self.preferred_width.as_ref().map(|x| (x, "preferred_width")))
            .chain(self.preferred_height.as_ref().map(|x| (x, "preferred_height")))
            .chain(self.horizontal_stretch.as_ref().map(|x| (x, "horizontal_stretch")))
            .chain(self.vertical_stretch.as_ref().map(|x| (x, "vertical_stretch")))
    }

    pub fn visit_named_references(&mut self, visitor: &mut impl FnMut(&mut NamedReference)) {
        self.maximum_width.as_mut().map(|e| visitor(&mut *e));
        self.minimum_width.as_mut().map(|e| visitor(&mut *e));
        self.maximum_height.as_mut().map(|e| visitor(&mut *e));
        self.minimum_height.as_mut().map(|e| visitor(&mut *e));
        self.preferred_width.as_mut().map(|e| visitor(&mut *e));
        self.preferred_height.as_mut().map(|e| visitor(&mut *e));
        self.horizontal_stretch.as_mut().map(|e| visitor(&mut *e));
        self.vertical_stretch.as_mut().map(|e| visitor(&mut *e));
    }
}

/// An element in a GridLayout
#[derive(Debug, Clone)]
pub struct GridLayoutElement {
    pub col: u16,
    pub row: u16,
    pub colspan: u16,
    pub rowspan: u16,
    pub item: LayoutItem,
}

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
pub struct LayoutGeometry {
    pub rect: LayoutRect,
    pub spacing: Option<NamedReference>,
    pub alignment: Option<NamedReference>,
    pub padding: Padding,
}

impl LayoutGeometry {
    fn visit_named_references(&mut self, visitor: &mut impl FnMut(&mut NamedReference)) {
        self.rect.visit_named_references(visitor);
        self.spacing.as_mut().map(|e| visitor(&mut *e));
        self.alignment.as_mut().map(|e| visitor(&mut *e));
        self.padding.visit_named_references(visitor);
    }
}

/// Return a named reference to a property if a binding is set on that property
fn binding_reference(element: &ElementRc, name: &str) -> Option<NamedReference> {
    element.borrow().bindings.contains_key(name).then(|| NamedReference::new(element, name))
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
            if e.name() == name && Rc::ptr_eq(&e.element(), grid_layout_element) {
                // Don't reference self
                return;
            }
            grid_layout_element
                .borrow_mut()
                .bindings
                .insert(name.to_owned(), Expression::PropertyReference(e).into());
        }
    }
}

impl LayoutGeometry {
    pub fn new(layout_element: &ElementRc, style_metrics: &Option<Rc<Component>>) -> Self {
        let style_metrics_element = style_metrics.as_ref().map(|comp| comp.root_element.clone());

        let padding = || {
            let style_metrics_element = style_metrics_element.clone();
            binding_reference(layout_element, "padding").or_else(|| {
                style_metrics_element.map(|metrics| NamedReference::new(&metrics, "layout_padding"))
            })
        };
        let spacing = binding_reference(layout_element, "spacing").or_else({
            let style_metrics_element = style_metrics_element.clone();
            move || {
                style_metrics_element.map(|metrics| NamedReference::new(&metrics, "layout_spacing"))
            }
        });
        let alignment = binding_reference(layout_element, "alignment");

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

        let rect = LayoutRect::install_on_element(layout_element);

        Self { rect, spacing, padding, alignment }
    }
}

/// Internal representation of a grid layout
#[derive(Debug, Clone)]
pub struct GridLayout {
    /// All the elements will be layout within that element.
    pub elems: Vec<GridLayoutElement>,

    pub geometry: LayoutGeometry,
}

impl GridLayout {
    fn visit_named_references(&mut self, visitor: &mut impl FnMut(&mut NamedReference)) {
        for cell in &mut self.elems {
            cell.item.constraints.visit_named_references(visitor);
        }
        self.geometry.visit_named_references(visitor);
    }
}

/// Internal representation of a BoxLayout
#[derive(Debug, Clone)]
pub struct BoxLayout {
    /// When true, this is a HorizonalLayout, otherwise a VerticalLayout
    pub is_horizontal: bool,
    pub elems: Vec<LayoutItem>,
    pub geometry: LayoutGeometry,
}

impl BoxLayout {
    fn visit_named_references(&mut self, visitor: &mut impl FnMut(&mut NamedReference)) {
        for cell in &mut self.elems {
            cell.constraints.visit_named_references(visitor);
        }
        self.geometry.visit_named_references(visitor);
    }
}

/// Internal representation of a path layout
#[derive(Debug, Clone)]
pub struct PathLayout {
    pub path: Path,
    pub elements: Vec<ElementRc>,
    pub rect: LayoutRect,
    pub offset_reference: Option<NamedReference>,
}

impl PathLayout {
    fn visit_named_references(&mut self, visitor: &mut impl FnMut(&mut NamedReference)) {
        self.rect.visit_named_references(visitor);
        self.offset_reference.as_mut().map(visitor);
    }
}

/// The [`Type`] for a runtime LayoutInfo structure
pub fn layout_info_type() -> Type {
    Type::Struct {
        fields: [
            "min_width",
            "min_height",
            "max_width",
            "max_height",
            "preferred_width",
            "preferred_height",
        ]
        .iter()
        .map(|s| (s.to_string(), Type::LogicalLength))
        .chain(
            [
                "min_width_percent",
                "min_height_percent",
                "max_width_percent",
                "max_height_percent",
                "horizontal_stretch",
                "vertical_stretch",
            ]
            .iter()
            .map(|s| (s.to_string(), Type::Float32)),
        )
        .collect(),
        name: Some("LayoutInfo".into()),
        node: None,
    }
}
