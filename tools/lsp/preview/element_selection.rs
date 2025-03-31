// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::{path::PathBuf, rc::Rc};

use i_slint_compiler::{
    object_tree::ElementRc,
    parser::{SyntaxKind, TextSize},
};
use i_slint_core::lengths::{LogicalPoint, LogicalRect};
use slint_interpreter::{ComponentHandle, ComponentInstance};

use crate::common;

use crate::preview::{ext::ElementRcNodeExt, ui, SelectionNotification};

#[derive(Clone, Debug)]
pub struct ElementSelection {
    pub path: PathBuf,
    pub offset: TextSize,
    pub instance_index: usize,
}

impl ElementSelection {
    pub fn as_element(&self) -> Option<ElementRc> {
        let component_instance = super::component_instance()?;

        let elements =
            component_instance.element_node_at_source_code_position(&self.path, self.offset.into());
        elements.get(self.instance_index).or_else(|| elements.first()).map(|(e, _)| e.clone())
    }

    pub fn as_element_node(&self) -> Option<common::ElementRcNode> {
        let element = self.as_element()?;

        let debug_index = {
            let e = element.borrow();
            e.debug.iter().position(|d| {
                d.node.source_file.path() == self.path && d.node.text_range().start() == self.offset
            })
        };

        debug_index.map(|i| common::ElementRcNode { element, debug_index: i })
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

fn lsp_element_node_position(
    element: &common::ElementRcNode,
) -> Option<(String, lsp_types::Range)> {
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
    position: LogicalPoint,
    component_instance: &ComponentInstance,
    selected_element: &ElementRc,
) -> Option<LogicalRect> {
    slint_interpreter::highlight::element_positions(
        &component_instance.clone_strong().into(),
        selected_element,
        slint_interpreter::highlight::ElementPositionFilter::ExcludeClipped,
    )
    .iter()
    .find(|p| p.contains(position))
    .copied()
}

pub fn unselect_element() {
    super::set_selected_element(None, &[], SelectionNotification::Never);
}

pub fn select_element_at_source_code_position(
    path: PathBuf,
    offset: TextSize,
    position: Option<LogicalPoint>,
    editor_notification: crate::preview::SelectionNotification,
) {
    let Some(component_instance) = super::component_instance() else {
        return;
    };
    select_element_at_source_code_position_impl(
        &component_instance,
        path,
        offset,
        position,
        editor_notification,
    )
}

fn select_element_at_source_code_position_impl(
    component_instance: &ComponentInstance,
    path: PathBuf,
    offset: TextSize,
    position: Option<LogicalPoint>,
    editor_notification: SelectionNotification,
) {
    let positions = component_instance.component_positions(&path, offset.into());

    let instance_index = position
        .and_then(|p| positions.iter().enumerate().find_map(|(i, g)| g.contains(p).then_some(i)))
        .unwrap_or_default();

    super::set_selected_element(
        Some(ElementSelection { path, offset, instance_index }),
        &positions,
        editor_notification,
    );
}

fn select_element_node(
    component_instance: &ComponentInstance,
    selected_element: &common::ElementRcNode,
    position: Option<LogicalPoint>,
) {
    let (path, offset) = selected_element.path_and_offset();

    select_element_at_source_code_position_impl(
        component_instance,
        path,
        offset,
        position,
        SelectionNotification::Never, // We update directly;-)
    );

    if let Some(document_position) = lsp_element_node_position(selected_element) {
        super::ask_editor_to_show_document(&document_position.0, document_position.1, false);
    }
}

// Return the real root element, skipping the WindowElement that might got added
pub fn root_element(component_instance: &ComponentInstance) -> ElementRc {
    let root_element = component_instance.definition().root_component().root_element.clone();
    if root_element.borrow().debug.is_empty() {
        // The root element has no debug set if it is a window inserted by the compiler.
        // That window will have one child -- the "real root", but it might
        // have a few more compiler-generated nodes in front or behind the "real root"!
        let child =
            root_element.borrow().children.iter().find(|c| !c.borrow().debug.is_empty()).cloned();
        child.unwrap_or(root_element)
    } else {
        root_element
    }
}

#[derive(Clone)]
pub struct SelectionCandidate {
    pub element: ElementRc,
    pub debug_index: usize,
    pub geometry: LogicalRect,
    pub is_in_root_component: bool,
}

impl SelectionCandidate {
    pub fn is_selected_element_node(&self, selection: &common::ElementRcNode) -> bool {
        self.as_element_node().map(|en| en.path_and_offset()) == Some(selection.path_and_offset())
    }

    pub fn as_element_node(&self) -> Option<common::ElementRcNode> {
        common::ElementRcNode::new(self.element.clone(), self.debug_index)
    }
}

impl std::fmt::Debug for SelectionCandidate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SelectionCandidate {{ {:?} }}@({:?})", self.as_element_node(), self.geometry)
    }
}

