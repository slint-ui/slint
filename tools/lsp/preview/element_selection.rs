// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

use std::{path::PathBuf, rc::Rc};

use i_slint_compiler::{diagnostics::SourceFile, object_tree::ElementRc};
use i_slint_core::lengths::{LogicalLength, LogicalPoint};
use rowan::TextRange;
use slint_interpreter::{highlight::ComponentPositions, ComponentInstance};

// Look at an element and if it is a sub component, jump to its root_element()
fn self_or_embedded_component_root(element: &ElementRc) -> ElementRc {
    let elem = element.borrow();
    if elem.repeated.is_some() {
        if let i_slint_compiler::langtype::ElementType::Component(base) = &elem.base_type {
            return base.root_element.clone();
        }
    }
    element.clone()
}

fn lsp_element_position(element: &ElementRc) -> (String, lsp_types::Range) {
    let e = &element.borrow();
    e.node
        .as_ref()
        .and_then(|n| {
            n.parent()
                .filter(|p| p.kind() == i_slint_compiler::parser::SyntaxKind::SubElement)
                .map_or_else(
                    || Some(n.source_file.text_size_to_file_line_column(n.text_range().start())),
                    |p| Some(p.source_file.text_size_to_file_line_column(p.text_range().start())),
                )
        })
        .map(|(f, sl, sc, el, ec)| {
            use lsp_types::{Position, Range};
            let start = Position::new((sl as u32).saturating_sub(1), (sc as u32).saturating_sub(1));
            let end = Position::new((el as u32).saturating_sub(1), (ec as u32).saturating_sub(1));

            (f, Range::new(start, end))
        })
        .unwrap_or_default()
}

// triggered from the UI, running in UI thread
pub fn element_covers_point(
    x: f32,
    y: f32,
    component_instance: &ComponentInstance,
    selected_element: &ElementRc,
) -> bool {
    let click_position = LogicalPoint::from_lengths(LogicalLength::new(x), LogicalLength::new(y));

    let Some(position) = component_instance.element_position(selected_element) else {
        return false;
    };

    position.contains(click_position)
}

pub fn unselect_element() {
    super::set_selected_element(None, ComponentPositions::default());
    return;
}

fn select_element(
    x: f32,
    y: f32,
    component_instance: &ComponentInstance,
    selected_element: Option<&ElementRc>,
) -> Option<ElementRc> {
    let click_position = LogicalPoint::from_lengths(LogicalLength::new(x), LogicalLength::new(y));

    let Some(c) = selected_element else {
        unselect_element();
        return None;
    };

    let Some(position) = component_instance.element_position(&c) else {
        return None;
    };
    if position.contains(click_position) {
        let secondary_positions = if let Some((path, offset)) = element_offset(&c) {
            component_instance.component_positions(path, offset)
        } else {
            ComponentPositions::default()
        };

        super::set_selected_element(Some((&c, position)), secondary_positions);
        let document_position = lsp_element_position(&c);
        if !document_position.0.is_empty() {
            super::ask_editor_to_show_document(document_position.0, document_position.1);
        }
        return Some(c.clone());
    }

    None
}

// triggered from the UI, running in UI thread
pub fn select_element_at_impl(
    x: f32,
    y: f32,
    component_instance: &ComponentInstance,
    root_element: &ElementRc,
    current_element: Option<&ElementRc>,
    reverse: bool,
) -> Option<ElementRc> {
    let re = root_element.borrow();
    let mut fw_iter = re.children.iter();
    let mut bw_iter = re.children.iter().rev();

    let iterator: &mut dyn Iterator<Item = &ElementRc> =
        if reverse { &mut bw_iter } else { &mut fw_iter };

    let mut skip = current_element.is_some();
    for c in &mut *iterator {
        let c = self_or_embedded_component_root(c);

        if skip {
            if let Some(ce) = current_element {
                if Rc::ptr_eq(ce, &c) {
                    skip = false;
                }
            }
            continue;
        }

        if let Some(result) = select_element(x, y, component_instance, Some(&c)) {
            return Some(result);
        }
    }

    None
}

