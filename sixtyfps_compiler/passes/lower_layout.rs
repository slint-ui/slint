/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2021 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2021 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
//! Passe that compute the layout constraint

use lyon_path::geom::euclid::approxeq::ApproxEq;

use crate::diagnostics::BuildDiagnostics;
use crate::diagnostics::Spanned;
use crate::expression_tree::*;
use crate::langtype::Type;
use crate::layout::*;
use crate::object_tree::*;
use crate::typeloader::TypeLoader;
use crate::typeregister::TypeRegister;
use std::rc::Rc;

pub async fn lower_layouts(
    component: &Rc<Component>,
    type_loader: &mut TypeLoader<'_>,
    diag: &mut BuildDiagnostics,
) {
    // Ignore import errors
    let mut build_diags_to_ignore = crate::diagnostics::BuildDiagnostics::default();
    let style_metrics = type_loader
        .import_type("sixtyfps_widgets.60", "StyleMetrics", &mut build_diags_to_ignore)
        .await;
    let style_metrics =
        style_metrics.and_then(|sm| if let Type::Component(c) = sm { Some(c) } else { None });

    *component.root_constraints.borrow_mut() =
        LayoutConstraints::new(&component.root_element, diag);

    recurse_elem_including_sub_components(component, &(), &mut |elem, _| {
        let component = elem.borrow().enclosing_component.upgrade().unwrap();
        lower_element_layout(
            &component,
            elem,
            &type_loader.global_type_registry.borrow(),
            &style_metrics,
            diag,
        );
        check_no_layout_properties(elem, diag);
    });
}

fn lower_element_layout(
    component: &Rc<Component>,
    elem: &ElementRc,
    type_register: &TypeRegister,
    style_metrics: &Option<Rc<Component>>,
    diag: &mut BuildDiagnostics,
) {
    let base_type = if let Type::Builtin(base_type) = &elem.borrow().base_type {
        base_type.clone()
    } else {
        return;
    };
    match base_type.name.as_str() {
        "Row" => panic!("Error caught at element lookup time"),
        "GridLayout" => lower_grid_layout(component, elem, diag),
        "HorizontalLayout" => lower_box_layout(elem, diag, Orientation::Horizontal),
        "VerticalLayout" => lower_box_layout(elem, diag, Orientation::Vertical),
        "PathLayout" => lower_path_layout(elem, diag),
        "Dialog" => {
            lower_dialog_layout(elem, style_metrics, diag);
            return; // the Dialog stays in the tree as a Dialog
        }
        _ => return,
    };

    {
        let mut elem = elem.borrow_mut();
        let elem = &mut *elem;
        let prev_base = std::mem::replace(&mut elem.base_type, type_register.lookup("Rectangle"));
        // Create fake properties for the layout properties
        for p in elem.bindings.keys() {
            if !elem.base_type.lookup_property(p).is_valid()
                && !elem.property_declarations.contains_key(p)
            {
                let ty = prev_base.lookup_property(p).property_type;
                if ty != Type::Invalid {
                    elem.property_declarations.insert(p.into(), ty.into());
                }
            }
        }
    }
}

