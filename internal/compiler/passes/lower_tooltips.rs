// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Lowers `ToolTip { text: ... }` to an input-transparent popup overlay.
//!
//! For each `ToolTip` child, this pass synthesizes a `PopupWindow` anchored around
//! the hovered parent element and contains the tooltip content.
//! The `ToolTip.placement` enum controls whether it appears at the mouse pointer position
//! (`pointer`) or relative to the hovered element (`above-element`/`below-element`/`left-element`/`right-element`).
//! Visibility is driven by runtime behavior in `TooltipArea`:
//! - hover enters: start/restart internal delay timer
//! - timer fires: invoke `show()` callback
//! - hover leaves: stop timer and invoke `hide()` callback
//!   `TooltipArea` also tracks the last known pointer position (`mouse-x`/`mouse-y`) for
//!   the `pointer` placement mode.
//!
//! Runtime popup handling marks tooltip popups as input-transparent overlays.
//! Tooltip show/hide delay uses `ToolTip.delay`.
//!
//! Tooltip content contract:
//! - A parent element may have **at most one** `ToolTip` child.
//! - `ToolTip` supports exactly one content mode:
//!   - text mode: `text` binding is present, no children
//!   - custom mode: children are present, no `text` binding
//! - placement uses effective popup size:
//!   - explicit size from tooltip content (`width`/`height`) if set (> 0)
//!   - otherwise `preferred-width`/`preferred-height`
//! - custom mode expects one root child element which is used directly as tooltip content.

use crate::diagnostics::{BuildDiagnostics, Spanned};
use crate::expression_tree::{BindingExpression, BuiltinFunction, Expression, Unit};
use crate::langtype::{ElementType, Enumeration, EnumerationValue, Type};
use crate::namedreference::NamedReference;
use crate::object_tree::*;
use crate::typeregister::{BUILTIN, TypeRegister};
use smol_str::{SmolStr, format_smolstr};
use std::cell::RefCell;
use std::rc::Rc;

const TOOLTIP_ELEMENT: &str = "ToolTip";
const TOOLTIP_IMPL_ELEMENT: &str = "ToolTipImpl";
const TOOLTIP_AREA_ELEMENT: &str = "TooltipArea";
const POPUP_WINDOW_ELEMENT: &str = "PopupWindow";
const TOOLTIP_POPUP_ID_PREFIX: &str = "tooltip-popup-overlay-";
const LAYOUT_ELEMENTS_DISALLOWING_TOOLTIP: &[&str] =
    &["GridLayout", "VerticalLayout", "HorizontalLayout", "FlexboxLayout"];

const MOUSE_X: &str = "mouse-x";
const MOUSE_Y: &str = "mouse-y";
const WIDTH: &str = "width";
const HEIGHT: &str = "height";
const PLACEMENT: &str = "placement";
const OFFSET: &str = "offset";
const DELAY: &str = "delay";
const TEXT: &str = "text";

/// Report an error and replace any named reference that points to the tooltip element itself.
/// References to the tooltip's *children* are caught later by `lower_popups::check_no_reference_to_popup`
/// once the children have been moved into the generated PopupWindow component.
fn check_no_reference_to_tooltip(
    tooltip_element: &ElementRc,
    parent_element: &ElementRc,
    component: &Rc<Component>,
    diag: &mut BuildDiagnostics,
) {
    let dummy_ref = NamedReference::new(parent_element, SmolStr::new_static(WIDTH));

    recurse_elem_including_sub_components_no_borrow(component, &(), &mut |source_elem, _| {
        if Rc::ptr_eq(source_elem, tooltip_element) {
            return;
        }
        visit_all_named_references_in_element(source_elem, |nr| {
            if !Rc::ptr_eq(&nr.element(), tooltip_element) {
                return;
            }
            let id = tooltip_element.borrow().id.clone();
            let prop_name = nr.name();
            let what = if id.is_empty() {
                format!("property or callback '{prop_name}'")
            } else {
                format!("property or callback '{id}.{prop_name}'")
            };
            diag.push_error(
                format!("Cannot access {what} inside of a ToolTip from enclosing component"),
                &*tooltip_element.borrow(),
            );
            *nr = dummy_ref.clone();
        });
    });
}

