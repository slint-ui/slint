// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free OR LicenseRef-Slint-commercial

use std::{path::PathBuf, rc::Rc};

use i_slint_compiler::diagnostics::SourceFile;
use i_slint_compiler::object_tree::{Component, ElementRc};
use i_slint_core::lengths::{LogicalLength, LogicalPoint};
use rowan::TextRange;
use slint_interpreter::ComponentInstance;

use crate::common::ElementRcNode;

#[derive(Clone, Debug)]
pub struct ElementSelection {
    pub path: PathBuf,
    pub offset: u32,
    pub instance_index: usize,
}

impl ElementSelection {
    pub fn as_element(&self) -> Option<ElementRc> {
        let component_instance = super::component_instance()?;

        let elements =
            component_instance.element_node_at_source_code_position(&self.path, self.offset);
        elements.get(self.instance_index).or_else(|| elements.first()).map(|(e, _)| e.clone())
    }

    pub fn as_element_node(&self) -> Option<ElementRcNode> {
        let element = self.as_element()?;

        let debug_index = {
            let e = element.borrow();
            e.debug.iter().position(|(n, _)| {
                n.source_file.path() == self.path
                    && u32::from(n.text_range().start()) == self.offset
            })
        };

        debug_index.map(|i| ElementRcNode { element, debug_index: i })
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

    component_instance
        .element_positions(selected_element)
        .iter()
        .any(|p| p.contains(click_position))
}

pub fn unselect_element() {
    super::set_selected_element(None, &[], false);
}

pub fn select_element_at_source_code_position(
    path: PathBuf,
    offset: u32,
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
        position,
        notify_editor_about_selection_after_update,
    )
}

fn select_element_at_source_code_position_impl(
    component_instance: &ComponentInstance,
    path: PathBuf,
    offset: u32,
    position: Option<LogicalPoint>,
    notify_editor_about_selection_after_update: bool,
) {
    let positions = component_instance.component_positions(&path, offset);

    let instance_index = position
        .and_then(|p| positions.iter().enumerate().find_map(|(i, g)| g.contains(p).then_some(i)))
        .unwrap_or_default();

    super::set_selected_element(
        Some(ElementSelection { path, offset, instance_index }),
        &positions,
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
    pub fn is_selected_element_node(&self, selection: &ElementRcNode) -> bool {
        self.as_element_node().map(|en| en.path_and_offset()) == Some(selection.path_and_offset())
    }

    pub fn as_element_node(&self) -> Option<ElementRcNode> {
        ElementRcNode::new(self.element.clone(), self.debug_index)
    }
}

impl std::fmt::Debug for SelectionCandidate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let tmp = self.component_stack.iter().map(|c| c.id.clone()).collect::<Vec<_>>();
        let component = format!("{:?}", tmp);
        write!(f, "SelectionCandidate {{ {:?} in {component} }}", self.as_element_node())
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
    let root_element = root_element(component_instance);
    let mut elements = Vec::new();
    collect_all_element_nodes_covering_impl(
        x,
        y,
        component_instance,
        &root_element,
        &vec![],
        &mut elements,
    );
    elements
}

pub fn is_root_element_node(
    component_instance: &ComponentInstance,
    element_node: &ElementRcNode,
) -> bool {
    let root_element = root_element(component_instance);
    let Some((root_path, root_offset)) = root_element
        .borrow()
        .debug
        .iter()
        .find(|(n, _)| !super::is_element_node_ignored(n))
        .map(|(n, _)| (n.source_file.path().to_owned(), u32::from(n.text_range().start())))
    else {
        return false;
    };

    let (path, offset) = element_node.path_and_offset();
    path == root_path && offset == root_offset
}

pub fn is_same_file_as_root_node(
    component_instance: &ComponentInstance,
    element_node: &ElementRcNode,
) -> bool {
    let root_element = root_element(component_instance);
    let Some(root_path) =
        root_element.borrow().debug.first().map(|(n, _)| n.source_file.path().to_owned())
    else {
        return false;
    };

    let (path, _) = element_node.path_and_offset();
    path == root_path
}

