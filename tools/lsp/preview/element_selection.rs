// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

use std::{
    path::{Path, PathBuf},
    rc::Rc,
};

use i_slint_compiler::diagnostics::SourceFile;
use i_slint_compiler::object_tree::{Component, ElementRc};
use i_slint_core::lengths::{LogicalLength, LogicalPoint};
use rowan::TextRange;
use slint_interpreter::{highlight::ComponentPositions, ComponentInstance};

#[derive(Clone, Debug)]
pub struct ElementRcNode {
    pub element: ElementRc,
    pub debug_index: usize,
}

impl ElementRcNode {
    pub fn find_in(element: ElementRc, path: &Path, offset: u32) -> Option<Self> {
        let debug_index = element.borrow().debug.iter().position(|(n, _)| {
            u32::from(n.text_range().start()) == offset && n.source_file.path() == path
        })?;

        Some(Self { element, debug_index })
    }

    pub fn with_element_debug<R>(
        &self,
        func: impl Fn(
            &i_slint_compiler::parser::syntax_nodes::Element,
            &Option<i_slint_compiler::layout::Layout>,
        ) -> R,
    ) -> R {
        let elem = self.element.borrow();
        let (n, l) = &elem.debug.get(self.debug_index).unwrap();
        func(n, l)
    }

    pub fn with_element_node<R>(
        &self,
        func: impl Fn(&i_slint_compiler::parser::syntax_nodes::Element) -> R,
    ) -> R {
        let elem = self.element.borrow();
        func(&elem.debug.get(self.debug_index).unwrap().0)
    }

    pub fn path_and_offset(&self) -> (PathBuf, u32) {
        self.with_element_node(|n| {
            (n.source_file.path().to_owned(), u32::from(n.text_range().start()))
        })
    }

    pub fn is_layout(&self) -> bool {
        self.with_element_debug(|_, l| l.is_some())
    }
}

#[derive(Clone, Debug)]
pub struct ElementSelection {
    pub path: PathBuf,
    pub offset: u32,
    pub instance_index: usize,
    pub is_layout: bool,
}

impl ElementSelection {
    pub fn as_element(&self) -> Option<ElementRc> {
        let component_instance = super::component_instance()?;

        let elements = component_instance.element_at_source_code_position(&self.path, self.offset);
        elements.get(self.instance_index).or_else(|| elements.first()).cloned()
    }
}

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

fn lsp_element_node_position(element: &ElementRcNode) -> Option<(String, lsp_types::Range)> {
    let location = element.with_element_node(|n| {
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
    super::set_selected_element(None, ComponentPositions::default(), false);
}

pub fn select_element_at_source_code_position(
    path: PathBuf,
    offset: u32,
    is_layout: bool,
    position: Option<LogicalPoint>,
    notify_editor_about_selection_after_update: bool,
) {
    let Some(component_instance) = super::component_instance() else {
        return;
    };
    select_element_at_source_code_position_impl(
        &component_instance,
        path,
        offset,
        is_layout,
        position,
        notify_editor_about_selection_after_update,
    )
}

fn select_element_at_source_code_position_impl(
    component_instance: &ComponentInstance,
    path: PathBuf,
    offset: u32,
    is_layout: bool,
    position: Option<LogicalPoint>,
    notify_editor_about_selection_after_update: bool,
) {
    let positions = component_instance.component_positions(&path, offset);

    let instance_index = position
        .and_then(|p| {
            positions.geometries.iter().enumerate().find_map(|(i, g)| g.contains(p).then_some(i))
        })
        .unwrap_or_default();

    super::set_selected_element(
        Some(ElementSelection { path, offset, instance_index, is_layout }),
        positions,
        notify_editor_about_selection_after_update,
    );
}

fn select_element_node(
    component_instance: &ComponentInstance,
    selected_element: &ElementRcNode,
    position: Option<LogicalPoint>,
) {
    let (path, offset) = selected_element.path_and_offset();

    select_element_at_source_code_position_impl(
        component_instance,
        path,
        offset,
        // FIXME: need to check which one of the node this refer to to know if this is a layout
        selected_element
            .element
            .borrow()
            .debug
            .get(selected_element.debug_index)
            .map(|(_, l)| l.is_some())
            .unwrap_or(false),
        position,
        false, // We update directly;-)
    );

    if let Some(document_position) = lsp_element_node_position(selected_element) {
        super::ask_editor_to_show_document(&document_position.0, document_position.1);
    }
}

fn element_node_source_range(
    element: &ElementRc,
    debug_index: usize,
) -> Option<(SourceFile, TextRange)> {
    let node = element.borrow().debug.get(debug_index)?.0.clone();
    let source_file = node.source_file.clone();
    let range = node.text_range();
    Some((source_file, range))
}

// Return the real root element, skipping any WindowElement that got added
pub fn root_element(component_instance: &ComponentInstance) -> ElementRc {
    let root_element = component_instance.definition().root_component().root_element.clone();
    if !root_element.borrow().debug.is_empty() {
        return root_element;
    }
    let child = root_element.borrow().children.first().cloned();
    child.unwrap_or(root_element)
}

#[derive(Clone)]
pub struct SelectionCandidate {
    pub component_stack: Vec<Rc<Component>>,
    pub element: ElementRc,
    pub debug_index: usize,
    pub text_range: Option<(SourceFile, TextRange)>,
}

impl SelectionCandidate {
    pub fn is_selected_element_node(&self, selection: &ElementSelection) -> bool {
        let Some((sf, r)) = self.text_range.as_ref() else {
            return false;
        };
        sf.path() == &selection.path && u32::from(r.start()) == selection.offset
    }

    pub fn as_element_node(&self) -> Option<ElementRcNode> {
        let (sf, range) = self.text_range.as_ref()?;
        ElementRcNode::find_in(self.element.clone(), sf.path(), u32::from(range.start()))
    }
}

impl std::fmt::Debug for SelectionCandidate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let tmp = self.component_stack.iter().map(|c| c.id.clone()).collect::<Vec<_>>();
        let component = format!("{:?}", tmp);
        write!(f, "{}({}) in {component}", self.element.borrow().id, self.debug_index)
    }
}