fn build_tooltip_content(
    popup_id: &SmolStr,
    enclosing_component: &std::rc::Weak<Component>,
    tooltip_impl_type: &ElementType,
    tooltip_text: Option<NamedReference>,
    children: Vec<ElementRc>,
) -> ElementRc {
    let mut bindings = std::collections::BTreeMap::new();
    if let Some(tooltip_text) = tooltip_text {
        bindings.insert(
            SmolStr::new_static("text"),
            RefCell::new(Expression::PropertyReference(tooltip_text).into()),
        );
    }
    Element {
        id: format_smolstr!("{}-content", popup_id),
        base_type: tooltip_impl_type.clone(),
        enclosing_component: enclosing_component.clone(),
        bindings,
        children,
        ..Default::default()
    }
    .make_rc()
}

fn bind_popup_effective_size_from_content(
    popup_window_rc: &ElementRc,
    tooltip_content_rc: &ElementRc,
) {
    let content_has_width = tooltip_content_rc.borrow().bindings.contains_key(WIDTH);
    let content_has_height = tooltip_content_rc.borrow().bindings.contains_key(HEIGHT);

    if content_has_width {
        let explicit_width = NamedReference::new(tooltip_content_rc, SmolStr::new_static(WIDTH));
        let mut width_binding: BindingExpression =
            Expression::PropertyReference(explicit_width).into();
        width_binding.priority = 1;
        popup_window_rc
            .borrow_mut()
            .bindings
            .insert(SmolStr::new_static(WIDTH), RefCell::new(width_binding));
    } else {
        let preferred_width =
            NamedReference::new(tooltip_content_rc, SmolStr::new_static("preferred-width"));
        let mut width_binding: BindingExpression =
            Expression::PropertyReference(preferred_width).into();
        width_binding.priority = 1;
        popup_window_rc
            .borrow_mut()
            .bindings
            .insert(SmolStr::new_static(WIDTH), RefCell::new(width_binding));
    }
    if content_has_height {
        let explicit_height = NamedReference::new(tooltip_content_rc, SmolStr::new_static(HEIGHT));
        let mut height_binding: BindingExpression =
            Expression::PropertyReference(explicit_height).into();
        height_binding.priority = 1;
        popup_window_rc
            .borrow_mut()
            .bindings
            .insert(SmolStr::new_static(HEIGHT), RefCell::new(height_binding));
    } else {
        let preferred_height =
            NamedReference::new(tooltip_content_rc, SmolStr::new_static("preferred-height"));
        let mut height_binding: BindingExpression =
            Expression::PropertyReference(preferred_height).into();
        height_binding.priority = 1;
        popup_window_rc
            .borrow_mut()
            .bindings
            .insert(SmolStr::new_static(HEIGHT), RefCell::new(height_binding));
    }
}

fn build_tooltip_area(
    popup_id: &SmolStr,
    enclosing_component: &std::rc::Weak<Component>,
    tooltip_area_type: &ElementType,
) -> ElementRc {
    Element {
        id: format_smolstr!("{}-area", popup_id),
        base_type: tooltip_area_type.clone(),
        enclosing_component: enclosing_component.clone(),
        bindings: [
            (
                SmolStr::new_static("x"),
                RefCell::new(Expression::NumberLiteral(0., Unit::Percent).into()),
            ),
            (
                SmolStr::new_static("y"),
                RefCell::new(Expression::NumberLiteral(0., Unit::Percent).into()),
            ),
            (
                SmolStr::new_static(WIDTH),
                RefCell::new(Expression::NumberLiteral(100., Unit::Percent).into()),
            ),
            (
                SmolStr::new_static(HEIGHT),
                RefCell::new(Expression::NumberLiteral(100., Unit::Percent).into()),
            ),
        ]
        .into_iter()
        .collect(),
        ..Default::default()
    }
    .make_rc()
}

