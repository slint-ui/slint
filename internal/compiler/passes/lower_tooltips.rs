// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Lowers `Tooltip { text: ... }` to an input-transparent popup overlay.
//!
//! For each `Tooltip` child, this pass synthesizes a `PopupWindow` anchored at
//! the pointer position and contains the tooltip content.
//! Visibility is driven by runtime behavior in `TooltipArea`:
//! - hover enters: start/restart internal delay timer
//! - timer fires: invoke `show()` callback
//! - hover leaves: stop timer and invoke `hide()` callback
//!   `TooltipArea` also tracks the last known pointer position
//!   (`mouse-x`/`mouse-y`) for positioning the popup near the cursor.
//!
//! Runtime popup handling marks tooltip popups as input-transparent overlays.
//!
//! Tooltip content contract:
//! - A parent element may have **at most one** `Tooltip` child.
//! - `Tooltip` supports exactly one content mode:
//!   - text mode: `text` binding is present, no children
//!   - custom mode: children are present, no `text` binding
//! - custom mode expects one root child element which is used directly as tooltip content.
//!
//! Placement around `for`/`if`:
//! - `for ... : Tooltip { ... }` is rejected at compile time.
//! - `if cond : Tooltip { ... }` is allowed; the condition is forwarded onto the synthesized
//!   `TooltipArea` (which owns the generated `PopupWindow` as its child), so the area only
//!   exists while `cond` is true.

use crate::diagnostics::{BuildDiagnostics, Spanned};
use crate::expression_tree::{BindingExpression, BuiltinFunction, Expression, Unit};
use crate::langtype::{ElementType, EnumerationValue};
use crate::namedreference::NamedReference;
use crate::object_tree::*;
use crate::typeregister::{BUILTIN, TypeRegister};
use smol_str::{SmolStr, format_smolstr};
use std::cell::RefCell;
use std::rc::Rc;

const TOOLTIP_ELEMENT: &str = "Tooltip";
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
const OFFSET: &str = "offset";
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
                format!("Cannot access {what} inside of a Tooltip from enclosing component"),
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
    let content_has_width = tooltip_content_rc.borrow().binding(WIDTH).is_some();
    let content_has_height = tooltip_content_rc.borrow().binding(HEIGHT).is_some();

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
    repeated: Option<RepeatedElementInfo>,
) -> ElementRc {
    let mut elem = Element {
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
        repeated,
        ..Default::default()
    };
    // `Element::from_node` runs `apply_default_type_properties` on user-written elements;
    // synthesized elements need the same treatment so the builtin defaults (`delay`,
    // `offset`) declared on `TooltipArea` reach the runtime.
    crate::object_tree::apply_default_type_properties(&mut elem);
    elem.make_rc()
}