fn lower_grid_layout(
    component: &Rc<Component>,
    grid_layout_element: &ElementRc,
    diag: &mut BuildDiagnostics,
) {
    let mut grid = GridLayout {
        elems: Default::default(),
        geometry: LayoutGeometry::new(grid_layout_element),
        dialog_button_roles: None,
    };

    let layout_cache_prop_h =
        create_new_prop(grid_layout_element, "layout-cache-h", Type::LayoutCache);
    let layout_cache_prop_v =
        create_new_prop(grid_layout_element, "layout-cache-v", Type::LayoutCache);
    let layout_info_prop_h =
        create_new_prop(grid_layout_element, "layoutinfo-h", layout_info_type());
    let layout_info_prop_v =
        create_new_prop(grid_layout_element, "layoutinfo-v", layout_info_type());

    let mut row = 0;
    let mut col = 0;

    let layout_children = std::mem::take(&mut grid_layout_element.borrow_mut().children);
    let mut collected_children = Vec::new();
    for layout_child in layout_children {
        let is_row = if let Type::Builtin(be) = &layout_child.borrow().base_type {
            be.name == "Row"
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
                    &x,
                    (&mut row, &mut col),
                    &layout_cache_prop_h,
                    &layout_cache_prop_v,
                    diag,
                );
                col += 1;
                collected_children.push(x);
            }
            if col > 0 {
                row += 1;
                col = 0;
            }
            component.optimized_elements.borrow_mut().push(layout_child);
        } else {
            grid.add_element(
                &layout_child,
                (&mut row, &mut col),
                &layout_cache_prop_h,
                &layout_cache_prop_v,
                diag,
            );
            col += 1;
            collected_children.push(layout_child);
        }
    }
    grid_layout_element.borrow_mut().children = collected_children;
    let span = grid_layout_element.borrow().to_source_location();
    layout_cache_prop_h.element().borrow_mut().bindings.insert(
        layout_cache_prop_h.name().into(),
        BindingExpression::new_with_span(
            Expression::SolveLayout(Layout::GridLayout(grid.clone()), Orientation::Horizontal),
            span.clone(),
        ),
    );
    layout_cache_prop_v.element().borrow_mut().bindings.insert(
        layout_cache_prop_v.name().into(),
        BindingExpression::new_with_span(
            Expression::SolveLayout(Layout::GridLayout(grid.clone()), Orientation::Vertical),
            span.clone(),
        ),
    );
    layout_info_prop_h.element().borrow_mut().bindings.insert(
        layout_info_prop_h.name().into(),
        BindingExpression::new_with_span(
            Expression::ComputeLayoutInfo(
                Layout::GridLayout(grid.clone()),
                Orientation::Horizontal,
            ),
            span.clone(),
        ),
    );
    layout_info_prop_v.element().borrow_mut().bindings.insert(
        layout_info_prop_v.name().into(),
        BindingExpression::new_with_span(
            Expression::ComputeLayoutInfo(Layout::GridLayout(grid), Orientation::Vertical),
            span,
        ),
    );
    grid_layout_element.borrow_mut().layout_info_prop =
        Some((layout_info_prop_h, layout_info_prop_v));
}

impl GridLayout {
    fn add_element(
        &mut self,
        item_element: &ElementRc,
        (row, col): (&mut u16, &mut u16),
        layout_cache_prop_h: &NamedReference,
        layout_cache_prop_v: &NamedReference,
        diag: &mut BuildDiagnostics,
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

        self.add_element_with_coord(
            item_element,
            (*row, *col),
            (rowspan, colspan),
            layout_cache_prop_h,
            layout_cache_prop_v,
            diag,
        )
    }

    fn add_element_with_coord(
        &mut self,
        item_element: &ElementRc,
        (row, col): (u16, u16),
        (rowspan, colspan): (u16, u16),
        layout_cache_prop_h: &NamedReference,
        layout_cache_prop_v: &NamedReference,
        diag: &mut BuildDiagnostics,
    ) {
        let index = self.elems.len();
        if let Some(layout_item) = create_layout_item(item_element, diag) {
            if layout_item.repeater_index.is_some() {
                diag.push_error(
                    "'if' or 'for' expressions are not currently supported in grid layouts"
                        .to_string(),
                    &*item_element.borrow(),
                );
                return;
            }

            let e = &layout_item.elem;
            set_prop_from_cache(e, "x", layout_cache_prop_h, index * 2, &None, diag);
            if !layout_item.item.constraints.fixed_width {
                set_prop_from_cache(e, "width", layout_cache_prop_h, index * 2 + 1, &None, diag);
            }
            set_prop_from_cache(e, "y", layout_cache_prop_v, index * 2, &None, diag);
            if !layout_item.item.constraints.fixed_height {
                set_prop_from_cache(e, "height", layout_cache_prop_v, index * 2 + 1, &None, diag);
            }

            self.elems.push(GridLayoutElement {
                col,
                row,
                colspan,
                rowspan,
                item: layout_item.item,
            });
        }
    }
}