fn wire_tooltip_placement(
    popup_window_rc: &ElementRc,
    parent_width: NamedReference,
    parent_height: NamedReference,
    pointer_x: NamedReference,
    pointer_y: NamedReference,
    tooltip_offset: NamedReference,
    tooltip_placement: NamedReference,
    placement_enum: Rc<Enumeration>,
) {
    let popup_width = NamedReference::new(popup_window_rc, SmolStr::new_static("width"));
    let popup_height = NamedReference::new(popup_window_rc, SmolStr::new_static("height"));
    let popup_preferred_width =
        NamedReference::new(popup_window_rc, SmolStr::new_static("preferred-width"));
    let popup_preferred_height =
        NamedReference::new(popup_window_rc, SmolStr::new_static("preferred-height"));
    let placement_value = |name: &str| -> EnumerationValue {
        EnumerationValue {
            value: placement_enum
                .values
                .iter()
                .position(|v| v == name)
                .expect("ToolTipPlacement variant must exist"),
            enumeration: placement_enum.clone(),
        }
    };

    let is_pointer = Expression::BinaryExpression {
        lhs: Box::new(Expression::PropertyReference(tooltip_placement.clone())),
        rhs: Box::new(Expression::EnumerationValue(placement_value("pointer"))),
        op: '=',
    };
    let is_left = Expression::BinaryExpression {
        lhs: Box::new(Expression::PropertyReference(tooltip_placement.clone())),
        rhs: Box::new(Expression::EnumerationValue(placement_value("left-element"))),
        op: '=',
    };
    let is_right = Expression::BinaryExpression {
        lhs: Box::new(Expression::PropertyReference(tooltip_placement.clone())),
        rhs: Box::new(Expression::EnumerationValue(placement_value("right-element"))),
        op: '=',
    };
    let is_above = Expression::BinaryExpression {
        lhs: Box::new(Expression::PropertyReference(tooltip_placement.clone())),
        rhs: Box::new(Expression::EnumerationValue(placement_value("above-element"))),
        op: '=',
    };
    let is_below = Expression::BinaryExpression {
        lhs: Box::new(Expression::PropertyReference(tooltip_placement)),
        rhs: Box::new(Expression::EnumerationValue(placement_value("below-element"))),
        op: '=',
    };
    let effective_popup_width = Expression::Condition {
        condition: Box::new(Expression::BinaryExpression {
            lhs: Box::new(Expression::PropertyReference(popup_width.clone())),
            rhs: Box::new(Expression::NumberLiteral(0., Unit::None)),
            op: '>',
        }),
        true_expr: Box::new(Expression::PropertyReference(popup_width.clone())),
        false_expr: Box::new(Expression::PropertyReference(popup_preferred_width.clone())),
    };
    let effective_popup_height = Expression::Condition {
        condition: Box::new(Expression::BinaryExpression {
            lhs: Box::new(Expression::PropertyReference(popup_height.clone())),
            rhs: Box::new(Expression::NumberLiteral(0., Unit::None)),
            op: '>',
        }),
        true_expr: Box::new(Expression::PropertyReference(popup_height.clone())),
        false_expr: Box::new(Expression::PropertyReference(popup_preferred_height.clone())),
    };
    let centered_x = Expression::BinaryExpression {
        lhs: Box::new(Expression::BinaryExpression {
            lhs: Box::new(Expression::PropertyReference(parent_width.clone())),
            rhs: Box::new(effective_popup_width.clone()),
            op: '-',
        }),
        rhs: Box::new(Expression::NumberLiteral(2., Unit::None)),
        op: '/',
    };
    let centered_y = Expression::BinaryExpression {
        lhs: Box::new(Expression::BinaryExpression {
            lhs: Box::new(Expression::PropertyReference(parent_height.clone())),
            rhs: Box::new(effective_popup_height.clone()),
            op: '-',
        }),
        rhs: Box::new(Expression::NumberLiteral(2., Unit::None)),
        op: '/',
    };
    let tooltip_offset_expr = Expression::PropertyReference(tooltip_offset);
    let x_left = Expression::BinaryExpression {
        lhs: Box::new(Expression::UnaryOp { sub: Box::new(effective_popup_width), op: '-' }),
        rhs: Box::new(tooltip_offset_expr.clone()),
        op: '-',
    };
    let x_right = Expression::BinaryExpression {
        lhs: Box::new(Expression::PropertyReference(parent_width)),
        rhs: Box::new(tooltip_offset_expr.clone()),
        op: '+',
    };
    let y_above = Expression::BinaryExpression {
        lhs: Box::new(Expression::UnaryOp { sub: Box::new(effective_popup_height), op: '-' }),
        rhs: Box::new(tooltip_offset_expr.clone()),
        op: '-',
    };
    let y_below = Expression::BinaryExpression {
        lhs: Box::new(Expression::PropertyReference(parent_height)),
        rhs: Box::new(tooltip_offset_expr.clone()),
        op: '+',
    };
    let x_pointer = Expression::PropertyReference(pointer_x);
    let y_pointer = Expression::BinaryExpression {
        lhs: Box::new(Expression::PropertyReference(pointer_y)),
        rhs: Box::new(tooltip_offset_expr),
        op: '+',
    };

    let mut x_binding: BindingExpression = Expression::Condition {
        condition: Box::new(is_pointer.clone()),
        true_expr: Box::new(x_pointer),
        false_expr: Box::new(Expression::Condition {
            condition: Box::new(is_left),
            true_expr: Box::new(x_left),
            false_expr: Box::new(Expression::Condition {
                condition: Box::new(is_right),
                true_expr: Box::new(x_right),
                false_expr: Box::new(centered_x),
            }),
        }),
    }
    .into();
    x_binding.priority = 1;
    popup_window_rc.borrow_mut().bindings.insert(SmolStr::new_static("x"), RefCell::new(x_binding));

    let mut y_binding: BindingExpression = Expression::Condition {
        condition: Box::new(is_pointer.clone()),
        true_expr: Box::new(y_pointer),
        false_expr: Box::new(Expression::Condition {
            condition: Box::new(is_above),
            true_expr: Box::new(y_above),
            false_expr: Box::new(Expression::Condition {
                condition: Box::new(is_below),
                true_expr: Box::new(y_below),
                false_expr: Box::new(centered_y),
            }),
        }),
    }
    .into();
    y_binding.priority = 1;
    popup_window_rc.borrow_mut().bindings.insert(SmolStr::new_static("y"), RefCell::new(y_binding));
}