fn element_offset(element: &ElementRc) -> Option<(PathBuf, u32)> {
    let Some(node) = &element.borrow().node else {
        return None;
    };
    let path = node.source_file.path().to_path_buf();
    let offset = node.text_range().start().into();
    Some((path, offset))
}

fn element_source_range(element: &ElementRc) -> Option<(SourceFile, TextRange)> {
    let Some(node) = &element.borrow().node else {
        return None;
    };
    let source_file = node.source_file.clone();
    let range = node.text_range();
    Some((source_file, range))
}

// Return the real root element, skipping any WindowElement that got added
fn root_element(component_instance: &ComponentInstance) -> ElementRc {
    let root_element = component_instance.definition().root_component().root_element.clone();

    if root_element.borrow().children.len() != 1 {
        return root_element;
    }

    let Some(child) = root_element.borrow().children.first().cloned() else {
        return root_element;
    };
    let Some((rsf, rr)) = element_source_range(&root_element) else {
        return root_element;
    };
    let Some((csf, cr)) = element_source_range(&child) else {
        return root_element;
    };

    if Rc::ptr_eq(&rsf, &csf) && rr == cr {
        child
    } else {
        root_element
    }
}

fn parent_element(root_element: &ElementRc, element: &ElementRc) -> Option<ElementRc> {
    for c in &root_element.borrow().children {
        if Rc::ptr_eq(c, element) {
            return Some(root_element.clone());
        }
    }

    for c in &root_element.borrow().children {
        if let Some(p) = parent_element(c, element) {
            return Some(p);
        }
    }

    None
}

// triggered from the UI, running in UI thread
pub fn select_element_at(x: f32, y: f32) {
    let Some(component_instance) = super::component_instance() else {
        return;
    };

    let root_element = root_element(&component_instance);

    if let Some(selected_element) = super::selected_element() {
        if element_covers_point(x, y, &component_instance, &selected_element) {
            // We clicked on the already selected element: Do nothing!
            return;
        }

        let mut parent = parent_element(&root_element, &selected_element);
        while let Some(p) = &parent {
            if select_element_at_impl(x, y, &component_instance, p, None, true).is_some() {
                return;
            }
            parent = parent_element(&root_element, p);
        }
    }

    select_element_at_impl(x, y, &component_instance, &root_element, None, true);
}

// triggered from the UI, running in UI thread
pub fn select_element_down(x: f32, y: f32, reverse: bool) {
    let Some(component_instance) = super::component_instance() else {
        return;
    };

    // We have an actively selected element (via the earlier click-event :-):
    let Some(selected_element) = super::selected_element() else {
        return;
    };

    if !reverse {
        let _ = select_element_at_impl(x, y, &component_instance, &selected_element, None, true);
    } else {
        if element_covers_point(x, y, &component_instance, &selected_element) {
            let _ = select_element(
                x,
                y,
                &component_instance,
                parent_element(&root_element(&component_instance), &selected_element).as_ref(),
            );
        }
    }
}

// triggered from the UI, running in UI thread
pub fn select_element_front_to_back(x: f32, y: f32, reverse: bool) {
    let Some(component_instance) = super::component_instance() else {
        return;
    };

    // We have an actively selected element (via the earlier click-event :-):
    let Some(selected_element) = super::selected_element() else {
        return;
    };

    if element_covers_point(x, y, &component_instance, &selected_element) {
        let Some(parent_element) =
            parent_element(&root_element(&component_instance), &selected_element)
        else {
            return;
        };
        // We clicked on the already selected element: Do nothing!
        let _ = select_element_at_impl(
            x,
            y,
            &component_instance,
            &parent_element,
            Some(&selected_element),
            !reverse,
        );
        return;
    }
}