// Traverse the element tree in reverse render order and collect information on
// all elements that "render" at the given x and y coordinates
fn collect_all_element_nodes_covering_impl(
    position: LogicalPoint,
    component_instance: &ComponentInstance,
    current_element: &ElementRc,
    result: &mut Vec<SelectionCandidate>,
) {
    let ce = self_or_embedded_component_root(current_element);

    for c in ce.borrow().children.iter().rev() {
        collect_all_element_nodes_covering_impl(position, component_instance, c, result);
    }

    if let Some(geometry) = element_covers_point(position, component_instance, current_element) {
        for (i, d) in ce.borrow().debug.iter().enumerate().rev() {
            if !common::is_element_node_ignored(&d.node)
                && !d.node.source_file.path().starts_with("builtin:/")
            {
                // All nodes have the same geometry
                result.push(SelectionCandidate {
                    element: ce.clone(),
                    debug_index: i,
                    is_in_root_component: false,
                    geometry,
                });
            }
        }
    }
}

fn assign_is_in_root_component(candidates: &mut Vec<SelectionCandidate>) {
    let mut root_text_range: Option<i_slint_compiler::parser::TextRange> = None;
    for sc in candidates.iter_mut().rev() {
        let Some(en) = sc.as_element_node() else {
            continue;
        };

        let node_text_range = en.with_element_node(|n| n.text_range());
        if let Some(rtr) = root_text_range {
            sc.is_in_root_component = rtr.contains_range(node_text_range);
        } else {
            root_text_range = Some(node_text_range);
            sc.is_in_root_component = true;
        }
    }
}

pub fn collect_all_element_nodes_covering(
    position: LogicalPoint,
    component_instance: &ComponentInstance,
) -> Vec<SelectionCandidate> {
    let root_element = root_element(component_instance);
    let mut elements = Vec::new();
    collect_all_element_nodes_covering_impl(
        position,
        component_instance,
        &root_element,
        &mut elements,
    );

    assign_is_in_root_component(&mut elements);

    elements
}

fn select_element_at_impl(
    component_instance: &ComponentInstance,
    position: LogicalPoint,
    enter_component: bool,
) -> Option<common::ElementRcNode> {
    for sc in &collect_all_element_nodes_covering(position, component_instance) {
        if let Some(en) = filter_nodes_for_selection(sc, enter_component) {
            return Some(en);
        }
    }
    None
}

pub fn select_element_at(x: f32, y: f32, enter_component: bool) {
    let Some(component_instance) = super::component_instance() else {
        return;
    };

    let position = LogicalPoint::new(x, y);

    let Some(en) = select_element_at_impl(&component_instance, position, enter_component) else {
        return;
    };

    select_element_node(&component_instance, &en, Some(position));
}

