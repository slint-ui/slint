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

use crate::diagnostics::BuildDiagnostics;
use crate::expression_tree::{BindingExpression, BuiltinFunction, Expression, Unit};
use crate::langtype::{ElementType, Enumeration, EnumerationValue, Type};
use crate::namedreference::NamedReference;
use crate::object_tree::*;
use crate::typeregister::{TypeRegister, BUILTIN};
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

fn build_tooltip_background(
    popup_id: &SmolStr,
    tooltip_text: NamedReference,
    enclosing_component: &std::rc::Weak<Component>,
    text_type: &ElementType,
    vertical_layout_type: &ElementType,
    rectangle_type: &ElementType,
    palette: &Rc<Component>,
    style_metrics: &Rc<Component>,
) -> ElementRc {
    let text_element = Element {
        id: format_smolstr!("{}-text", popup_id),
        base_type: text_type.clone(),
        enclosing_component: enclosing_component.clone(),
        bindings: [(
            SmolStr::new_static("text"),
            RefCell::new(Expression::PropertyReference(tooltip_text).into()),
        )]
        .into_iter()
        .collect(),
        ..Default::default()
    };
    let text_element_rc = text_element.make_rc();

    let padded_text = Element {
        id: format_smolstr!("{}-padded", popup_id),
        base_type: vertical_layout_type.clone(),
        enclosing_component: enclosing_component.clone(),
        children: vec![text_element_rc],
        bindings: [(
            SmolStr::new_static("padding"),
            RefCell::new(
                Expression::PropertyReference(NamedReference::new(
                    &style_metrics.root_element,
                    SmolStr::new_static("layout-padding"),
                ))
                .into(),
            ),
        )]
        .into_iter()
        .collect(),
        ..Default::default()
    };
    let padded_text_rc = padded_text.make_rc();

    let background_rect = Element {
        id: format_smolstr!("{}-bg", popup_id),
        base_type: rectangle_type.clone(),
        enclosing_component: enclosing_component.clone(),
        children: vec![padded_text_rc],
        bindings: [
            (
                SmolStr::new_static("background"),
                RefCell::new(
                    Expression::Cast {
                        from: Expression::PropertyReference(NamedReference::new(
                            &palette.root_element,
                            SmolStr::new_static("alternate-background"),
                        ))
                        .into(),
                        to: Type::Brush,
                    }
                    .into(),
                ),
            ),
            (
                SmolStr::new_static("border-radius"),
                RefCell::new(
                    Expression::PropertyReference(NamedReference::new(
                        &style_metrics.root_element,
                        SmolStr::new_static("layout-padding"),
                    ))
                    .into(),
                ),
            ),
            (
                SmolStr::new_static("border-width"),
                RefCell::new(Expression::NumberLiteral(1., Unit::Px).into()),
            ),
            (
                SmolStr::new_static("border-color"),
                RefCell::new(
                    Expression::Cast {
                        from: Expression::PropertyReference(NamedReference::new(
                            &palette.root_element,
                            SmolStr::new_static("border"),
                        ))
                        .into(),
                        to: Type::Brush,
                    }
                    .into(),
                ),
            ),
        ]
        .into_iter()
        .collect(),
        ..Default::default()
    };
    background_rect.make_rc()
}

