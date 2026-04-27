// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Lowers `ToolTip { text: ... }` to an input-transparent popup overlay.
//!
//! For each `ToolTip` child, this pass synthesizes a `PopupWindow` anchored around
//! the hovered parent element and contains the tooltip content.
//! The `ToolTip.placement` enum controls whether it appears at top/bottom/left/right.
//! Visibility is driven by an injected `TooltipArea` item's `has-hover` callback:
//! - hover enters: start/restart a delay timer
//! - timer fires: `ShowPopupWindow`
//! - hover leaves: stop timer and `ClosePopupWindow`
//!
//! Runtime popup handling marks tooltip popups as input-transparent overlays.
//! Tooltip show/hide delay currently uses a fixed delay constant.
//!
//! Tooltip content contract:
//! - `ToolTip` supports exactly one content mode:
//!   - text mode: `text` binding is present, no children
//!   - custom mode: children are present, no `text` binding
//! - placement uses effective popup size:
//!   - explicit `width`/`height` if set (> 0)
//!   - otherwise `preferred-width`/`preferred-height`
//! - custom mode wraps children in a layout-aware container so preferred size
//!   propagates predictably into placement calculations.

use crate::diagnostics::BuildDiagnostics;
use crate::expression_tree::{BindingExpression, BuiltinFunction, Expression, Unit};
use crate::langtype::{ElementType, Enumeration, EnumerationValue, Type};
use crate::namedreference::NamedReference;
use crate::object_tree::*;
use crate::typeregister::{BUILTIN, TypeRegister};
use smol_str::{SmolStr, format_smolstr};
use std::cell::RefCell;
use std::rc::Rc;

const TOOLTIP_ELEMENT: &str = "ToolTip";
const TOOLTIP_AREA_ELEMENT: &str = "TooltipArea";
const POPUP_WINDOW_ELEMENT: &str = "PopupWindow";
const TOOLTIP_POPUP_ID_PREFIX: &str = "tooltip-popup-overlay-";
const TOOLTIP_DELAY_MS: f64 = 500.;
const TOOLTIP_GAP_PX: f64 = 8.;

const HAS_HOVER: &str = "has-hover";
const WIDTH: &str = "width";
const HEIGHT: &str = "height";
const PLACEMENT: &str = "placement";

fn build_tooltip_visual(
    popup_id: &SmolStr,
    enclosing_component: &std::rc::Weak<Component>,
    tooltip_impl_type: &ElementType,
    tooltip_text: Option<NamedReference>,
    children: Vec<ElementRc>,
) -> ElementRc {
    let bindings = tooltip_text
        .map(|tooltip_text| {
            [(
                SmolStr::new_static("text"),
                RefCell::new(Expression::PropertyReference(tooltip_text).into()),
            )]
            .into_iter()
            .collect()
        })
        .unwrap_or_default();
    Element {
        id: format_smolstr!("{}-visual", popup_id),
        base_type: tooltip_impl_type.clone(),
        enclosing_component: enclosing_component.clone(),
        bindings,
        children,
        ..Default::default()
    }
    .make_rc()
}

fn build_custom_tooltip_content(
    popup_id: &SmolStr,
    enclosing_component: &std::rc::Weak<Component>,
    vertical_layout_type: &ElementType,
    children: Vec<ElementRc>,
) -> ElementRc {
    Element {
        id: format_smolstr!("{}-custom", popup_id),
        base_type: vertical_layout_type.clone(),
        enclosing_component: enclosing_component.clone(),
        children,
        ..Default::default()
    }
    .make_rc()
}