pub fn selection_stack_at(
    x: f32,
    y: f32,
) -> slint::ModelRc<crate::preview::ui::SelectionStackFrame> {
    let Some(component_instance) = &super::component_instance() else {
        return Default::default();
    };
    let root_element = root_element(component_instance);
    let Some(root_geometry) = component_instance.element_positions(&root_element).first().cloned()
    else {
        return Default::default();
    };

    let position = LogicalPoint::new(x, y);

    let (known_components, mut selected) = crate::preview::PREVIEW_STATE.with(|preview_state| {
        let preview_state = preview_state.borrow();

        let known_components = preview_state.known_components.clone();
        let selected =
            preview_state.selected.as_ref().and_then(|s| s.as_element_node()).filter(|en| {
                en.geometries(component_instance).iter().any(|gr| gr.contains(position))
            });

        (known_components, selected)
    });

    let mut longest_path_prefix = PathBuf::new();

    let mut result = collect_all_element_nodes_covering(position, component_instance)
        .iter()
        .filter(|sn| filter_nodes_for_selection(sn, true).is_some())
        .map(|sc| {
            let (type_name, id, is_layout, is_selected, path, offset) = sc
                .as_element_node()
                .map(|en| {
                    let (path, offset) = en.path_and_offset();
                    let offset: u32 = offset.into();

                    let is_selected = if selected.is_none() {
                        select_element_node(component_instance, &en, Some(position));
                        selected = Some(en.clone());
                        true
                    } else {
                        selected.as_ref() == Some(&en)
                    };

                    let (type_name, id, is_layout) = en.with_element_debug(|di| {
                        let id = di
                            .node
                            .parent()
                            .and_then(|p| {
                                if p.kind() == SyntaxKind::SubElement {
                                    p.child_token(SyntaxKind::Identifier)
                                        .map(|t| t.text().to_string())
                                } else {
                                    None
                                }
                            })
                            .unwrap_or_default();

                        let type_name = {
                            di.node
                                .parent()
                                .and_then(|p| {
                                    if p.kind() == SyntaxKind::Component {
                                        p.child_node(SyntaxKind::DeclaredIdentifier)
                                            .map(|t| t.text().to_string())
                                    } else {
                                        None
                                    }
                                })
                                .or_else(|| {
                                    di.node
                                        .QualifiedName()
                                        .map(|qn| qn.text().to_string().trim().to_string())
                                })
                                .unwrap_or_default()
                                .trim()
                                .to_string()
                        };

                        (type_name, id, di.layout.is_some())
                    });

                    (type_name, id, is_layout, is_selected, path, offset)
                })
                .unwrap_or_default();

            if path.strip_prefix("/@").is_err() && path != PathBuf::new() {
                if longest_path_prefix == PathBuf::new() {
                    longest_path_prefix = path.clone();
                } else {
                    longest_path_prefix =
                        std::iter::zip(longest_path_prefix.components(), path.components())
                            .take_while(|(l, p)| l == p)
                            .map(|(l, _)| l)
                            .collect();
                }
            }

            let width = (sc.geometry.size.width / root_geometry.size.width) * 100.0;
            let height = (sc.geometry.size.height / root_geometry.size.height) * 100.0;
            let x = ((sc.geometry.origin.x + root_geometry.origin.x) / root_geometry.size.width)
                * 100.0;
            let y = ((sc.geometry.origin.y + root_geometry.origin.y) / root_geometry.size.height)
                * 100.0;

            let is_interactive = known_components
                .iter()
                .position(|kc| kc.name.as_str() == type_name.as_str())
                .map(|index| known_components.get(index).unwrap().is_interactive)
                .unwrap_or_default();

            crate::preview::ui::SelectionStackFrame {
                width,
                height,
                x,
                y,
                is_in_root_component: sc.is_in_root_component,
                is_selected,
                is_layout,
                is_interactive,
                type_name: type_name.into(),
                file_name: path.to_string_lossy().to_string().into(),
                element_path: path.to_string_lossy().to_string().into(),
                element_offset: offset as i32,
                id: id.into(),
            }
        })
        .collect::<Vec<_>>();

    for frame in result.iter_mut() {
        let file_name = PathBuf::from(frame.file_name.to_string());
        let new_file_name = {
            if let Some(library) = file_name.to_string_lossy().strip_prefix("/@") {
                format!("@{library:?}")
            } else if file_name == longest_path_prefix {
                file_name.file_name().unwrap_or_default().to_string_lossy().to_string()
            } else {
                file_name
                    .strip_prefix(&longest_path_prefix)
                    .unwrap_or(&file_name)
                    .to_string_lossy()
                    .to_string()
            }
        };
        frame.file_name = new_file_name.into();
    }

    Rc::new(slint::VecModel::from(result)).into()
}

pub fn filter_sort_selection_stack(
    model: slint::ModelRc<crate::preview::ui::SelectionStackFrame>,
    filter_text: slint::SharedString,
    filter: crate::preview::ui::SelectionStackFilter,
) -> slint::ModelRc<crate::preview::ui::SelectionStackFrame> {
    use crate::preview::ui::{SelectionStackFilter, SelectionStackFrame};
    use slint::ModelExt;

    fn filter_fn(frame: &SelectionStackFrame, filter: SelectionStackFilter) -> bool {
        match filter {
            SelectionStackFilter::Nothing => false,
            SelectionStackFilter::Layouts => frame.is_layout,
            SelectionStackFilter::Interactive => frame.is_interactive,
            SelectionStackFilter::Others => !frame.is_interactive && !frame.is_layout,
            SelectionStackFilter::LayoutsAndInteractive => frame.is_layout || frame.is_interactive,
            SelectionStackFilter::LayoutsAndOthers => {
                frame.is_layout || (!frame.is_layout && !frame.is_interactive)
            }
            SelectionStackFilter::InteractiveAndOthers => {
                frame.is_interactive || (!frame.is_layout && !frame.is_interactive)
            }
            SelectionStackFilter::Everything => true,
        }
    }

    let filter_text = filter_text.to_string();

    if filter_text.is_empty() && filter == SelectionStackFilter::Everything {
        model
    } else if filter_text.as_str().chars().any(|c| !c.is_lowercase()) {
        Rc::new(model.filter(move |frame| {
            filter_fn(frame, filter)
                && (frame.id.contains(&filter_text)
                    || frame.type_name.contains(&filter_text)
                    || frame.file_name.contains(&filter_text))
        }))
        .into()
    } else {
        Rc::new(model.filter(move |frame| {
            filter_fn(frame, filter)
                && (frame.id.to_lowercase().contains(&filter_text)
                    || frame.type_name.to_lowercase().contains(&filter_text)
                    || frame.file_name.to_lowercase().contains(&filter_text))
        }))
        .into()
    }
}

