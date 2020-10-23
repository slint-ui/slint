/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
//! Passe that compute the layout constraint

use crate::diagnostics::BuildDiagnostics;
use crate::expression_tree::*;
use crate::langtype::Type;
use crate::layout::*;
use crate::object_tree::*;
use std::rc::Rc;

fn property_reference(element: &ElementRc, name: &str) -> Box<Expression> {
    Box::new(Expression::PropertyReference(NamedReference {
        element: Rc::downgrade(element),
        name: name.into(),
    }))
}

pub fn binding_reference(element: &ElementRc, name: &str) -> Option<Expression> {
    if element.borrow().bindings.contains_key(name) {
        Some(Expression::PropertyReference(NamedReference {
            element: Rc::downgrade(element),
            name: name.into(),
        }))
    } else {
        None
    }
}

pub fn init_fake_property(
    grid_layout_element: &ElementRc,
    name: &str,
    lazy_default: impl Fn() -> Option<Expression>,
) {
    if grid_layout_element.borrow().property_declarations.contains_key(name)
        && !grid_layout_element.borrow().bindings.contains_key(name)
    {
        if let Some(e) = lazy_default() {
            grid_layout_element.borrow_mut().bindings.insert(name.to_owned(), e.into());
        }
    }
}

fn lower_grid_layout(
    component: &Rc<Component>,
    rect: LayoutRect,
    grid_layout_element: &ElementRc,
    collected_children: &mut Vec<ElementRc>,
    diag: &mut BuildDiagnostics,
) -> Option<Layout> {
    let mut grid = GridLayout {
        elems: Default::default(),
        geometry: LayoutGeometry::new(rect, &grid_layout_element),
    };

    let mut row = 0;
    let mut col = 0;

    let layout_children = std::mem::take(&mut grid_layout_element.borrow_mut().children);
    for layout_child in layout_children {
        let is_row = if let Type::Builtin(be) = &layout_child.borrow().base_type {
            be.native_class.class_name == "Row"
        } else {
            false
        };
        if is_row {
            if col > 0 {
                row += 1;
                col = 0;
            }
            let row_children = std::mem::take(&mut layout_child.borrow_mut().children);
            for x in row_children {
                grid.add_element(x, &mut row, &mut col, diag, &component, collected_children);
                col += 1;
            }
            component.optimized_elements.borrow_mut().push(layout_child.clone());
        } else {
            grid.add_element(
                layout_child,
                &mut row,
                &mut col,
                diag,
                &component,
                collected_children,
            );
            col += 1;
        }
    }
    component.optimized_elements.borrow_mut().push(grid_layout_element.clone());
    if !grid.elems.is_empty() {
        Some(grid.into())
    } else {
        None
    }
}

fn lower_box_layout(
    component: &Rc<Component>,
    rect: LayoutRect,
    layout_element: &ElementRc,
    collected_children: &mut Vec<ElementRc>,
    diag: &mut BuildDiagnostics,
) -> Option<Layout> {
    let is_horizontal = layout_element.borrow().base_type.to_string() == "HorizontalLayout";
    let mut layout = BoxLayout {
        is_horizontal,
        elems: Default::default(),
        geometry: LayoutGeometry::new(rect, &layout_element),
    };
    let layout_children = std::mem::take(&mut layout_element.borrow_mut().children);
    for layout_child in layout_children {
        if let Some(item) = create_layout_item(&layout_child, component, collected_children, diag) {
            layout
                .elems
                .push(BoxLayoutElement { item, constraints: LayoutConstraints::new(&layout_child) })
        }
    }
    component.optimized_elements.borrow_mut().push(layout_element.clone());
    if !layout.elems.is_empty() {
        Some(layout.into())
    } else {
        None
    }
}

fn lower_path_layout(
    component: &Rc<Component>,
    rect: LayoutRect,
    path_layout_element: &ElementRc,
    collected_children: &mut Vec<ElementRc>,
    diag: &mut BuildDiagnostics,
) -> Option<Layout> {
    let layout_children = std::mem::take(&mut path_layout_element.borrow_mut().children);
    collected_children.extend(layout_children.iter().cloned());
    component.optimized_elements.borrow_mut().push(path_layout_element.clone());
    let path_elements_expr = match path_layout_element.borrow_mut().bindings.remove("elements") {
        Some(ExpressionSpanned { expression: Expression::PathElements { elements }, .. }) => {
            elements
        }
        _ => {
            diag.push_error("Internal error: elements binding in PathLayout does not contain path elements expression".into(), &*path_layout_element.borrow());
            return None;
        }
    };

    if layout_children.is_empty() {
        return None;
    }

    let rect = LayoutRect {
        x_reference: property_reference(path_layout_element, "x"),
        y_reference: property_reference(path_layout_element, "y"),
        width_reference: rect.width_reference,
        height_reference: rect.height_reference,
    };

    Some(
        PathLayout {
            elements: layout_children,
            path: path_elements_expr,
            rect,
            offset_reference: property_reference(path_layout_element, "offset"),
        }
        .into(),
    )
}

fn layout_parse_function(
    layout_element_candidate: &ElementRc,
) -> Option<
    &'static dyn Fn(
        &Rc<Component>,
        LayoutRect,
        &ElementRc,
        &mut Vec<ElementRc>,
        &mut BuildDiagnostics,
    ) -> Option<Layout>,
