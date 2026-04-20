// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Lowers `ToolTip { text: ... }` to an input-transparent popup overlay.
//!
//! The tooltip is shown when the parent gets hovered and closed when the hover ends.
//! This pass uses `PopupWindow` for the overlay and relies on runtime support
//! to ensure tooltip popups do not take over pointer input dispatch.

use crate::expression_tree::{BuiltinFunction, Expression, Unit};
use crate::langtype::{EnumerationValue, Type};
use crate::namedreference::NamedReference;
use crate::object_tree::*;
use crate::typeregister::{TypeRegister, BUILTIN};
use smol_str::{SmolStr, format_smolstr};
use std::cell::RefCell;
use std::rc::Rc;

const TOOLTIP_ELEMENT: &str = "ToolTip";
const POPUP_WINDOW_ELEMENT: &str = "PopupWindow";
const TOOLTIP_POPUP_ID_PREFIX: &str = "tooltip-popup-overlay-";

const HAS_HOVER: &str = "has-hover";
const MOUSE_X: &str = "mouse-x";
const MOUSE_Y: &str = "mouse-y";

pub fn lower_tooltips(
    component: &Rc<Component>,
    type_register: &TypeRegister,
    palette: &Rc<Component>,
    style_metrics: &Rc<Component>,
) {
    let tooltip_type = type_register.lookup_builtin_element(TOOLTIP_ELEMENT).unwrap();
    let popup_window_type = type_register.lookup_builtin_element(POPUP_WINDOW_ELEMENT).unwrap();
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
        if elem.borrow().id.starts_with(TOOLTIP_POPUP_ID_PREFIX) {
            return;
        }

        let supports_hover = {
            let elem_borrow = elem.borrow();
            !matches!(elem_borrow.lookup_property(HAS_HOVER).property_type, Type::Invalid)
                && !matches!(elem_borrow.lookup_property(MOUSE_X).property_type, Type::Invalid)
                && !matches!(elem_borrow.lookup_property(MOUSE_Y).property_type, Type::Invalid)
        };
        if !supports_hover {
            return;
        }

        let tooltip_child_index = elem.borrow().children.iter().position(|child| {
            matches!(&child.borrow().base_type, t if *t == tooltip_type)
        });
        let Some(tooltip_child_index) = tooltip_child_index else {
            return;
        };

        let (tooltip_child, enclosing_component, popup_id, popup_id_for_text) = {
            let mut elem_borrow = elem.borrow_mut();
            let tooltip_child = elem_borrow.children.remove(tooltip_child_index);
            let enclosing_component = elem_borrow.enclosing_component.clone();
            let popup_id = format_smolstr!(
                "{}{}",
                TOOLTIP_POPUP_ID_PREFIX,
                tooltip_popup_id_counter
            );
            tooltip_popup_id_counter += 1;
            let popup_id_for_text = popup_id.clone();
            (tooltip_child, enclosing_component, popup_id, popup_id_for_text)
        };

        let mouse_x = NamedReference::new(elem, SmolStr::new_static(MOUSE_X));
        let mouse_y = NamedReference::new(elem, SmolStr::new_static(MOUSE_Y));

        let tooltip_text = NamedReference::new(&tooltip_child, SmolStr::new_static("text"));
        let text_element = Element {
            id: format_smolstr!("{}-text", popup_id_for_text),
            base_type: text_type.clone(),
            enclosing_component: enclosing_component.clone(),
            bindings: [
                (
                    SmolStr::new_static("text"),
                    RefCell::new(Expression::PropertyReference(tooltip_text).into()),
                ),
            ]
            .into_iter()
            .collect(),
            ..Default::default()
        };
        let text_element_rc = text_element.make_rc();

        let padded_text = Element {
            id: format_smolstr!("{}-padded", popup_id_for_text),
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
            id: format_smolstr!("{}-bg", popup_id_for_text),
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
        let background_rect_rc = background_rect.make_rc();

        let popup_window = Element {
            id: popup_id,
            base_type: popup_window_type.clone(),
            enclosing_component,
            popup_window_kind: PopupWindowKind::Tooltip,
            children: vec![tooltip_child, background_rect_rc],
            bindings: [
                (
                    SmolStr::new_static("x"),
                    RefCell::new(Expression::PropertyReference(mouse_x).into()),
                ),
                (
                    SmolStr::new_static("y"),
                    RefCell::new(Expression::PropertyReference(mouse_y).into()),
                ),
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

        let has_hover_nr = NamedReference::new(elem, SmolStr::new_static(HAS_HOVER));
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

        let callback = Expression::Condition {
            condition: Box::new(Expression::PropertyReference(has_hover_nr)),
            true_expr: Box::new(Expression::CodeBlock(vec![show_popup])),
            false_expr: Box::new(Expression::CodeBlock(vec![close_popup])),
        };

        {
            let mut elem_borrow = elem.borrow_mut();
            elem_borrow
                .change_callbacks
                .entry(SmolStr::new_static(HAS_HOVER))
                .or_default()
                .borrow_mut()
                .push(callback);

            elem_borrow.children.insert(tooltip_child_index, popup_window_rc);
            elem_borrow.has_popup_child = true;
        }
    });
}