fn lower_box_layout(
    layout_element: &ElementRc,
    diag: &mut BuildDiagnostics,
    orientation: Orientation,
) {
    let mut layout = BoxLayout {
        orientation,
        elems: Default::default(),
        geometry: LayoutGeometry::new(layout_element),
    };

    let layout_cache_prop = create_new_prop(layout_element, "layout-cache", Type::LayoutCache);
    let layout_info_prop_v = create_new_prop(layout_element, "layoutinfo-v", layout_info_type());
    let layout_info_prop_h = create_new_prop(layout_element, "layoutinfo-h", layout_info_type());

    let layout_children = std::mem::take(&mut layout_element.borrow_mut().children);

    let (begin_padding, end_padding) = match orientation {
        Orientation::Horizontal => (&layout.geometry.padding.top, &layout.geometry.padding.bottom),
        Orientation::Vertical => (&layout.geometry.padding.left, &layout.geometry.padding.right),
    };
    let (pos, size, pad, ortho) = match orientation {
        Orientation::Horizontal => ("x", "width", "y", "height"),
        Orientation::Vertical => ("y", "height", "x", "width"),
    };
    let pad_expr = begin_padding.clone().map(Expression::PropertyReference);
    let mut size_expr = Expression::PropertyReference(NamedReference::new(layout_element, ortho));
    if let Some(p) = begin_padding {
        size_expr = Expression::BinaryExpression {
            lhs: Box::new(std::mem::take(&mut size_expr)),
            rhs: Box::new(Expression::PropertyReference(p.clone())),
            op: '-',
        }
    }
    if let Some(p) = end_padding {
        size_expr = Expression::BinaryExpression {
            lhs: Box::new(std::mem::take(&mut size_expr)),
            rhs: Box::new(Expression::PropertyReference(p.clone())),
            op: '-',
        }
    }

    for layout_child in &layout_children {
        if let Some(item) = create_layout_item(layout_child, diag) {
            let index = layout.elems.len() * 2;
            let rep_idx = &item.repeater_index;
            let (fixed_size, fixed_ortho) = match orientation {
                Orientation::Horizontal => {
                    (item.item.constraints.fixed_width, item.item.constraints.fixed_height)
                }
                Orientation::Vertical => {
                    (item.item.constraints.fixed_height, item.item.constraints.fixed_width)
                }
            };
            let actual_elem = &item.elem;
            set_prop_from_cache(actual_elem, pos, &layout_cache_prop, index, rep_idx, diag);
            if !fixed_size {
                set_prop_from_cache(
                    actual_elem,
                    size,
                    &layout_cache_prop,
                    index + 1,
                    rep_idx,
                    diag,
                );
            }
            if let Some(pad_expr) = pad_expr.clone() {
                actual_elem.borrow_mut().bindings.insert(pad.into(), pad_expr.into());
            }
            if !fixed_ortho {
                actual_elem.borrow_mut().bindings.insert(ortho.into(), size_expr.clone().into());
            }
            layout.elems.push(item.item);
        }
    }
    layout_element.borrow_mut().children = layout_children;
    let span = layout_element.borrow().to_source_location();
    layout_cache_prop.element().borrow_mut().bindings.insert(
        layout_cache_prop.name().into(),
        BindingExpression::new_with_span(
            Expression::SolveLayout(Layout::BoxLayout(layout.clone()), orientation),
            span.clone(),
        ),
    );
    layout_info_prop_h.element().borrow_mut().bindings.insert(
        layout_info_prop_h.name().into(),
        BindingExpression::new_with_span(
            Expression::ComputeLayoutInfo(
                Layout::BoxLayout(layout.clone()),
                Orientation::Horizontal,
            ),
            span.clone(),
        ),
    );
    layout_info_prop_v.element().borrow_mut().bindings.insert(
        layout_info_prop_v.name().into(),
        BindingExpression::new_with_span(
            Expression::ComputeLayoutInfo(Layout::BoxLayout(layout), Orientation::Vertical),
            span,
        ),
    );
    layout_element.borrow_mut().layout_info_prop = Some((layout_info_prop_h, layout_info_prop_v));
}