fn build_tooltip_delay_timer(
    popup_id: &SmolStr,
    enclosing_component: &std::rc::Weak<Component>,
    timer_type: &ElementType,
) -> ElementRc {
    let mut timer_interval: BindingExpression = Expression::NumberLiteral(TOOLTIP_DELAY_MS, Unit::Ms).into();
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

fn wire_tooltip_placement(
    popup_window_rc: &ElementRc,
    parent_width: NamedReference,
    parent_height: NamedReference,
    tooltip_placement: NamedReference,
    placement_enum: Rc<Enumeration>,
) {
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
    let centered_x = Expression::BinaryExpression {
        lhs: Box::new(Expression::BinaryExpression {
            lhs: Box::new(Expression::PropertyReference(parent_width.clone())),
            rhs: Box::new(Expression::PropertyReference(popup_preferred_width.clone())),
            op: '-',
        }),
        rhs: Box::new(Expression::NumberLiteral(2., Unit::None)),
        op: '/',
    };
    let centered_y = Expression::BinaryExpression {
        lhs: Box::new(Expression::BinaryExpression {
            lhs: Box::new(Expression::PropertyReference(parent_height.clone())),
            rhs: Box::new(Expression::PropertyReference(popup_preferred_height.clone())),
            op: '-',
        }),
        rhs: Box::new(Expression::NumberLiteral(2., Unit::None)),
        op: '/',
    };
    let x_left = Expression::BinaryExpression {
        lhs: Box::new(Expression::UnaryOp {
            sub: Box::new(Expression::PropertyReference(popup_preferred_width)),
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
            sub: Box::new(Expression::PropertyReference(popup_preferred_height)),
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
    popup_window_rc
        .borrow_mut()
        .bindings
        .insert(SmolStr::new_static("x"), RefCell::new(x_binding));

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
    popup_window_rc
        .borrow_mut()
        .bindings
        .insert(SmolStr::new_static("y"), RefCell::new(y_binding));
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

pub fn lower_tooltips(
    component: &Rc<Component>,
    type_register: &TypeRegister,
    palette: &Rc<Component>,
    style_metrics: &Rc<Component>,
    diag: &mut BuildDiagnostics,
) {
    let tooltip_type = type_register.lookup_builtin_element(TOOLTIP_ELEMENT).unwrap();
    let tooltip_area_type = type_register.lookup_builtin_element(TOOLTIP_AREA_ELEMENT).unwrap();
    let popup_window_type = type_register.lookup_builtin_element(POPUP_WINDOW_ELEMENT).unwrap();
    let timer_type = type_register.lookup_builtin_element("Timer").unwrap();
    let text_type = type_register.lookup_builtin_element("Text").unwrap();
    let rectangle_type = type_register.lookup_builtin_element("Rectangle").unwrap();
    let vertical_layout_type = type_register.lookup_builtin_element("VerticalLayout").unwrap();

    let popup_close_policy_enum = BUILTIN.with(|e| e.enums.PopupClosePolicy.clone());
    let popup_close_policy_no_auto_close = EnumerationValue {
        value: popup_close_policy_enum
            .values
            .iter()
            .position(|v| v == "no-auto-close")
            .unwrap(),
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

        let tooltip_child_index = elem.borrow().children.iter().position(|child| {
            matches!(&child.borrow().base_type, t if *t == tooltip_type)
        });
        let Some(tooltip_child_index) = tooltip_child_index else {
            return;
        };

        let (tooltip_config, enclosing_component, popup_id, popup_id_for_text) = {
            let mut elem_borrow = elem.borrow_mut();
            let tooltip_config = elem_borrow.children.remove(tooltip_child_index);
            let enclosing_component = elem_borrow.enclosing_component.clone();
            let popup_id = format_smolstr!(
                "{}{}",
                TOOLTIP_POPUP_ID_PREFIX,
                tooltip_popup_id_counter
            );
            tooltip_popup_id_counter += 1;
            let popup_id_for_text = popup_id.clone();
            (tooltip_config, enclosing_component, popup_id, popup_id_for_text)
        };

        let parent_width = NamedReference::new(elem, SmolStr::new_static(WIDTH));
        let parent_height = NamedReference::new(elem, SmolStr::new_static(HEIGHT));

        let tooltip_text = NamedReference::new(&tooltip_config, SmolStr::new_static("text"));
        let tooltip_placement = NamedReference::new(&tooltip_config, SmolStr::new_static(PLACEMENT));
        let tooltip_area = Element {
            id: format_smolstr!("{}-area", popup_id_for_text),
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
        .make_rc();
        let background_rect_rc = build_tooltip_background(
            &popup_id_for_text,
            tooltip_text,
            &enclosing_component,
            &text_type,
            &vertical_layout_type,
            &rectangle_type,
            palette,
            style_metrics,
        );

        let placement_enum = match tooltip_config.borrow().lookup_property(PLACEMENT).property_type {
            Type::Enumeration(en) => en,
            _ => {
                diag.push_error("ToolTip.placement must be an enum value".into(), &*tooltip_config.borrow());
                return;
            }
        };

        let popup_window = Element {
            id: popup_id,
            base_type: popup_window_type.clone(),
            enclosing_component: enclosing_component.clone(),
            popup_window_kind: Some(PopupWindowKind::Tooltip),
            children: vec![tooltip_config, background_rect_rc],
            bindings: [
                (
                    SmolStr::new_static("close-policy"),
                    RefCell::new(
                        Expression::EnumerationValue(popup_close_policy_no_auto_close.clone()).into(),
                    ),
                ),
            ]
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