> {
    if let Type::Builtin(be) = &layout_element_candidate.borrow().base_type {
        match be.native_class.class_name.as_str() {
            "Row" => panic!("Error caught at element lookup time"),
            "GridLayout" => Some(&lower_grid_layout),
            "HorizontalLayout" => Some(&lower_box_layout),
            "VerticalLayout" => Some(&lower_box_layout),
            "PathLayout" => Some(&lower_path_layout),
            _ => None,
        }
    } else {
        None
    }
}

fn lower_element_layout(
    component: &Rc<Component>,
    elem: &ElementRc,
    diag: &mut BuildDiagnostics,
) -> Vec<Layout> {
    let old_children = {
        let mut elem = elem.borrow_mut();
        let new_children = Vec::with_capacity(elem.children.len());
        std::mem::replace(&mut elem.children, new_children)
    };

    // lay out within the current element's boundaries.
    let rect_to_layout = LayoutRect {
        x_reference: Box::new(Expression::NumberLiteral(0., Unit::Phx)),
        y_reference: Box::new(Expression::NumberLiteral(0., Unit::Phx)),
        width_reference: property_reference(elem, "width"),
        height_reference: property_reference(elem, "height"),
    };

    let mut found_layouts = Vec::new();

    for child in old_children {
        if let Some(layout_parser) = layout_parse_function(&child) {
            let mut children = std::mem::take(&mut elem.borrow_mut().children);
            if let Some(layout) =
                layout_parser(component, rect_to_layout.clone(), &child, &mut children, diag)
            {
                found_layouts.push(layout);
            }
            elem.borrow_mut().children = children;
            continue;
        } else {
            if !child.borrow().child_of_layout {
                check_no_layout_properties(&child, diag);
                // Don't check again in case we reach this element a second time, to avoid duplicate errors
                child.borrow_mut().child_of_layout = true;
            }
            elem.borrow_mut().children.push(child);
        }
    }

    found_layouts
}

/// Currently this just removes the layout from the tree
pub fn lower_layouts(component: &Rc<Component>, diag: &mut BuildDiagnostics) {
    recurse_elem(&component.root_element, &(), &mut |elem, _| {
        let mut layouts = lower_element_layout(component, elem, diag);
        component.layouts.borrow_mut().append(&mut layouts);

        if elem.borrow().repeated.is_some() {
            if let Type::Component(base) = &elem.borrow().base_type {
                lower_layouts(base, diag);
            }
        }
    });
    check_no_layout_properties(&component.root_element, diag);
}

/// Create a LayoutElement for the given `item_element`  returns None is the layout is empty
fn create_layout_item(
    item_element: &ElementRc,
    component: &Rc<Component>,
    collected_children: &mut Vec<ElementRc>,
    diag: &mut BuildDiagnostics,
) -> Option<LayoutItem> {
    if let Some(nested_layout_parser) = layout_parse_function(item_element) {
        let layout_rect = LayoutRect::install_on_element(&item_element);

        nested_layout_parser(component, layout_rect, &item_element, collected_children, diag)
            .map(|x| Box::new(x).into())
    } else {
        item_element.borrow_mut().child_of_layout = true;
        collected_children.push(item_element.clone());
        let element = item_element.clone();
        let layout = {
            let mut layouts = lower_element_layout(component, &element, diag);
            if layouts.is_empty() {
                None
            } else {
                Some(layouts.remove(0))
            }
        };
        Some(LayoutElement { element, layout }.into())
    }
}

impl GridLayout {
    fn add_element(
        &mut self,
        item_element: ElementRc,
        row: &mut u16,
        col: &mut u16,
        diag: &mut BuildDiagnostics,
        component: &Rc<Component>,
        collected_children: &mut Vec<ElementRc>,
    ) {
        let mut get_const_value = |name: &str| {
            item_element
                .borrow()
                .bindings
                .get(name)
                .and_then(|e| eval_const_expr(&e.expression, name, e, diag))
        };
        let colspan = get_const_value("colspan").unwrap_or(1);
        let rowspan = get_const_value("rowspan").unwrap_or(1);
        if let Some(r) = get_const_value("row") {
            *row = r;
            *col = 0;
        }
        if let Some(c) = get_const_value("col") {
            *col = c;
        }

        if let Some(layout_item) =
            create_layout_item(&item_element, component, collected_children, diag)
        {
            self.elems.push(GridLayoutElement {
                col: *col,
                row: *row,
                colspan,
                rowspan,
                item: layout_item,
                constraints: LayoutConstraints::new(&item_element),
            });
        }
    }
}

pub fn find_expression(name: &str, item_element: &ElementRc) -> Option<Box<Expression>> {
    item_element.borrow().bindings.get(name).map(|_| property_reference(item_element, name))
}

fn eval_const_expr(
    expression: &Expression,
    name: &str,
    span: &dyn crate::diagnostics::SpannedWithSourceFile,
    diag: &mut BuildDiagnostics,
) -> Option<u16> {
    match expression {
        Expression::NumberLiteral(v, Unit::None) => {
            let r = *v as u16;
            if r as f32 != *v as f32 {
                diag.push_error(format!("'{}' must be a positive integer", name), span);
                None
            } else {
                Some(r)
            }
        }
        Expression::Cast { from, .. } => eval_const_expr(&from, name, span, diag),
        _ => {
            diag.push_error(format!("'{}' must be an integer literal", name), span);
            None
        }
    }
}

fn check_no_layout_properties(item: &ElementRc, diag: &mut BuildDiagnostics) {
    for (prop, expr) in item.borrow().bindings.iter() {
        if matches!(prop.as_ref(), "col" | "row" | "colspan" | "rowspan") {
            diag.push_error(format!("{} used outside of a GridLayout", prop), expr);
        }
    }
}