fn lower_dialog_layout(
    dialog_element: &ElementRc,
    style_metrics: &Option<Rc<Component>>,
    diag: &mut BuildDiagnostics,
) {
    let mut grid = GridLayout {
        elems: Default::default(),
        geometry: LayoutGeometry::new(dialog_element),
        dialog_button_roles: None,
    };
    if let Some(metrics) = style_metrics.as_ref().map(|comp| &comp.root_element) {
        grid.geometry.padding.bottom.get_or_insert(NamedReference::new(metrics, "layout-padding"));
        grid.geometry.padding.top.get_or_insert(NamedReference::new(metrics, "layout-padding"));
        grid.geometry.padding.left.get_or_insert(NamedReference::new(metrics, "layout-padding"));
        grid.geometry.padding.right.get_or_insert(NamedReference::new(metrics, "layout-padding"));
        grid.geometry.spacing.get_or_insert(NamedReference::new(metrics, "layout-spacing"));
    }

    let layout_cache_prop_h = create_new_prop(dialog_element, "layout-cache-h", Type::LayoutCache);
    let layout_cache_prop_v = create_new_prop(dialog_element, "layout-cache-v", Type::LayoutCache);
    let layout_info_prop_h = create_new_prop(dialog_element, "layoutinfo-h", layout_info_type());
    let layout_info_prop_v = create_new_prop(dialog_element, "layoutinfo-v", layout_info_type());

    let mut main_widget = None;
    let mut button_roles = vec![];
    for layout_child in &dialog_element.borrow().children {
        let is_button = layout_child.borrow().property_declarations.get("kind").map_or(false, |pd| {
            matches!(&pd.property_type, Type::Enumeration(e) if e.name == "StandardButtonKind")
        });

        if is_button {
            grid.add_element_with_coord(
                layout_child,
                (1, button_roles.len() as u16),
                (1, 1),
                &layout_cache_prop_h,
                &layout_cache_prop_v,
                diag,
            );
            match layout_child.borrow().bindings.get("kind") {
                None => diag.push_error(
                    "The `kind` property of the StandardButton in a Dialog must be set".into(),
                    &*layout_child.borrow(),
                ),
                Some(binding) => {
                    if let Expression::EnumerationValue(val) = &binding.expression {
                        let en = &val.enumeration;
                        debug_assert_eq!(en.name, "StandardButtonKind");
                        let role = match en.values[val.value].as_str() {
                            "ok" => "Accept",
                            "cancel" => "Reject",
                            "apply" => "Apply",
                            "close" => "Reject",
                            "reset" => "Reset",
                            "help" => "Help",
                            "yes" => "Accept",
                            "no" => "Reject",
                            "abort" => "Reject",
                            "retry" => "Accept",
                            "ignore" => "Accept",
                            _ => unreachable!(),
                        };
                        button_roles.push(role);
                    } else {
                        diag.push_error(
                            "The `kind` property of the StandardButton in a Dialog must be known at compile-time"
                                .into(),
                            binding,
                        );
                    }
                }
            }
        } else if main_widget.is_some() {
            diag.push_error(
                "A Dialog should have a single child element that is not StandardButton".into(),
                &*layout_child.borrow(),
            );
        } else {
            main_widget = Some(layout_child.clone())
        }
    }

    if let Some(main_widget) = main_widget {
        grid.add_element_with_coord(
            &main_widget,
            (0, 0),
            (1, button_roles.len() as u16 + 1),
            &layout_cache_prop_h,
            &layout_cache_prop_v,
            diag,
        );
    } else {
        diag.push_error(
            "A Dialog should have a single child element that is not StandardButton".into(),
            &*dialog_element.borrow(),
        );
    }
    grid.dialog_button_roles = Some(button_roles);

    let span = dialog_element.borrow().to_source_location();
    layout_cache_prop_h.element().borrow_mut().bindings.insert(
        layout_cache_prop_h.name().into(),
        BindingExpression::new_with_span(
            Expression::SolveLayout(Layout::GridLayout(grid.clone()), Orientation::Horizontal),
            span.clone(),
        ),
    );
    layout_cache_prop_v.element().borrow_mut().bindings.insert(
        layout_cache_prop_v.name().into(),
        BindingExpression::new_with_span(
            Expression::SolveLayout(Layout::GridLayout(grid.clone()), Orientation::Vertical),
            span.clone(),
        ),
    );
    layout_info_prop_h.element().borrow_mut().bindings.insert(
        layout_info_prop_h.name().into(),
        BindingExpression::new_with_span(
            Expression::ComputeLayoutInfo(
                Layout::GridLayout(grid.clone()),
                Orientation::Horizontal,
            ),
            span.clone(),
        ),
    );
    layout_info_prop_v.element().borrow_mut().bindings.insert(
        layout_info_prop_v.name().into(),
        BindingExpression::new_with_span(
            Expression::ComputeLayoutInfo(Layout::GridLayout(grid), Orientation::Vertical),
            span,
        ),
    );
    dialog_element.borrow_mut().layout_info_prop = Some((layout_info_prop_h, layout_info_prop_v));
}