fn build_tooltip_delay_timer(
    popup_id: &SmolStr,
    enclosing_component: &std::rc::Weak<Component>,
    timer_type: &ElementType,
) -> ElementRc {
    let mut timer_interval: BindingExpression =
        Expression::NumberLiteral(TOOLTIP_DELAY_MS, Unit::Ms).into();
    timer_interval.priority = 1;
    Element {
        id: format_smolstr!("{}-delay", popup_id),
        base_type: timer_type.clone(),
        enclosing_component: enclosing_component.clone(),
        bindings: [(SmolStr::new_static("interval"), RefCell::new(timer_interval))]
            .into_iter()
            .collect(),
        ..Default::default()
    }
    .make_rc()
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

    let is_left = Expression::BinaryExpression {
        lhs: Box::new(Expression::PropertyReference(tooltip_placement.clone())),
        rhs: Box::new(Expression::EnumerationValue(placement_value("left"))),
        op: '=',
    };
    let is_right = Expression::BinaryExpression {
        lhs: Box::new(Expression::PropertyReference(tooltip_placement.clone())),
        rhs: Box::new(Expression::EnumerationValue(placement_value("right"))),
        op: '=',
    };
    let is_top = Expression::BinaryExpression {
        lhs: Box::new(Expression::PropertyReference(tooltip_placement.clone())),
        rhs: Box::new(Expression::EnumerationValue(placement_value("top"))),
        op: '=',
    };
    let is_bottom = Expression::BinaryExpression {
        lhs: Box::new(Expression::PropertyReference(tooltip_placement)),
        rhs: Box::new(Expression::EnumerationValue(placement_value("bottom"))),
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
    let x_left = Expression::BinaryExpression {
        lhs: Box::new(Expression::UnaryOp {
            sub: Box::new(effective_popup_width),
            op: '-',
        }),
        rhs: Box::new(Expression::NumberLiteral(TOOLTIP_GAP_PX, Unit::Px)),
        op: '-',
    };
    let x_right = Expression::BinaryExpression {
        lhs: Box::new(Expression::PropertyReference(parent_width)),
        rhs: Box::new(Expression::NumberLiteral(TOOLTIP_GAP_PX, Unit::Px)),
        op: '+',
    };
    let y_top = Expression::BinaryExpression {
        lhs: Box::new(Expression::UnaryOp {
            sub: Box::new(effective_popup_height),
            op: '-',
        }),
        rhs: Box::new(Expression::NumberLiteral(TOOLTIP_GAP_PX, Unit::Px)),
        op: '-',
    };
    let y_bottom = Expression::BinaryExpression {
        lhs: Box::new(Expression::PropertyReference(parent_height)),
        rhs: Box::new(Expression::NumberLiteral(TOOLTIP_GAP_PX, Unit::Px)),
        op: '+',
    };

    let mut x_binding: BindingExpression = Expression::Condition {
        condition: Box::new(is_left),
        true_expr: Box::new(x_left),
        false_expr: Box::new(Expression::Condition {
            condition: Box::new(is_right),
            true_expr: Box::new(x_right),
            false_expr: Box::new(centered_x),
        }),
    }
    .into();
    x_binding.priority = 1;
    popup_window_rc.borrow_mut().bindings.insert(SmolStr::new_static("x"), RefCell::new(x_binding));

    let mut y_binding: BindingExpression = Expression::Condition {
        condition: Box::new(is_top),
        true_expr: Box::new(y_top),
        false_expr: Box::new(Expression::Condition {
            condition: Box::new(is_bottom),
            true_expr: Box::new(y_bottom),
            false_expr: Box::new(centered_y),
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
    timer_element_rc: ElementRc,
) {
    let has_hover_nr = NamedReference::new(tooltip_area, SmolStr::new_static(HAS_HOVER));
    let popup_weak = Rc::downgrade(&popup_window_rc);
    let timer_running = NamedReference::new(&timer_element_rc, SmolStr::new_static("running"));
    let timer_weak = Rc::downgrade(&timer_element_rc);

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
    let restart_timer = Expression::FunctionCall {
        function: BuiltinFunction::RestartTimer.into(),
        arguments: vec![Expression::ElementReference(timer_weak)],
        source_location: None,
    };
    let set_running_true = Expression::SelfAssignment {
        lhs: Box::new(Expression::PropertyReference(timer_running.clone())),
        rhs: Box::new(Expression::BoolLiteral(true)),
        op: '=',
        node: None,
    };
    let set_running_false = Expression::SelfAssignment {
        lhs: Box::new(Expression::PropertyReference(timer_running.clone())),
        rhs: Box::new(Expression::BoolLiteral(false)),
        op: '=',
        node: None,
    };

    let callback = Expression::Condition {
        condition: Box::new(Expression::PropertyReference(has_hover_nr)),
        true_expr: Box::new(Expression::CodeBlock(vec![set_running_true, restart_timer])),
        false_expr: Box::new(Expression::CodeBlock(vec![set_running_false.clone(), close_popup])),
    };
    let timer_triggered = Expression::CodeBlock(vec![show_popup, set_running_false]);

    tooltip_area
        .borrow_mut()
        .change_callbacks
        .entry(SmolStr::new_static(HAS_HOVER))
        .or_default()
        .borrow_mut()
        .push(callback);

    {
        let mut elem_borrow = elem.borrow_mut();
        elem_borrow.children.insert(tooltip_child_index, tooltip_area.clone());
        elem_borrow.children.insert(tooltip_child_index + 1, timer_element_rc.clone());
        elem_borrow.children.insert(tooltip_child_index + 2, popup_window_rc);
        elem_borrow.has_popup_child = true;
    }
    let mut timer_triggered_binding: BindingExpression = timer_triggered.into();
    timer_triggered_binding.priority = 1;
    timer_element_rc
        .borrow_mut()
        .bindings
        .insert(SmolStr::new_static("triggered"), RefCell::new(timer_triggered_binding));
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
    let timer_type = type_register.lookup_builtin_element("Timer").unwrap();
    let vertical_layout_type = type_register.lookup_builtin_element("VerticalLayout").unwrap();

    let popup_close_policy_enum = BUILTIN.with(|e| e.enums.PopupClosePolicy.clone());
    let popup_close_policy_no_auto_close = EnumerationValue {
        value: popup_close_policy_enum.values.iter().position(|v| v == "no-auto-close").unwrap(),
        enumeration: popup_close_policy_enum,
    };

    let mut tooltip_popup_id_counter: u32 = 0;
    recurse_elem_including_sub_components_no_borrow(component, &(), &mut |elem, _| {
        // Recurse-with-subcomponents traversal also visits generated children. Skip tooltip popup
        // overlays produced by this pass, otherwise we'd try to lower our own output again.
        let is_generated_tooltip_popup = {
            let elem_borrow = elem.borrow();
            matches!(&elem_borrow.base_type, t if *t == popup_window_type)
                && matches!(elem_borrow.popup_window_kind, Some(PopupWindowKind::Tooltip))
        };
        if is_generated_tooltip_popup {
            return;
        }

        let tooltip_child_index = elem
            .borrow()
            .children
            .iter()
            .position(|child| matches!(&child.borrow().base_type, t if *t == tooltip_type));
        let Some(tooltip_child_index) = tooltip_child_index else {
            return;
        };

        let tooltip_candidate = elem.borrow().children[tooltip_child_index].clone();
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

        let (tooltip_config, enclosing_component, popup_id, popup_id_for_text, custom_children) = {
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
            let popup_id_for_text = popup_id.clone();
            (tooltip_config, enclosing_component, popup_id, popup_id_for_text, custom_children)
        };

        let parent_width = NamedReference::new(elem, SmolStr::new_static(WIDTH));
        let parent_height = NamedReference::new(elem, SmolStr::new_static(HEIGHT));

        let tooltip_placement =
            NamedReference::new(&tooltip_config, SmolStr::new_static(PLACEMENT));
        let tooltip_area =
            build_tooltip_area(&popup_id_for_text, &enclosing_component, &tooltip_area_type);
        let tooltip_visual = if has_custom_content {
            build_custom_tooltip_content(
                &popup_id_for_text,
                &enclosing_component,
                &vertical_layout_type,
                custom_children,
            )
        } else {
            let tooltip_text = NamedReference::new(&tooltip_config, SmolStr::new_static("text"));
            build_tooltip_visual(
                &popup_id_for_text,
                &enclosing_component,
                tooltip_impl_type,
                Some(tooltip_text),
                Vec::new(),
            )
        };
        let popup_children = vec![tooltip_config.clone(), tooltip_visual];

        let placement_enum = match tooltip_config.borrow().lookup_property(PLACEMENT).property_type
        {
            Type::Enumeration(en) => en,
            _ => {
                diag.push_error(
                    "ToolTip.placement must be an enum value".into(),
                    &*tooltip_config.borrow(),
                );
                return;
            }
        };

        let popup_window = Element {
            id: popup_id,
            base_type: popup_window_type.clone(),
            enclosing_component: enclosing_component.clone(),
            popup_window_kind: Some(PopupWindowKind::Tooltip),
            children: popup_children,
            bindings: [(
                SmolStr::new_static("close-policy"),
                RefCell::new(
                    Expression::EnumerationValue(popup_close_policy_no_auto_close.clone()).into(),
                ),
            )]
            .into_iter()
            .collect(),
            ..Default::default()
        };
        let popup_window_rc = popup_window.make_rc();
        wire_tooltip_placement(
            &popup_window_rc,
            parent_width,
            parent_height,
            tooltip_placement,
            placement_enum,
        );

        let timer_element_rc =
            build_tooltip_delay_timer(&popup_id_for_text, &enclosing_component, &timer_type);
        wire_tooltip_visibility_behavior(
            elem,
            tooltip_child_index,
            &tooltip_area,
            popup_window_rc,
            timer_element_rc,
        );
    });
}

pub async fn lower_tooltips(
    doc: &Document,
    type_loader: &mut crate::typeloader::TypeLoader,
    diag: &mut BuildDiagnostics,
) {
    // First check if any ToolTip is used - avoid loading std-widgets.slint if not needed.
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

    // Ignore import errors.
    let mut build_diags_to_ignore = BuildDiagnostics::default();
    let tooltip_component = type_loader
        .import_component("std-widgets.slint", TOOLTIP_ELEMENT, &mut build_diags_to_ignore)
        .await
        .expect("can't load ToolTip from std-widgets.slint");
    let tooltip_style_type = ElementType::Component(tooltip_component);

    doc.visit_all_used_components(|component| {
        lower_tooltips_in_component(component, &doc.local_registry, &tooltip_style_type, diag);
    });
}