fn wire_tooltip_visibility_behavior(
    elem: &ElementRc,
    tooltip_child_index: usize,
    tooltip_area: &ElementRc,
    popup_window_rc: ElementRc,
) {
    let popup_weak = Rc::downgrade(&popup_window_rc);
    let show_popup = Expression::FunctionCall {
        function: BuiltinFunction::ShowPopupWindow.into(),
        arguments: vec![Expression::ElementReference(popup_weak.clone())],
        source_location: None,
    };
    let close_popup = Expression::FunctionCall {
        function: BuiltinFunction::ClosePopupWindow.into(),
        arguments: vec![Expression::ElementReference(popup_weak)],
        source_location: None,
    };

    tooltip_area.borrow_mut().bindings.insert(
        SmolStr::new_static("show"),
        RefCell::new(Expression::CodeBlock(vec![show_popup]).into()),
    );
    tooltip_area.borrow_mut().bindings.insert(
        SmolStr::new_static("hide"),
        RefCell::new(Expression::CodeBlock(vec![close_popup]).into()),
    );

    {
        let mut elem_borrow = elem.borrow_mut();
        elem_borrow.children.insert(tooltip_child_index, tooltip_area.clone());
        elem_borrow.children.insert(tooltip_child_index + 1, popup_window_rc);
        elem_borrow.has_popup_child = true;
    }
}

