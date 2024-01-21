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
    component_instance: &ComponentInstance,
    selected_element: &ElementRc,
    layer: usize,
) {
    eprintln!("  select_element({}, {layer})", selected_element.borrow().id);
    let Some(position) = component_instance.element_position(&selected_element) else {
        return;
    };

    let secondary_positions = if let Some((path, offset)) = element_offset(selected_element) {
        component_instance.component_positions(path, offset)
    } else {
        ComponentPositions::default()
    };

    super::set_selected_element(Some((&selected_element, position, layer)), secondary_positions);
    let document_position = lsp_element_position(&selected_element);
    if !document_position.0.is_empty() {
        super::ask_editor_to_show_document(document_position.0, document_position.1);
    }
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

fn visit_tree_element(
    x: f32,
    y: f32,
    component_instance: &ComponentInstance,
    root_element: &ElementRc,
    current_element: &ElementRc,
    target_layer: usize,
    current_layer: usize,
    switch_files: bool,
    previous: &(usize, Option<ElementRc>),
) -> ((usize, Option<ElementRc>), (usize, Option<ElementRc>)) {
    let ce = self_or_embedded_component_root(current_element);

    let mut current_layer = current_layer;
    let mut previous = previous.clone();

    for c in ce.borrow().children.iter().rev() {
        let (p, (ncl, fe)) = visit_tree_element(
            x,
            y,
            component_instance,
            root_element,
            c,
            target_layer,
            current_layer,
            switch_files,
            &previous,
        );

        current_layer = ncl;
        previous = p;

        if fe.is_some() {
            return (previous, (current_layer, fe));
        }
    }

    if element_covers_point(x, y, component_instance, &ce)
        && !Rc::ptr_eq(current_element, root_element)
    {
        current_layer += 1;

        let same_source = (|| {
            let Some(re) = &root_element.borrow().node else {
                return false;
            };
            let Some(ce) = &ce.borrow().node else {
                return false;
            };
            Rc::ptr_eq(&re.source_file, &ce.source_file)
        })();
        let file_ok = switch_files || same_source;

        if file_ok && current_layer < target_layer && current_layer > previous.0 {
            eprintln!(
                "    visit: {x},{y} (target: {target_layer}/{current_layer}): {} => Found prev candidate in self!",
                ce.borrow().id
            );
            previous = (current_layer, Some(ce.clone()))
        }

        if file_ok && current_layer > target_layer {
            eprintln!(
                "    visit: {x},{y} (target: {target_layer}/{current_layer}): {} => Found next in self!",
                ce.borrow().id
            );
            return (previous, (current_layer, Some(ce)));
        }

        if file_ok && current_layer < target_layer {
            previous = (current_layer, Some(ce.clone()));
        }
    }

    // eprintln!("    visit: {x},{y} (target: {target_layer}/{current_layer}): {} => mot found", current_element.borrow().id);
    (previous, (current_layer, None))
}

pub fn select_element_at(x: f32, y: f32) {
    let Some(component_instance) = super::component_instance() else {
        return;
    };

    let root_element = root_element(&component_instance);

    if let Some((selected_element, _)) = super::selected_element() {
        if element_covers_point(x, y, &component_instance, &selected_element) {
            // We clicked on the already selected element: Do nothing!
            return;
        }
    }

    let (_, (layer, next)) = visit_tree_element(
        x,
        y,
        &component_instance,
        &root_element,
        &root_element,
        0,
        0,
        false,
        &(0, None),
    );
    if let Some(n) = next {
        select_element(&component_instance, &n, layer);
    }
}

pub fn select_element_behind(x: f32, y: f32, switch_files: bool, reverse: bool) {
    let Some(component_instance) = super::component_instance() else {
        return;
    };

    let root_element = root_element(&component_instance);
    let target_layer = super::selected_element().map(|(_, l)| l).unwrap_or_default();
    eprintln!("select_element_behind: {x},{y} (switch: {switch_files}, reverse: {reverse}), target: {target_layer}");

    let (previous, next) = visit_tree_element(
        x,
        y,
        &component_instance,
        &root_element,
        &root_element,
        target_layer,
        0,
        switch_files,
        &(0, None),
    );
    eprintln!("select_element_behind: {x},{y} (switch: {switch_files}, reverse: {reverse}) => Prev: {:?}, Next: {:?}", previous.1.as_ref().map(|e| e.borrow().id.clone()), next.1.as_ref().map(|e| e.borrow().id.clone()));
    let to_select = if reverse { previous } else { next };
    eprintln!("select_element_behind: {x},{y} (switch: {switch_files}, reverse: {reverse}) => To select: {:?}", to_select.1.as_ref().map(|e| e.borrow().id.clone()));
    if let (layer, Some(ts)) = to_select {
        eprintln!("select_element_behind: {x},{y} (switch: {switch_files}, reverse: {reverse}) => SETTING {}@{layer}", ts.borrow().id);
        select_element(&component_instance, &ts, layer);
    }
}
