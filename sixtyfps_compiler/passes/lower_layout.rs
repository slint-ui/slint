/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
//! Passe that compute the layout constraint

use crate::expression_tree::*;
use crate::langtype::Type;
use crate::layout::*;
use crate::object_tree::*;
use crate::typeregister::TypeRegister;
use crate::{diagnostics::BuildDiagnostics, typeloader::TypeLoader};
use std::cell::RefCell;
use std::rc::Rc;

fn lower_grid_layout(
    component: &Rc<Component>,
    rect: LayoutRect,
    grid_layout_element: &ElementRc,
    collected_children: &mut Vec<ElementRc>,
    type_register: &TypeRegister,
    style_metrics: &Option<Rc<Component>>,
    diag: &mut BuildDiagnostics,
) -> Option<Layout> {
    let mut grid = GridLayout {
        elems: Default::default(),
        geometry: LayoutGeometry::new(rect, &grid_layout_element, style_metrics),
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
                grid.add_element(
                    x,
                    (&mut row, &mut col),
                    diag,
                    type_register,
                    style_metrics,
                    &component,
                    collected_children,
                );
                col += 1;
            }
            component.optimized_elements.borrow_mut().push(layout_child.clone());
        } else {
            grid.add_element(
                layout_child,
                (&mut row, &mut col),
                diag,
                type_register,
                style_metrics,
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
    type_register: &TypeRegister,
    style_metrics: &Option<Rc<Component>>,
    diag: &mut BuildDiagnostics,
) -> Option<Layout> {
    let is_horizontal = layout_element.borrow().base_type.to_string() == "HorizontalLayout";
    let mut layout = BoxLayout {
        is_horizontal,
        elems: Default::default(),
        geometry: LayoutGeometry::new(rect, &layout_element, style_metrics),
    };
    let layout_children = std::mem::take(&mut layout_element.borrow_mut().children);
    for layout_child in layout_children {
        if let Some(item) = create_layout_item(
            &layout_child,
            component,
            collected_children,
            type_register,
            style_metrics,
            diag,
        ) {
            layout.elems.push(item)
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
    _type_register: &TypeRegister,
    _style_metrics: &Option<Rc<Component>>,
    diag: &mut BuildDiagnostics,
) -> Option<Layout> {
    let layout_children = std::mem::take(&mut path_layout_element.borrow_mut().children);
    collected_children.extend(layout_children.iter().cloned());
    component.optimized_elements.borrow_mut().push(path_layout_element.clone());
    let path_elements_expr = match path_layout_element.borrow_mut().bindings.remove("elements") {
        Some(BindingExpression { expression: Expression::PathElements { elements }, .. }) => {
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
        x_reference: Some(NamedReference::new(path_layout_element, "x")),
        y_reference: Some(NamedReference::new(path_layout_element, "y")),
        width_reference: rect.width_reference,
        height_reference: rect.height_reference,
    };

    Some(
        PathLayout {
            elements: layout_children,
            path: path_elements_expr,
            rect,
            offset_reference: NamedReference::new(path_layout_element, "offset"),
        }
        .into(),
    )
}

type LayoutParseFunction = dyn Fn(
    &Rc<Component>,
    LayoutRect,
    &ElementRc,
    &mut Vec<ElementRc>,
    &TypeRegister,
    &Option<Rc<Component>>,
    &mut BuildDiagnostics,
) -> Option<Layout>;

fn layout_parse_function(
    layout_element_candidate: &ElementRc,
) -> Option<&'static LayoutParseFunction> {
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
    type_register: &TypeRegister,
    style_metrics: &Option<Rc<Component>>,
    diag: &mut BuildDiagnostics,
) -> LayoutVec {
    let old_children = {
        let mut elem = elem.borrow_mut();
        let new_children = Vec::with_capacity(elem.children.len());
        std::mem::replace(&mut elem.children, new_children)
    };

    // lay out within the current element's boundaries.
    let rect_to_layout = LayoutRect {
        x_reference: None,
        y_reference: None,
        width_reference: Some(NamedReference::new(elem, "width")),
        height_reference: Some(NamedReference::new(elem, "height")),
    };

    let mut found_layouts = LayoutVec::default();

    for child in old_children {
        if let Some(layout_parser) = layout_parse_function(&child) {
            let mut children = std::mem::take(&mut elem.borrow_mut().children);
            if let Some(layout) = layout_parser(
                component,
                rect_to_layout.clone(),
                &child,
                &mut children,
                type_register,
                style_metrics,
                diag,
            ) {
                if Rc::ptr_eq(elem, &component.root_element) {
                    found_layouts.main_layout = Some(found_layouts.len());
                }
                found_layouts.push(layout);
            }
            elem.borrow_mut().children = children;
            continue;
        } else {
            elem.borrow_mut().children.push(child);
        }
    }

    found_layouts
}

/// Currently this just removes the layout from the tree
pub async fn lower_layouts<'a>(
    component: &Rc<Component>,
    type_loader: &mut TypeLoader<'a>,
    diag: &mut BuildDiagnostics,
) {
    // Ignore import errors
    let mut file_diags_to_ignore = crate::diagnostics::FileDiagnostics::default();
    let mut build_diags_to_ignore = crate::diagnostics::BuildDiagnostics::default();
    let style_metrics = type_loader
        .import_type(
            "sixtyfps_widgets.60",
            "StyleMetrics",
            &mut file_diags_to_ignore,
            &mut build_diags_to_ignore,
        )
        .await;
    let style_metrics =
        style_metrics.and_then(|sm| if let Type::Component(c) = sm { Some(c) } else { None });
    lower_layouts_impl(component, &type_loader.global_type_registry.borrow(), &style_metrics, diag);
}

fn lower_layouts_impl(
    component: &Rc<Component>,
    type_register: &TypeRegister,
    style_metrics: &Option<Rc<Component>>,
    diag: &mut BuildDiagnostics,
) {
    // FIXME: one should enable minimum_width and minimum_height on the window, but not height and width
    //component.layouts.borrow_mut().root_constraints =
    //    LayoutConstraints::new(&component.root_element, diag);

    recurse_elem_including_sub_components(&component, &(), &mut |elem, _| {
        let component = elem.borrow().enclosing_component.upgrade().unwrap();
        let mut layouts =
            lower_element_layout(&component, elem, type_register, style_metrics, diag);
        let mut component_layouts = component.layouts.borrow_mut();
        component_layouts.main_layout = component_layouts
            .main_layout
            .or_else(|| layouts.main_layout.map(|x| x + component_layouts.len()));
        component_layouts.append(&mut layouts);
        check_no_layout_properties(elem, diag);
    });
}

/// Create a LayoutItem for the given `item_element`  returns None is the layout is empty
fn create_layout_item(
    item_element: &ElementRc,
    component: &Rc<Component>,
    collected_children: &mut Vec<ElementRc>,
    type_register: &TypeRegister,
    style_metrics: &Option<Rc<Component>>,
    diag: &mut BuildDiagnostics,
) -> Option<LayoutItem> {
    let fix_explicit_percent = |prop: &str, item: &ElementRc| {
        if !item.borrow().bindings.get(prop).map_or(false, |b| b.ty() == Type::Percent) {
            return;
        }
        let mut item = item.borrow_mut();
        let b = item.bindings.remove(prop).unwrap();
        // FIXME: this should be the preferred size instead, progably
        item.bindings.insert(format!("minimum_{}", prop), b.clone());
        item.bindings.insert(format!("maximum_{}", prop), b);
        item.property_declarations.insert(
            format!("minimum_{}", prop),
            PropertyDeclaration { property_type: Type::Percent, ..PropertyDeclaration::default() },
        );
        item.property_declarations.insert(
            format!("maximum_{}", prop),
            PropertyDeclaration { property_type: Type::Percent, ..PropertyDeclaration::default() },
        );
    };
    fix_explicit_percent("width", item_element);
    fix_explicit_percent("height", item_element);

    let constraints = LayoutConstraints::new(item_element, diag);

    item_element.borrow_mut().child_of_layout = true;

    if item_element.borrow().repeated.is_some() {
        let rep_comp = item_element.borrow().base_type.as_component().clone();
        fix_explicit_percent("width", &rep_comp.root_element);
        fix_explicit_percent("height", &rep_comp.root_element);

        rep_comp.layouts.borrow_mut().root_constraints =
            LayoutConstraints::new(&rep_comp.root_element, diag);
        rep_comp.root_element.borrow_mut().child_of_layout = true;
        collected_children.push(item_element.clone());

        if layout_parse_function(&rep_comp.root_element).is_some() {
            let new_root = ElementRc::new(RefCell::new(Element {
                id: format!("{}_rootrect", item_element.borrow().id),
                base_type: type_register.lookup_element("Rectangle").unwrap(),
                enclosing_component: Rc::downgrade(&rep_comp),
                ..Default::default()
            }));
            drop(rep_comp);
            crate::object_tree::inject_element_as_repeated_element(item_element, new_root);
        }

        Some(LayoutItem { element: Some(item_element.clone()), layout: None, constraints })
    } else if let Some(nested_layout_parser) = layout_parse_function(item_element) {
        let layout_rect = LayoutRect::install_on_element(&item_element);

        nested_layout_parser(
            component,
            layout_rect,
            &item_element,
            collected_children,
            type_register,
            style_metrics,
            diag,
        )
        .map(|x| LayoutItem { layout: Some(x), constraints, element: None })
    } else {
        collected_children.push(item_element.clone());
        let element = item_element.clone();
        let layout = {
            let mut layouts =
                lower_element_layout(component, &element, type_register, style_metrics, diag);
            if layouts.is_empty() {
                None
            } else {
                Some(layouts.remove(0))
            }
        };
        Some(LayoutItem { element: Some(element), layout, constraints })
    }
}

impl GridLayout {
    fn add_element(
        &mut self,
        item_element: ElementRc,
        (row, col): (&mut u16, &mut u16),
        diag: &mut BuildDiagnostics,
        type_register: &TypeRegister,
        style_metrics: &Option<Rc<Component>>,
        component: &Rc<Component>,
        collected_children: &mut Vec<ElementRc>,
    ) {
        let mut get_const_value = |name: &str| {
            item_element
                .borrow_mut()
                .bindings
                .remove(name)
                .and_then(|e| eval_const_expr(&e.expression, name, &e, diag))
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

        if let Some(layout_item) = create_layout_item(
            &item_element,
            component,
            collected_children,
            type_register,
            style_metrics,
            diag,
        ) {
            self.elems.push(GridLayoutElement {
                col: *col,
                row: *row,
                colspan,
                rowspan,
                item: layout_item,
            });
        }
    }
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
            if (r as f32 - *v as f32).abs() > f32::EPSILON {
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