// Traverse the element tree in reverse render order and collect information on
// all elements that "render" at the given x and y coordinates
fn collect_all_element_nodes_covering_impl(
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
        collect_all_element_nodes_covering_impl(
            x,
            y,
            component_instance,
            c,
            children_component_stack,
            result,
        );
    }

    if element_covers_point(x, y, component_instance, &ce) {
        for (i, _) in ce.borrow().debug.iter().enumerate().rev() {
            // All nodes have the same geometry
            let text_range = element_node_source_range(&ce, i);
            result.push(SelectionCandidate {
                element: ce.clone(),
                debug_index: i,
                component_stack: component_stack.clone(),
                text_range,
            });
        }
    }
}

pub fn collect_all_element_nodes_covering(
    x: f32,
    y: f32,
    component_instance: &ComponentInstance,
) -> Vec<SelectionCandidate> {
    let root_element = root_element(&component_instance);
    let mut elements = Vec::new();
    collect_all_element_nodes_covering_impl(
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
    let Some((root_path, root_offset)) = root_element
        .borrow()
        .debug
        .get(0)
        .map(|(n, _)| (n.source_file.path().to_owned(), u32::from(n.text_range().start())))
    else {
        return;
    };

    if let Some(se) = super::selected_element() {
        if let Some(element) = se.as_element() {
            if element_covers_point(x, y, &component_instance, &element) {
                // We clicked on the already selected element: Do nothing!
                return;
            }
        }
    }

    for sc in &collect_all_element_nodes_covering(x, y, &component_instance) {
        let Some(en) = sc.as_element_node() else {
            continue;
        };
        let (path, offset) = en.path_and_offset();

        if en.with_element_node(|n| super::is_element_node_ignored(n)) {
            continue;
        }
        if !enter_component && path != root_path {
            continue;
        }
        if path == root_path && offset == root_offset {
            continue;
        }

        select_element_node(&component_instance, &en, Some(LogicalPoint::new(x, y)));
        break;
    }
}

pub fn select_element_behind(x: f32, y: f32, enter_component: bool, reverse: bool) {
    let Some(component_instance) = super::component_instance() else {
        return;
    };
    let root_element = root_element(&component_instance);
    let Some((root_path, root_offset)) = root_element
        .borrow()
        .debug
        .get(0)
        .map(|(n, _)| (n.source_file.path().to_owned(), u32::from(n.text_range().start())))
    else {
        return;
    };

    let elements = collect_all_element_nodes_covering(x, y, &component_instance);

    let Some(selected_element_data) = super::selected_element() else {
        return;
    };

    let Some(current_selection_position) =
        elements.iter().position(|sc| sc.is_selected_element_node(&selected_element_data))
    else {
        return;
    };

    let target_range = if reverse {
        if current_selection_position == 0 {
            return;
        }
        (current_selection_position - 1)..=0
    } else {
        if current_selection_position == elements.len() - 1 {
            return;
        }
        (current_selection_position + 1)..=elements.len() - 1
    };

    for i in target_range {
        let sc = elements.get(i).unwrap();
        let Some(en) = sc.as_element_node() else {
            continue;
        };
        let (path, offset) = en.path_and_offset();

        if en.with_element_node(|n| super::is_element_node_ignored(n)) {
            continue;
        }

        if !enter_component && !en.with_element_node(|n| n.source_file.path() == &root_path) {
            continue;
        }

        if path == root_path && offset == root_offset {
            continue;
        }

        select_element_node(&component_instance, &en, Some(LogicalPoint::new(x, y)));
        break;
    }
}

// Called from UI thread!
pub fn reselect_element() {
    let Some(selected) = super::selected_element() else {
        return;
    };
    let Some(component_instance) = super::component_instance() else {
        return;
    };
    let positions = component_instance.component_positions(&selected.path, selected.offset);

    super::set_selected_element(Some(selected), positions, false);
}