pub fn parent_layout_kind(element: &common::ElementRcNode) -> ui::LayoutKind {
    element.parent().map(|p| p.layout_kind()).unwrap_or(ui::LayoutKind::None)
}

fn filter_nodes_for_selection(
    selection_candidate: &SelectionCandidate,
    enter_component: bool,
) -> Option<common::ElementRcNode> {
    if !selection_candidate.is_in_root_component && !enter_component {
        return None;
    }

    selection_candidate.as_element_node().filter(|en| {
        en.with_element_node(|n| n.parent().map_or(true, |p| p.kind() != SyntaxKind::Component))
    })
}

pub fn select_element_behind_impl(
    component_instance: &ComponentInstance,
    selected_element_node: &common::ElementRcNode,
    position: LogicalPoint,
    enter_component: bool,
    reverse: bool,
) -> Option<common::ElementRcNode> {
    let elements = collect_all_element_nodes_covering(position, component_instance);
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
        if let Some(en) =
            filter_nodes_for_selection(elements.get(mapped_index).unwrap(), enter_component)
        {
            return Some(en);
        }
    }

    None
}

pub fn select_element_behind(x: f32, y: f32, enter_component: bool, reverse: bool) {
    let Some(component_instance) = super::component_instance() else {
        return;
    };
    let position = LogicalPoint::new(x, y);
    let Some(selected_element_node) =
        super::selected_element().and_then(|sel| sel.as_element_node())
    else {
        return;
    };

    let Some(en) = select_element_behind_impl(
        &component_instance,
        &selected_element_node,
        position,
        enter_component,
        reverse,
    ) else {
        return;
    };

    select_element_node(&component_instance, &en, Some(position));
}

// Called from UI thread!
pub fn reselect_element() {
    let Some(selected) = super::selected_element() else {
        super::set_selected_element(None, &[], SelectionNotification::Never);
        return;
    };
    let Some(component_instance) = super::component_instance() else {
        return;
    };
    let positions = component_instance.component_positions(&selected.path, selected.offset.into());

    super::set_selected_element(Some(selected), &positions, SelectionNotification::Never);
}

#[cfg(test)]
mod tests {
    use crate::common::test;

    use std::path::PathBuf;

    use i_slint_core::lengths::LogicalPoint;
    use slint_interpreter::ComponentInstance;