fn lower_tooltips_in_component(
    component: &Rc<Component>,
    type_register: &TypeRegister,
    tooltip_impl_type: &ElementType,
    diag: &mut BuildDiagnostics,
) {
    let tooltip_type = type_register.lookup_builtin_element(TOOLTIP_ELEMENT).unwrap();
    let tooltip_area_type = type_register.lookup_builtin_element(TOOLTIP_AREA_ELEMENT).unwrap();
    let popup_window_type = type_register.lookup_builtin_element(POPUP_WINDOW_ELEMENT).unwrap();

    let popup_close_policy_enum = BUILTIN.with(|e| e.enums.PopupClosePolicy.clone());
    let popup_close_policy_no_auto_close = EnumerationValue {
        value: popup_close_policy_enum.values.iter().position(|v| v == "no-auto-close").unwrap(),
        enumeration: popup_close_policy_enum,
    };

    let mut tooltip_popup_id_counter: u32 = 0;
    recurse_elem_including_sub_components_no_borrow(component, &(), &mut |elem, _| {
        // Traversal also visits generated children; skip tooltip popups created by this pass.
        let is_generated_tooltip_popup = {
            let elem_borrow = elem.borrow();
            matches!(&elem_borrow.base_type, t if *t == popup_window_type) && elem_borrow.is_tooltip
        };
        if is_generated_tooltip_popup {
            return;
        }

        let is_tooltip_like =
            matches!(&elem.borrow().builtin_type(), Some(b) if b.name == TOOLTIP_ELEMENT);
        let is_direct_tooltip = matches!(&elem.borrow().base_type, t if *t == tooltip_type);
        if is_tooltip_like && !is_direct_tooltip {
            diag.push_error("ToolTip cannot be inherited".into(), &*elem.borrow());
            return;
        }

        let tooltip_indices: Vec<usize> = elem
            .borrow()
            .children
            .iter()
            .enumerate()
            .filter_map(|(idx, child)| {
                matches!(&child.borrow().base_type, t if *t == tooltip_type).then_some(idx)
            })
            .collect();
        if tooltip_indices.is_empty() {
            return;
        }
        if tooltip_indices.len() > 1 {
            let children = elem.borrow().children.clone();
            for idx in tooltip_indices.iter().skip(1) {
                let child = &children[*idx];
                diag.push_error(
                    "Only one ToolTip is allowed as a child of an element".into(),
                    &*child.borrow(),
                );
            }
            return;
        }
        let tooltip_child_index = tooltip_indices[0];

        let tooltip_candidate = elem.borrow().children[tooltip_child_index].clone();
        if elem.borrow().builtin_type().is_some_and(|builtin| {
            LAYOUT_ELEMENTS_DISALLOWING_TOOLTIP.contains(&builtin.name.as_str())
        }) {
            diag.push_error(
                "ToolTip cannot be used inside layout elements".into(),
                &*tooltip_candidate.borrow(),
            );
            return;
        }
        if elem.borrow().builtin_type().is_some_and(|builtin| builtin.is_non_item_type) {
            diag.push_error(
                "ToolTip cannot be used inside non-item elements".into(),
                &*tooltip_candidate.borrow(),
            );
            return;
        }

        let has_custom_content = !tooltip_candidate.borrow().children.is_empty();
        let has_text_binding = tooltip_candidate.borrow().bindings.contains_key("text");
        if has_custom_content && has_text_binding {
            diag.push_error(
                "ToolTip cannot have both text and custom content".into(),
                &*tooltip_candidate.borrow(),
            );
            return;
        }
        if !has_custom_content && !has_text_binding {
            diag.push_error(
                "ToolTip must provide either text or custom content".into(),
                &*tooltip_candidate.borrow(),
            );
            return;
        }
        if has_custom_content && tooltip_candidate.borrow().children.len() > 1 {
            diag.push_error(
                "ToolTip custom content must have exactly one root child element".into(),
                &*tooltip_candidate.borrow(),
            );
            return;
        }

        check_no_reference_to_tooltip(&tooltip_candidate, elem, component, diag);

        let (tooltip_config, enclosing_component, popup_id, custom_children) = {
            let mut elem_borrow = elem.borrow_mut();
            let tooltip_config = elem_borrow.children.remove(tooltip_child_index);
            let custom_children = if has_custom_content {
                std::mem::take(&mut tooltip_config.borrow_mut().children)
            } else {
                Vec::new()
            };
            let enclosing_component = elem_borrow.enclosing_component.clone();
            let popup_id =
                format_smolstr!("{}{}", TOOLTIP_POPUP_ID_PREFIX, tooltip_popup_id_counter);
            tooltip_popup_id_counter += 1;
            (tooltip_config, enclosing_component, popup_id, custom_children)
        };

        let parent_width = NamedReference::new(elem, SmolStr::new_static(WIDTH));
        let parent_height = NamedReference::new(elem, SmolStr::new_static(HEIGHT));

        let tooltip_area = build_tooltip_area(&popup_id, &enclosing_component, &tooltip_area_type);
        let copied_binding = |source_property: &str, target_property: &str| {
            if let Some(binding) = tooltip_config.borrow().bindings.get(source_property) {
                tooltip_area
                    .borrow_mut()
                    .bindings
                    .insert(SmolStr::new(target_property), RefCell::new(binding.borrow().clone()));
            }
        };
        copied_binding(PLACEMENT, PLACEMENT);
        copied_binding(OFFSET, OFFSET);
        copied_binding(DELAY, DELAY);
        if has_text_binding {
            copied_binding(TEXT, TEXT);
        }

        let tooltip_placement = NamedReference::new(&tooltip_area, SmolStr::new_static(PLACEMENT));
        let tooltip_offset = NamedReference::new(&tooltip_area, SmolStr::new_static(OFFSET));
        let pointer_x = NamedReference::new(&tooltip_area, SmolStr::new_static(MOUSE_X));
        let pointer_y = NamedReference::new(&tooltip_area, SmolStr::new_static(MOUSE_Y));
        let tooltip_text = (!has_custom_content)
            .then(|| NamedReference::new(&tooltip_area, SmolStr::new_static(TEXT)));
        let tooltip_content = build_tooltip_content(
            &popup_id,
            &enclosing_component,
            tooltip_impl_type,
            tooltip_text,
            custom_children,
        );
        let popup_children = vec![tooltip_content.clone()];

        let placement_enum = match tooltip_area.borrow().lookup_property(PLACEMENT).property_type {
            Type::Enumeration(en) => en,
            _ => {
                diag.push_error(
                    "ToolTip.placement must be an enum value".into(),
                    &*tooltip_area.borrow(),
                );
                return;
            }
        };

        let popup_window = Element {
            id: popup_id,
            base_type: popup_window_type.clone(),
            enclosing_component: enclosing_component.clone(),
            is_tooltip: true,
            children: popup_children,
            bindings: [(
                SmolStr::new_static("close-policy"),
                RefCell::new(
                    Expression::EnumerationValue(popup_close_policy_no_auto_close.clone()).into(),
                ),
            )]
            .into_iter()
            .collect(),
            // Carry the ToolTip's source location so diagnostics from
            // lower_popups point back to the original ToolTip element.
            debug: tooltip_config.borrow().debug.clone(),
            ..Default::default()
        };
        let popup_window_rc = popup_window.make_rc();
        bind_popup_effective_size_from_content(&popup_window_rc, &tooltip_content);
        wire_tooltip_placement(
            &popup_window_rc,
            parent_width,
            parent_height,
            pointer_x,
            pointer_y,
            tooltip_offset,
            tooltip_placement,
            placement_enum,
        );

        wire_tooltip_visibility_behavior(elem, tooltip_child_index, &tooltip_area, popup_window_rc);
    });
}