fn select_element_at_impl(
    component_instance: &ComponentInstance,
    x: f32,
    y: f32,
    enter_component: bool,
) -> Option<ElementRcNode> {
    for sc in &collect_all_element_nodes_covering(x, y, component_instance) {
        if let Some(en) = filter_nodes_for_selection(component_instance, sc, enter_component) {
            return Some(en);
        }
    }
    None
}

pub fn select_element_at(x: f32, y: f32, enter_component: bool) {
    let Some(component_instance) = super::component_instance() else {
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

    let Some(en) = select_element_at_impl(&component_instance, x, y, enter_component) else {
        return;
    };

    select_element_node(&component_instance, &en, Some(LogicalPoint::new(x, y)));
}

pub fn is_element_node_in_layout(element: &ElementRcNode) -> bool {
    if element.debug_index > 0 {
        // If we are not the first node, then we might have been inlined right
        // after a layout managing us
        element
            .element
            .borrow()
            .debug
            .get(element.debug_index - 1)
            .map(|d| d.1.is_some())
            .unwrap_or_default()
    } else {
        // If we are the first node, then we might be a child of a layout stored
        // in the last node of our parent element.
        let Some(parent) = i_slint_compiler::object_tree::find_parent_element(&element.element)
        else {
            return false;
        };
        let r = parent.borrow().debug.last().map(|d| d.1.is_some()).unwrap_or_default();
        r
    }
}

fn filter_nodes_for_selection(
    component_instance: &ComponentInstance,
    selection_candidate: &SelectionCandidate,
    enter_component: bool,
) -> Option<ElementRcNode> {
    let en = selection_candidate.as_element_node()?;

    if en.with_element_node(super::is_element_node_ignored) {
        return None;
    }

    if !enter_component && !is_same_file_as_root_node(component_instance, &en) {
        return None;
    }

    if is_root_element_node(component_instance, &en) {
        return None;
    }

    Some(en)
}

pub fn select_element_behind_impl(
    component_instance: &ComponentInstance,
    selected_element_node: &ElementRcNode,
    x: f32,
    y: f32,
    enter_component: bool,
    reverse: bool,
) -> Option<ElementRcNode> {
    let elements = collect_all_element_nodes_covering(x, y, component_instance);
    let current_selection_position =
        elements.iter().position(|sc| sc.is_selected_element_node(selected_element_node))?;

    let (start_position, iterations) = if reverse {
        let start_position = current_selection_position.saturating_sub(1);
        (start_position, current_selection_position)
    } else {
        let start_position = current_selection_position + 1;
        (start_position, elements.len().saturating_sub(current_selection_position + 1))
    };

    for i in 0..iterations {
        let mapped_index = if reverse {
            assert!(i <= start_position);
            start_position - i
        } else {
            assert!(i + start_position < elements.len());
            start_position + i
        };
        if let Some(en) = filter_nodes_for_selection(
            component_instance,
            elements.get(mapped_index).unwrap(),
            enter_component,
        ) {
            return Some(en);
        }
    }

    None
}

pub fn select_element_behind(x: f32, y: f32, enter_component: bool, reverse: bool) {
    let Some(component_instance) = super::component_instance() else {
        return;
    };
    let Some(selected_element_node) =
        super::selected_element().and_then(|sel| sel.as_element_node())
    else {
        return;
    };

    let Some(en) = select_element_behind_impl(
        &component_instance,
        &selected_element_node,
        x,
        y,
        enter_component,
        reverse,
    ) else {
        return;
    };

    select_element_node(&component_instance, &en, Some(LogicalPoint::new(x, y)));
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

    super::set_selected_element(Some(selected), &positions, false);
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use slint_interpreter::ComponentInstance;

    fn demo_app() -> ComponentInstance {
        crate::preview::test::compile_test(
            "fluent",
            r#"import { Button } from "std-widgets.slint";

component SomeComponent { // 69
    @children
}

component Main { // 109
    width: 200px;
    height: 200px;

    HorizontalLayout { // 160
        Rectangle { // 194
            SomeComponent { // 225
                Button { // 264
                    text: "Press me";
                }
            }
        }
    }
}

export component Entry inherits Main { /* @lsp:ignore-node */ } // 401
"#,
        )
    }

    #[test]
    fn test_find_covering_elements() {
        let component_instance = demo_app();

        let mut covers_center =
            super::collect_all_element_nodes_covering(100.0, 100.0, &component_instance);

        // Remove the "button" implenmentation details. They must be at the start:
        let button_path = PathBuf::from("builtin:/fluent-base/button.slint");
        let first_non_button = covers_center
            .iter()
            .position(|sc| {
                sc.as_element_node().map(|en| en.path_and_offset().0).as_ref() != Some(&button_path)
            })
            .unwrap();
        covers_center.drain(0..first_non_button);

        let test_file = PathBuf::from("/test_data.slint");

        let expected_offsets = [264_u32, 69, 225, 194, 160, 109, 401];
        assert_eq!(covers_center.len(), expected_offsets.len());

        for (candidate, expected_offset) in covers_center.iter().zip(&expected_offsets) {
            let (path, offset) = candidate.as_element_node().unwrap().path_and_offset();
            assert_eq!(&path, &test_file);
            assert_eq!(offset, *expected_offset);
        }

        let covers_below =
            super::collect_all_element_nodes_covering(100.0, 180.0, &component_instance);

        // All but the button itself as well as the SomeComponent (impl and use)
        assert_eq!(covers_below.len(), covers_center.len() - 3);

        for (below, center) in covers_below.iter().zip(&covers_center[3..]) {
            assert_eq!(
                below.as_element_node().map(|en| en.path_and_offset()),
                center.as_element_node().map(|en| en.path_and_offset())
            );
        }
    }

    #[test]
    fn test_element_selection() {
        let component_instance = demo_app();

        let button_path = PathBuf::from("builtin:/fluent-base/button.slint");
        let mut covers_center =
            super::collect_all_element_nodes_covering(100.0, 100.0, &component_instance)
                .iter()
                .flat_map(|sc| sc.as_element_node())
                .map(|en| en.path_and_offset())
                .collect::<Vec<_>>();
        let first_non_button = covers_center.iter().position(|(p, _)| p != &button_path).unwrap();
        covers_center.drain(1..(first_non_button - 1)); // strip all but first/last of button

        // Select without crossing file boundries
        let select =
            super::select_element_at_impl(&component_instance, 100.0, 100.0, false).unwrap();
        assert_eq!(&select.path_and_offset(), covers_center.get(2).unwrap());

        // Move deeper into the image:
        let next = super::select_element_behind_impl(
            &component_instance,
            &select,
            100.0,
            100.0,
            false,
            false,
        )
        .unwrap();
        assert_eq!(&next.path_and_offset(), covers_center.get(3).unwrap());
        let next = super::select_element_behind_impl(
            &component_instance,
            &next,
            100.0,
            100.0,
            false,
            false,
        )
        .unwrap();
        assert_eq!(&next.path_and_offset(), covers_center.get(4).unwrap());
        let next = super::select_element_behind_impl(
            &component_instance,
            &next,
            100.0,
            100.0,
            false,
            false,
        )
        .unwrap();
        assert_eq!(&next.path_and_offset(), covers_center.get(5).unwrap());
        let next = super::select_element_behind_impl(
            &component_instance,
            &next,
            100.0,
            100.0,
            false,
            false,
        )
        .unwrap();
        assert_eq!(&next.path_and_offset(), covers_center.get(6).unwrap());
        assert!(super::select_element_behind_impl(
            &component_instance,
            &next,
            100.0,
            100.0,
            false,
            false
        )
        .is_none());

        // Move towards the viewer:
        let prev = super::select_element_behind_impl(
            &component_instance,
            &next,
            100.0,
            100.0,
            false,
            true,
        )
        .unwrap();
        assert_eq!(&prev.path_and_offset(), covers_center.get(5).unwrap());
        let prev = super::select_element_behind_impl(
            &component_instance,
            &prev,
            100.0,
            100.0,
            false,
            true,
        )
        .unwrap();
        assert_eq!(&prev.path_and_offset(), covers_center.get(4).unwrap());
        let prev = super::select_element_behind_impl(
            &component_instance,
            &prev,
            100.0,
            100.0,
            false,
            true,
        )
        .unwrap();
        assert_eq!(&prev.path_and_offset(), covers_center.get(3).unwrap());
        let prev = super::select_element_behind_impl(
            &component_instance,
            &prev,
            100.0,
            100.0,
            false,
            true,
        )
        .unwrap();
        assert_eq!(&prev.path_and_offset(), covers_center.get(2).unwrap());
        assert!(super::select_element_behind_impl(
            &component_instance,
            &prev,
            100.0,
            100.0,
            false,
            true
        )
        .is_none());

        assert_eq!(
            super::select_element_behind_impl(
                &component_instance,
                &select,
                100.0,
                100.0,
                false,
                true
            ),
            None
        );

        // Select with crossing file boundries
        let select =
            super::select_element_at_impl(&component_instance, 100.0, 100.0, true).unwrap();
        assert_eq!(&select.path_and_offset(), covers_center.get(0).unwrap());

        // move to the last in the button definition:
        let mut button = select;
        loop {
            button = super::select_element_behind_impl(
                &component_instance,
                &button,
                100.0,
                100.0,
                true,
                false,
            )
            .unwrap();
            if &button.path_and_offset() == covers_center.get(1).unwrap() {
                break;
            }
        }

        // Move deeper into the image:
        let next = super::select_element_behind_impl(
            &component_instance,
            &button,
            100.0,
            100.0,
            true,
            false,
        )
        .unwrap();
        assert_eq!(&next.path_and_offset(), covers_center.get(2).unwrap());
        let next = super::select_element_behind_impl(
            &component_instance,
            &next,
            100.0,
            100.0,
            true,
            false,
        )
        .unwrap();
        assert_eq!(&next.path_and_offset(), covers_center.get(3).unwrap());
        let next = super::select_element_behind_impl(
            &component_instance,
            &next,
            100.0,
            100.0,
            true,
            false,
        )
        .unwrap();
        assert_eq!(&next.path_and_offset(), covers_center.get(4).unwrap());
        let next = super::select_element_behind_impl(
            &component_instance,
            &next,
            100.0,
            100.0,
            true,
            false,
        )
        .unwrap();
        assert_eq!(&next.path_and_offset(), covers_center.get(5).unwrap());
        let next = super::select_element_behind_impl(
            &component_instance,
            &next,
            100.0,
            100.0,
            true,
            false,
        )
        .unwrap();
        assert_eq!(&next.path_and_offset(), covers_center.get(6).unwrap());
        assert!(super::select_element_behind_impl(
            &component_instance,
            &next,
            100.0,
            100.0,
            false,
            false
        )
        .is_none());

        // Move towards the viewer:
        let prev =
            super::select_element_behind_impl(&component_instance, &next, 100.0, 100.0, true, true)
                .unwrap();
        assert_eq!(&prev.path_and_offset(), covers_center.get(5).unwrap());
        let prev =
            super::select_element_behind_impl(&component_instance, &prev, 100.0, 100.0, true, true)
                .unwrap();
        assert_eq!(&prev.path_and_offset(), covers_center.get(4).unwrap());
        let prev =
            super::select_element_behind_impl(&component_instance, &prev, 100.0, 100.0, true, true)
                .unwrap();
        assert_eq!(&prev.path_and_offset(), covers_center.get(3).unwrap());
        let prev =
            super::select_element_behind_impl(&component_instance, &prev, 100.0, 100.0, true, true)
                .unwrap();
        assert_eq!(&prev.path_and_offset(), covers_center.get(2).unwrap());
        let prev =
            super::select_element_behind_impl(&component_instance, &prev, 100.0, 100.0, true, true)
                .unwrap();
        assert_eq!(&prev.path_and_offset(), covers_center.get(1).unwrap());

        button = prev;
        loop {
            button = super::select_element_behind_impl(
                &component_instance,
                &button,
                100.0,
                100.0,
                true,
                true,
            )
            .unwrap();
            if &button.path_and_offset() == covers_center.get(0).unwrap() {
                break;
            }
        }

        assert!(super::select_element_behind_impl(
            &component_instance,
            &button,
            100.0,
            100.0,
            true,
            true
        )
        .is_none());
    }
}