    fn demo_app() -> ComponentInstance {
        crate::preview::test::interpret_test(
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
        let type_loader = demo_app();

        let mut covers_center = super::collect_all_element_nodes_covering(
            LogicalPoint::new(100.0, 100.0),
            &type_loader,
        );

        // Remove the "button" implementation details. They must be at the start:
        let button_path = PathBuf::from("builtin:/fluent/button.slint");
        let first_non_button = covers_center
            .iter()
            .position(|sc| {
                sc.as_element_node().map(|en| en.path_and_offset().0).as_ref() != Some(&button_path)
            })
            .unwrap();
        covers_center.drain(0..first_non_button);

        let test_file = test::test_file_name("test_data.slint");

        let expected_offsets = [264_u32, 69, 225, 194, 160, 109];
        assert_eq!(covers_center.len(), expected_offsets.len());

        for (candidate, expected_offset) in covers_center.iter().zip(&expected_offsets) {
            let (path, offset) = candidate.as_element_node().unwrap().path_and_offset();
            assert_eq!(&path, &test_file);
            assert_eq!(offset, (*expected_offset).into());
        }

        let covers_below = super::collect_all_element_nodes_covering(
            LogicalPoint::new(100.0, 180.0),
            &type_loader,
        );

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

        let covers_center = super::collect_all_element_nodes_covering(
            LogicalPoint::new(100.0, 100.0),
            &component_instance,
        )
        .iter()
        .flat_map(|sc| sc.as_element_node())
        .map(|en| en.path_and_offset())
        .collect::<Vec<_>>();

        eprintln!("Covers:");
        for (i, (p, ts)) in covers_center.iter().enumerate() {
            println!("   {i}: {p:?}:{ts:?}");
        }
        eprintln!("Done");

        // Select without crossing boundaries
        // --------------------------------------------------------------------
        let select = super::select_element_at_impl(
            &component_instance,
            LogicalPoint::new(100.0, 100.0),
            false,
        )
        .unwrap();
        assert_eq!(&select.path_and_offset(), covers_center.first().unwrap());

        // Try to move towards the viewer:
        assert!(super::select_element_behind_impl(
            &component_instance,
            &select,
            LogicalPoint::new(100.0, 100.0),
            false,
            true
        )
        .is_none());

        // Move deeper into the image:
        let next = super::select_element_behind_impl(
            &component_instance,
            &select,
            LogicalPoint::new(100.0, 100.0),
            false,
            false,
        )
        .unwrap();
        assert_eq!(&next.path_and_offset(), covers_center.get(2).unwrap());
        let next = super::select_element_behind_impl(
            &component_instance,
            &next,
            LogicalPoint::new(100.0, 100.0),
            false,
            false,
        )
        .unwrap();
        assert_eq!(&next.path_and_offset(), covers_center.get(3).unwrap());
        let next = super::select_element_behind_impl(
            &component_instance,
            &next,
            LogicalPoint::new(100.0, 100.0),
            false,
            false,
        )
        .unwrap();
        assert_eq!(&next.path_and_offset(), covers_center.get(4).unwrap());

        assert!(super::select_element_behind_impl(
            &component_instance,
            &next,
            LogicalPoint::new(100.0, 100.0),
            false,
            false
        )
        .is_none());

        // Move towards the viewer:
        let prev = super::select_element_behind_impl(
            &component_instance,
            &next,
            LogicalPoint::new(100.0, 100.0),
            false,
            true,
        )
        .unwrap();
        assert_eq!(&prev.path_and_offset(), covers_center.get(3).unwrap());
        let prev = super::select_element_behind_impl(
            &component_instance,
            &prev,
            LogicalPoint::new(100.0, 100.0),
            false,
            true,
        )
        .unwrap();
        assert_eq!(&prev.path_and_offset(), covers_center.get(2).unwrap());
        let prev = super::select_element_behind_impl(
            &component_instance,
            &prev,
            LogicalPoint::new(100.0, 100.0),
            false,
            true,
        )
        .unwrap();
        assert_eq!(&prev.path_and_offset(), covers_center.first().unwrap());

        // Select with crossing component boundaries
        // --------------------------------------------------------------------
        let select = super::select_element_at_impl(
            &component_instance,
            LogicalPoint::new(100.0, 100.0),
            true,
        )
        .unwrap();
        assert_eq!(&select.path_and_offset(), covers_center.first().unwrap());

        // Move deeper into the image:
        let next = super::select_element_behind_impl(
            &component_instance,
            &select,
            LogicalPoint::new(100.0, 100.0),
            true,
            false,
        )
        .unwrap();
        assert_eq!(&next.path_and_offset(), covers_center.get(2).unwrap());
        let next = super::select_element_behind_impl(
            &component_instance,
            &next,
            LogicalPoint::new(100.0, 100.0),
            true,
            false,
        )
        .unwrap();
        assert_eq!(&next.path_and_offset(), covers_center.get(3).unwrap());
        let next = super::select_element_behind_impl(
            &component_instance,
            &next,
            LogicalPoint::new(100.0, 100.0),
            true,
            false,
        )
        .unwrap();
        assert_eq!(&next.path_and_offset(), covers_center.get(4).unwrap());

        assert!(super::select_element_behind_impl(
            &component_instance,
            &next,
            LogicalPoint::new(100.0, 100.0),
            true,
            false
        )
        .is_none());

        // Move towards the viewer:
        let prev = super::select_element_behind_impl(
            &component_instance,
            &next,
            LogicalPoint::new(100.0, 100.0),
            true,
            true,
        )
        .unwrap();
        assert_eq!(&prev.path_and_offset(), covers_center.get(3).unwrap());
        let prev = super::select_element_behind_impl(
            &component_instance,
            &prev,
            LogicalPoint::new(100.0, 100.0),
            true,
            true,
        )
        .unwrap();
        assert_eq!(&prev.path_and_offset(), covers_center.get(2).unwrap());
        let prev = super::select_element_behind_impl(
            &component_instance,
            &prev,
            LogicalPoint::new(100.0, 100.0),
            true,
            true,
        )
        .unwrap();
        assert_eq!(&prev.path_and_offset(), covers_center.first().unwrap());

        assert!(super::select_element_behind_impl(
            &component_instance,
            &prev,
            LogicalPoint::new(100.0, 100.0),
            true,
            true
        )
        .is_none());
    }
}
