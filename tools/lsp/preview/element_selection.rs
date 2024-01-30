// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

use std::{path::PathBuf, rc::Rc};

use i_slint_compiler::{
    diagnostics::{SourceFile, Spanned},
    object_tree::{Component, ElementRc},
};
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

fn lsp_element_position(element: &ElementRc) -> Option<(String, lsp_types::Range)> {
    let e = element.borrow();
    let location = crate::common::filter_ignore_nodes_in_element(&e).next().and_then(|n| {
        n.parent()
            .filter(|p| p.kind() == i_slint_compiler::parser::SyntaxKind::SubElement)
            .map_or_else(
                || Some(n.source_file.text_size_to_file_line_column(n.text_range().start())),
                |p| Some(p.source_file.text_size_to_file_line_column(p.text_range().start())),
            )
    });
    location.map(|(f, sl, sc, el, ec)| {
        use lsp_types::{Position, Range};
        let start = Position::new((sl as u32).saturating_sub(1), (sc as u32).saturating_sub(1));
        let end = Position::new((el as u32).saturating_sub(1), (ec as u32).saturating_sub(1));

        (f, Range::new(start, end))
    })
}

fn element_covers_point(
    x: f32,
    y: f32,
    component_instance: &ComponentInstance,
    selected_element: &ElementRc,
) -> bool {
    let click_position = LogicalPoint::from_lengths(LogicalLength::new(x), LogicalLength::new(y));

    component_instance.element_position(selected_element).iter().any(|p| p.contains(click_position))
}

pub fn unselect_element() {
    super::set_selected_element(None, ComponentPositions::default());
}

fn select_element(component_instance: &ComponentInstance, selected_element: &ElementRc) {
    let secondary_positions = if let Some((path, offset)) = element_offset(selected_element) {
        component_instance.component_positions(path, offset)
    } else {
        ComponentPositions::default()
    };

    let Some(position) = secondary_positions.geometries.get(0).cloned() else {
        return;
    };

    super::set_selected_element(Some((&selected_element, position)), secondary_positions);
    if let Some(document_position) = lsp_element_position(&selected_element) {
        super::ask_editor_to_show_document(&document_position.0, document_position.1);
    }
}

fn element_offset(element: &ElementRc) -> Option<(PathBuf, u32)> {
    let Some(node) = element.borrow().node.first().cloned() else {
        return None;
    };
    let path = node.source_file.path().to_path_buf();
    let offset = node.text_range().start().into();
    Some((path, offset))
}

fn element_source_range(element: &ElementRc) -> Option<(SourceFile, TextRange)> {
    let node = element.borrow().node.first().cloned()?;
    let source_file = node.source_file.clone();
    let range = node.text_range();
    Some((source_file, range))
}

// Return the real root element, skipping any WindowElement that got added
fn root_element(component_instance: &ComponentInstance) -> ElementRc {
    let root_element = component_instance.definition().root_component().root_element.clone();
    if !root_element.borrow().node.is_empty() {
        return root_element;
    }
    let child = root_element.borrow().children.first().cloned();
    child.unwrap_or(root_element)
}

#[derive(Clone)]
struct SelectionCandidate {
    component_stack: Vec<Rc<Component>>,
    element: ElementRc,
    text_range: Option<(SourceFile, TextRange)>,
}

impl SelectionCandidate {
    fn is_element(&self, element: &ElementRc) -> bool {
        Rc::ptr_eq(&self.element, element)
    }

    fn is_component_root_element(&self) -> bool {
        let Some(c) = self.component_stack.last() else {
            return false;
        };
        Rc::ptr_eq(&self.element, &c.root_element)
    }

    fn is_builtin(&self) -> bool {
        let elem = self.element.borrow();
        let Some(node) = elem.node.first() else {
            return true;
        };
        let Some(sf) = node.source_file() else {
            return true;
        };
        sf.path().starts_with("builtin:/")
    }

    fn same_file(&self, element: &ElementRc) -> bool {
        let Some((s, _)) = &self.text_range else {
            return false;
        };
        let Some((o, _)) = &element_source_range(element) else {
            return false;
        };

        s.path() == o.path()
    }
}