fn lower_path_layout(layout_element: &ElementRc, diag: &mut BuildDiagnostics) {
    let layout_cache_prop = create_new_prop(layout_element, "layout-cache", Type::LayoutCache);

    let path_elements_expr = match layout_element.borrow_mut().bindings.remove("elements") {
        Some(BindingExpression { expression: Expression::PathElements { elements }, .. }) => {
            elements
        }
        _ => {
            diag.push_error("Internal error: elements binding in PathLayout does not contain path elements expression".into(), &*layout_element.borrow());
            return;
        }
    };

    let elements = layout_element.borrow().children.clone();
    if elements.is_empty() {
        return;
    }
    for (index, e) in elements.iter().enumerate() {
        let (repeater_index, actual_elem) = if e.borrow().repeated.is_some() {
            (
                Some(Expression::RepeaterIndexReference { element: Rc::downgrade(e) }),
                e.borrow().base_type.as_component().root_element.clone(),
            )
        } else {
            (None, e.clone())
        };
        let set_prop_from_cache = |prop: &str, offset: usize, size_prop: &str| {
            let size = NamedReference::new(&actual_elem, size_prop);
            let expression = Expression::BinaryExpression {
                lhs: Box::new(Expression::LayoutCacheAccess {
                    layout_cache_prop: layout_cache_prop.clone(),
                    index: index * 2 + offset,
                    repeater_index: repeater_index.as_ref().map(|x| Box::new(x.clone())),
                }),
                op: '-',
                rhs: Box::new(Expression::BinaryExpression {
                    lhs: Box::new(Expression::PropertyReference(size)),
                    op: '/',
                    rhs: Box::new(Expression::NumberLiteral(2., Unit::None)),
                }),
            };
            actual_elem.borrow_mut().bindings.insert(prop.into(), expression.into());
        };
        set_prop_from_cache("x", 0, "width");
        set_prop_from_cache("y", 1, "height");
    }
    let rect = LayoutRect::install_on_element(layout_element);
    let path_layout = Layout::PathLayout(PathLayout {
        elements,
        path: path_elements_expr,
        rect,
        offset_reference: layout_element
            .borrow()
            .bindings
            .contains_key("spacing")
            .then(|| NamedReference::new(layout_element, "spacing")),
    });
    let binding = BindingExpression::new_with_span(
        Expression::SolveLayout(path_layout, Orientation::Horizontal),
        layout_element.borrow().to_source_location(),
    );
    layout_cache_prop
        .element()
        .borrow_mut()
        .bindings
        .insert(layout_cache_prop.name().into(), binding);
}

struct CreateLayoutItemResult {
    item: LayoutItem,
    elem: ElementRc,
    repeater_index: Option<Expression>,
}