pub async fn lower_tooltips(
    doc: &Document,
    type_loader: &mut crate::typeloader::TypeLoader,
    diag: &mut BuildDiagnostics,
) {
    let mut has_tooltip = false;
    doc.visit_all_used_components(|component| {
        recurse_elem_including_sub_components_no_borrow(component, &(), &mut |elem, _| {
            if matches!(&elem.borrow().builtin_type(), Some(b) if b.name == TOOLTIP_ELEMENT) {
                has_tooltip = true;
            }
        })
    });

    if !has_tooltip {
        return;
    }

    let mut import_diag = BuildDiagnostics::default();
    let tooltip_component = type_loader
        .import_component("std-widgets.slint", TOOLTIP_IMPL_ELEMENT, &mut import_diag)
        .await;
    for diagnostic in import_diag {
        diag.push_compiler_error(diagnostic);
    }
    let Some(tooltip_component) = tooltip_component else {
        let generic_location = doc.node.as_ref().map(|n| n.to_source_location());
        diag.push_error(
            "`ToolTip` style implementation could not be loaded from std-widgets".into(),
            &generic_location,
        );
        return;
    };
    let tooltip_style_type = ElementType::Component(tooltip_component);

    doc.visit_all_used_components(|component| {
        lower_tooltips_in_component(component, &doc.local_registry, &tooltip_style_type, diag);
    });
}