impl std::fmt::Debug for SelectionCandidate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let tmp = self.component_stack.iter().map(|c| c.id.clone()).collect::<Vec<_>>();
        let component = format!("{:?}", tmp);
        write!(f, "{} in {component}", self.element.borrow().id)
    }
}

// Traverse the element tree in reverse render order and collect information on
// all elements that "render" at the given x and y coordinates
fn collect_all_elements_covering_impl(
    x: f32,
    y: f32,
    component_instance: &ComponentInstance,
    current_element: &ElementRc,
    component_stack: &Vec<Rc<Component>>,
    result: &mut Vec<SelectionCandidate>,
) {
    let ce = self_or_embedded_component_root(current_element);
    let Some(component) = ce.borrow().enclosing_component.upgrade() else {
        return;
    };
    let component_root_element = component.root_element.clone();

    let mut tmp;
    let children_component_stack = {
        if Rc::ptr_eq(&component_root_element, &ce) {
            tmp = component_stack.clone();
            tmp.push(component.clone());
            &tmp
        } else {
            component_stack
        }
    };

    for c in ce.borrow().children.iter().rev() {
        collect_all_elements_covering_impl(
            x,
            y,
            component_instance,
            c,
            children_component_stack,
            result,
        );
    }

    if element_covers_point(x, y, component_instance, &ce) {
        let text_range = element_source_range(&ce);
        result.push(SelectionCandidate {
            element: ce,
            component_stack: component_stack.clone(),
            text_range,
        });
    }
}

fn collect_all_elements_covering(
    x: f32,
    y: f32,
    component_instance: &ComponentInstance,
    root_element: &ElementRc,
) -> Vec<SelectionCandidate> {
    let mut elements = Vec::new();
    collect_all_elements_covering_impl(
        x,
        y,
        &component_instance,
        &root_element,
        &vec![],
        &mut elements,
    );
    elements
}

pub fn select_element_at(x: f32, y: f32, enter_component: bool) {
    let Some(component_instance) = super::component_instance() else {
        return;
    };

    let root_element = root_element(&component_instance);

    if let Some(se) = super::selected_element() {
        if element_covers_point(x, y, &component_instance, &se) {
            // We clicked on the already selected element: Do nothing!
            return;
        }
    }

    let elements = collect_all_elements_covering(x, y, &component_instance, &root_element);

    if let Some(element) = elements
        .iter()
        .filter(|sc| enter_component || sc.same_file(&root_element))
        .filter(|sc| !(sc.is_builtin() && !sc.is_component_root_element()))
        .filter(|sc| !sc.is_element(&root_element))
        .next()
    {
        select_element(&component_instance, &element.element);
    }
}

pub fn select_element_behind(x: f32, y: f32, enter_component: bool, reverse: bool) {
    let Some(component_instance) = super::component_instance() else {
        return;
    };

    let root_element = root_element(&component_instance);
    let Some(selected_element) = super::selected_element() else {
        return;
    };
    let Some(selected_component) = selected_element.borrow().enclosing_component.upgrade() else {
        return;
    };

    let elements = collect_all_elements_covering(x, y, &component_instance, &root_element);

    let to_select = {
        let it = elements
            .iter()
            .filter(|sc| {
                !(sc.is_builtin() && !sc.is_component_root_element())
                    || sc.is_element(&selected_element)
            })
            .filter(|sc| {
                enter_component || sc.same_file(&root_element) || sc.is_element(&selected_element)
            })
            .filter(|sc| {
                !Rc::ptr_eq(&sc.element, &selected_component.root_element)
                    && !Rc::ptr_eq(&sc.element, &root_element)
            }); // Filter out the component root itself and the root, we want to select inside of that

        if reverse {
            it.take_while(|sc| !sc.is_element(&selected_element)).last()
        } else {
            it.skip_while(|sc| !sc.is_element(&selected_element)).nth(1)
        }
    };

    if let Some(ts) = to_select {
        select_element(&component_instance, &ts.element);
    }
}