fn wire_tooltip_placement(
    popup_window_rc: &ElementRc,
    pointer_x: NamedReference,
    pointer_y: NamedReference,
    tooltip_offset: NamedReference,
) {
    let tooltip_offset_expr = Expression::PropertyReference(tooltip_offset);
    let x_pointer = Expression::PropertyReference(pointer_x);
    let y_pointer = Expression::BinaryExpression {
        lhs: Box::new(Expression::PropertyReference(pointer_y)),
        rhs: Box::new(tooltip_offset_expr),
        op: '+',
    };

    let mut x_binding: BindingExpression = x_pointer.into();
    x_binding.priority = 1;
    popup_window_rc.borrow_mut().bindings.insert(SmolStr::new_static("x"), RefCell::new(x_binding));

    let mut y_binding: BindingExpression = y_pointer.into();
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

    // Make the PopupWindow a child of the TooltipArea so that the popup's bindings
    // (`x`/`y` referring to `TooltipArea.mouse-x`/`mouse-y`) and the conditional
    // gating (`repeated` on `TooltipArea`) stay in the same scope.
    tooltip_area.borrow_mut().children.push(popup_window_rc);
    elem.borrow_mut().children.insert(tooltip_child_index, tooltip_area.clone());
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
            diag.push_error("Tooltip cannot be inherited".into(), &*elem.borrow());
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
                    "Only one Tooltip is allowed as a child of an element".into(),
                    &*child.borrow(),
                );
            }
            return;
        }
        let tooltip_child_index = tooltip_indices[0];

        let tooltip_candidate = elem.borrow().children[tooltip_child_index].clone();
        // `if cond : Tooltip { ... }` is allowed (the wrapper switches the synthesized
        // TooltipArea + PopupWindow on/off with the condition); `for ... : Tooltip { ... }`
        // is not.
        let tooltip_repeated = tooltip_candidate.borrow_mut().repeated.take();
        if tooltip_repeated.as_ref().is_some_and(|r| !r.is_conditional_element) {
            diag.push_error(
                "Tooltip cannot be in a `for` element".into(),
                &*tooltip_candidate.borrow(),
            );
            return;
        }
        let parent_name = elem.borrow().builtin_type().map(|b| b.name.clone());
        if parent_name
            .as_ref()
            .is_some_and(|name| LAYOUT_ELEMENTS_DISALLOWING_TOOLTIP.contains(&name.as_str()))
        {
            diag.push_error(
                format!("Tooltip cannot be added to {}", parent_name.as_ref().unwrap()),
                &*tooltip_candidate.borrow(),
            );
            return;
        }
        if elem.borrow().builtin_type().is_some_and(|builtin| {
            builtin.is_non_item_type || builtin.disallow_global_types_as_child_elements
        }) {
            diag.push_error(
                format!("Tooltip cannot be added to {}", parent_name.as_ref().unwrap()),
                &*tooltip_candidate.borrow(),
            );
            return;
        }

        let has_custom_content = !tooltip_candidate.borrow().children.is_empty();
        let has_text_binding = tooltip_candidate.borrow().binding("text").is_some();
        if has_custom_content && has_text_binding {
            diag.push_error(
                "Tooltip cannot have both text and custom content".into(),
                &*tooltip_candidate.borrow(),
            );
            return;
        }
        if !has_custom_content && !has_text_binding {
            diag.push_error(
                "Tooltip must provide either text or custom content".into(),
                &*tooltip_candidate.borrow(),
            );
            return;
        }
        if has_custom_content && tooltip_candidate.borrow().children.len() > 1 {
            diag.push_error(
                "Tooltip custom content must have exactly one root child element".into(),
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

        let tooltip_area = build_tooltip_area(
            &popup_id,
            &enclosing_component,
            &tooltip_area_type,
            tooltip_repeated,
        );
        // Propagate a user-set binding from the `Tooltip` element onto the synthesized
        // `TooltipArea`, keyed by the same property name. Currently only `text` is shared,
        // but the helper is kept so adding more shared properties later is a one-liner.
        let copy_binding = |property: &str| {
            if let Some(binding) = tooltip_config.borrow().bindings.get(property) {
                tooltip_area
                    .borrow_mut()
                    .bindings
                    .insert(SmolStr::new(property), RefCell::new(binding.borrow().clone()));
            }
        };
        if has_text_binding {
            copy_binding(TEXT);
        }

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
            // Carry the Tooltip's source location so diagnostics from
            // lower_popups point back to the original Tooltip element.
            debug: tooltip_config.borrow().debug.clone(),
            ..Default::default()
        };
        let popup_window_rc = popup_window.make_rc();
        bind_popup_effective_size_from_content(&popup_window_rc, &tooltip_content);
        wire_tooltip_placement(&popup_window_rc, pointer_x, pointer_y, tooltip_offset);

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
        .import_component("std-widgets-impl.slint", TOOLTIP_IMPL_ELEMENT, &mut import_diag)
        .await;
    for diagnostic in import_diag {
        diag.push_compiler_error(diagnostic);
    }
    let Some(tooltip_component) = tooltip_component else {
        let generic_location = doc.node.as_ref().map(|n| n.to_source_location());
        diag.push_error(
            "`Tooltip` style implementation could not be loaded from std-widgets".into(),
            &generic_location,
        );
        return;
    };
    let tooltip_style_type = ElementType::Component(tooltip_component);

    doc.visit_all_used_components(|component| {
        lower_tooltips_in_component(component, &doc.local_registry, &tooltip_style_type, diag);
    });
}