/// Create a LayoutItem for the given `item_element`  returns None is the layout is empty
fn create_layout_item(
    item_element: &ElementRc,
    diag: &mut BuildDiagnostics,
) -> Option<CreateLayoutItemResult> {
    let fix_explicit_percent = |prop: &str, item: &ElementRc| {
        if !item.borrow().bindings.get(prop).map_or(false, |b| b.ty() == Type::Percent) {
            return;
        }
        let mut item = item.borrow_mut();
        let b = item.bindings.remove(prop).unwrap();
        item.bindings.insert(format!("min-{}", prop), b.clone());
        item.bindings.insert(format!("max-{}", prop), b);
        item.property_declarations.insert(
            format!("min-{}", prop),
            PropertyDeclaration { property_type: Type::Percent, ..PropertyDeclaration::default() },
        );
        item.property_declarations.insert(
            format!("max-{}", prop),
            PropertyDeclaration { property_type: Type::Percent, ..PropertyDeclaration::default() },
        );
    };
    fix_explicit_percent("width", item_element);
    fix_explicit_percent("height", item_element);

    item_element.borrow_mut().child_of_layout = true;
    let (repeater_index, actual_elem) = if let Some(r) = &item_element.borrow().repeated {
        let rep_comp = item_element.borrow().base_type.as_component().clone();
        fix_explicit_percent("width", &rep_comp.root_element);
        fix_explicit_percent("height", &rep_comp.root_element);

        *rep_comp.root_constraints.borrow_mut() =
            LayoutConstraints::new(&rep_comp.root_element, diag);
        rep_comp.root_element.borrow_mut().child_of_layout = true;
        (
            Some(if r.is_conditional_element {
                Expression::NumberLiteral(0., Unit::None)
            } else {
                Expression::RepeaterIndexReference { element: Rc::downgrade(item_element) }
            }),
            rep_comp.root_element.clone(),
        )
    } else {
        (None, item_element.clone())
    };

    let constraints = LayoutConstraints::new(&actual_elem, diag);
    Some(CreateLayoutItemResult {
        item: LayoutItem { element: item_element.clone(), constraints },
        elem: actual_elem,
        repeater_index,
    })
}

fn set_prop_from_cache(
    elem: &ElementRc,
    prop: &str,
    layout_cache_prop: &NamedReference,
    index: usize,
    repeater_index: &Option<Expression>,
    diag: &mut BuildDiagnostics,
) {
    let old = elem.borrow_mut().bindings.insert(
        prop.into(),
        BindingExpression::new_with_span(
            Expression::LayoutCacheAccess {
                layout_cache_prop: layout_cache_prop.clone(),
                index,
                repeater_index: repeater_index.as_ref().map(|x| Box::new(x.clone())),
            },
            layout_cache_prop.element().borrow().to_source_location(),
        ),
    );
    if let Some(old) = old {
        diag.push_error(
            format!("The property '{}' cannot be set for elements placed in a layout, because the layout is already setting it", prop),
            &old,
        );
    }
}

fn eval_const_expr(
    expression: &Expression,
    name: &str,
    span: &dyn crate::diagnostics::Spanned,
    diag: &mut BuildDiagnostics,
) -> Option<u16> {
    match expression {
        Expression::NumberLiteral(v, Unit::None) => {
            if *v < 0. || *v > u16::MAX as f64 || !v.trunc().approx_eq(v) {
                diag.push_error(format!("'{}' must be a positive integer", name), span);
                None
            } else {
                Some(*v as u16)
            }
        }
        Expression::Cast { from, .. } => eval_const_expr(from, name, span, diag),
        _ => {
            diag.push_error(format!("'{}' must be an integer literal", name), span);
            None
        }
    }
}

/// Create a new property based on the name. (it might get a different name if that property exist)
pub fn create_new_prop(elem: &ElementRc, tentative_name: &str, ty: Type) -> NamedReference {
    let mut e = elem.borrow_mut();
    if !e.lookup_property(tentative_name).is_valid() {
        e.property_declarations.insert(tentative_name.into(), ty.into());
        drop(e);
        NamedReference::new(elem, tentative_name)
    } else {
        let mut counter = 0;
        loop {
            counter += 1;
            let name = format!("{}{}", tentative_name, counter);
            if !e.lookup_property(&name).is_valid() {
                e.property_declarations.insert(name.clone(), ty.into());
                drop(e);
                return NamedReference::new(elem, &name);
            }
        }
    }
}

/// Checks that there is grid-layout specific properties left
fn check_no_layout_properties(item: &ElementRc, diag: &mut BuildDiagnostics) {
    for (prop, expr) in item.borrow().bindings.iter() {
        if matches!(prop.as_ref(), "col" | "row" | "colspan" | "rowspan") {
            diag.push_error(format!("{} used outside of a GridLayout", prop), expr);
        }
    }
}
