// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_compiler::source_path::SourcePath;
use std::{path::PathBuf, rc::Rc};

use i_slint_compiler::{
    object_tree::ElementRc,
    parser::{SyntaxKind, TextSize},
};
use i_slint_core::lengths::LogicalPoint;
use slint_interpreter::{ComponentInstance, highlight::HighlightedRect};

use crate::editor_preview;
use crate::preview::{self, SelectionNotification, ext::ElementRcNodeExt, ui};

#[derive(Clone, Debug)]
pub struct ElementSelection {
    pub path: PathBuf,
    pub offset: TextSize,
    pub instance_index: usize,
}

impl ElementSelection {
    fn source_path(&self) -> SourcePath {
        SourcePath::new(&self.path)
    }

    fn as_element_in(&self, component_instance: &ComponentInstance) -> Option<ElementRc> {
        let elements = component_instance
            .element_node_at_source_code_position(&self.source_path(), self.offset.into());
        elements
            .get(self.instance_index)
            .or_else(|| elements.first())
            .map(|(element, _)| element.clone())
    }

    fn as_element_node_in(
        &self,
        component_instance: &ComponentInstance,
    ) -> Option<editor_preview::ElementRcNode> {
        let element = self.as_element_in(component_instance)?;

        let debug_index = {
            let element = element.borrow();
            element.debug.iter().position(|debug_info| {
                debug_info.node.source_file.path_buf() == self.path
                    && debug_info.node.text_range().start() == self.offset
            })
        };

        debug_index.map(|debug_index| editor_preview::ElementRcNode { element, debug_index })
    }

    pub fn as_element_node(&self) -> Option<editor_preview::ElementRcNode> {
        let component_instance = super::component_instance()?;
        self.as_element_node_in(&component_instance)
    }
}

fn lsp_element_node_position(
    element: &editor_preview::ElementRcNode,
    format: editor_preview::ByteFormat,
) -> Option<(String, lsp_types::Range)> {
    let (f, sl, sc, el, ec) = element.with_element_node(|n| {
        n.parent()
            .filter(|p| p.kind() == i_slint_compiler::parser::SyntaxKind::SubElement)
            .map_or_else(
                || n.source_file.text_size_to_file_line_column(n.text_range().start(), format),
                |p| p.source_file.text_size_to_file_line_column(p.text_range().start(), format),
            )
    });

    use lsp_types::{Position, Range};
    let start = Position::new((sl as u32).saturating_sub(1), (sc as u32).saturating_sub(1));
    let end = Position::new((el as u32).saturating_sub(1), (ec as u32).saturating_sub(1));
    Some((f, Range::new(start, end)))
}

fn element_geometries(
    component_instance: &ComponentInstance,
    element: &ElementRc,
) -> Vec<HighlightedRect> {
    let element = element.borrow();
    let Some(debug_info) = element.debug.first() else { return Vec::new() };
    component_instance.component_positions(
        debug_info.node.source_file.path(),
        debug_info.node.text_range().start().into(),
    )
}

pub fn unselect_element() {
    super::set_selected_element(None, SelectionNotification::Never);
}

pub fn select_element_at_source_code_position(
    path: PathBuf,
    offset: TextSize,
    position: Option<LogicalPoint>,
    editor_notification: preview::SelectionNotification,
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
    let positions = component_instance.component_positions(&SourcePath::new(&path), offset.into());

    let instance_index = position
        .and_then(|p| positions.iter().enumerate().find_map(|(i, g)| g.contains(p).then_some(i)))
        .unwrap_or_default();

    super::set_selected_element(
        Some(ElementSelection { path, offset, instance_index }),
        editor_notification,
    );
}

pub fn highlight_positions(
    source_uri: slint::SharedString,
    offset: i32,
) -> slint::ModelRc<ui::SelectionRectangle> {
    let Some(component_instance) = super::component_instance() else {
        return Default::default();
    };

    let Some(path) = crate::Url::parse(source_uri.as_str())
        .ok()
        .and_then(|u| crate::editor_preview::uri_to_file(&u))
    else {
        return Default::default();
    };
    let offset = TextSize::new(offset as u32);
    let positions = component_instance.component_positions(&path, offset.into());
    let model = slint::VecModel::from_iter(positions.iter().map(|g| ui::SelectionRectangle {
        width: g.rect.size.width,
        height: g.rect.size.height,
        x: g.rect.origin.x,
        y: g.rect.origin.y,
        angle: g.angle,
    }));
    slint::ModelRc::new(model)
}

fn select_element_node(
    component_instance: &ComponentInstance,
    selected_element: &editor_preview::ElementRcNode,
    position: Option<LogicalPoint>,
) {
    let (path, offset) = selected_element.path_and_offset();

    select_element_at_source_code_position_impl(
        component_instance,
        path.to_path_buf(),
        offset,
        position,
        SelectionNotification::Never, // We update directly;-)
    );

    let format = preview::PREVIEW_STATE.with_borrow(|ps| ps.format());

    if let Some(document_position) = lsp_element_node_position(selected_element, format) {
        let to_lsp = preview::PREVIEW_STATE.with_borrow(|ps| ps.to_lsp.borrow().clone().unwrap());
        to_lsp.ask_editor_to_show_document(&document_position.0, document_position.1, false).ok();
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
    pub selection: ElementSelection,
    pub geometry: HighlightedRect,
    pub is_in_root_component: bool,
}

impl SelectionCandidate {
    pub fn is_selected_element_node(
        &self,
        component_instance: &ComponentInstance,
        selection: &editor_preview::ElementRcNode,
    ) -> bool {
        self.as_element_node(component_instance).map(|element| element.path_and_offset())
            == Some(selection.path_and_offset())
    }

    pub fn as_element_node(
        &self,
        component_instance: &ComponentInstance,
    ) -> Option<editor_preview::ElementRcNode> {
        self.selection.as_element_node_in(component_instance)
    }
}

impl std::fmt::Debug for SelectionCandidate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SelectionCandidate {{ {:?} }}@({:?})", self.selection, self.geometry)
    }
}

fn assign_is_in_root_component(
    component_instance: &ComponentInstance,
    candidates: &mut [SelectionCandidate],
) {
    let mut root_anchor: Option<(PathBuf, i_slint_compiler::parser::TextRange)> = None;
    for candidate in candidates.iter_mut().rev() {
        let Some(element) = candidate.as_element_node(component_instance) else {
            continue;
        };

        let (node_path, node_text_range) = element
            .with_element_node(|node| (node.source_file.path().to_path_buf(), node.text_range()));
        if let Some((root_path, root_text_range)) = &root_anchor {
            candidate.is_in_root_component =
                &node_path == root_path && root_text_range.contains_range(node_text_range);
        } else {
            root_anchor = Some((node_path, node_text_range));
            candidate.is_in_root_component = true;
        }
    }
}

pub fn collect_all_element_nodes_covering(
    position: LogicalPoint,
    component_instance: &ComponentInstance,
) -> Vec<SelectionCandidate> {
    let mut candidates = component_instance
        .element_candidates_at(position)
        .into_iter()
        .filter_map(|candidate| {
            let source_file = candidate.source_location.source_file.as_ref()?;
            if matches!(source_file.path(), SourcePath::Builtin(_)) {
                return None;
            }
            let offset = u32::try_from(candidate.source_location.span.offset).ok()?.into();
            let selection = ElementSelection {
                path: source_file.path().to_path_buf(),
                offset,
                instance_index: candidate.instance_index,
            };
            let element = selection.as_element_node_in(component_instance)?;
            if editor_preview::is_element_node_ignored(
                &element.with_element_debug(|debug_info| debug_info.node.clone()),
            ) {
                return None;
            }
            Some(SelectionCandidate {
                selection,
                is_in_root_component: false,
                geometry: candidate.geometry,
            })
        })
        .collect::<Vec<_>>();
    assign_is_in_root_component(component_instance, &mut candidates);
    candidates
}

fn select_element_at_impl(
    component_instance: &ComponentInstance,
    position: LogicalPoint,
    enter_component: bool,
) -> Option<editor_preview::ElementRcNode> {
    for candidate in &collect_all_element_nodes_covering(position, component_instance) {
        if let Some(element) =
            filter_nodes_for_selection(component_instance, candidate, enter_component)
        {
            return Some(element);
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

pub fn selection_stack_at(x: f32, y: f32) -> slint::ModelRc<ui::SelectionStackFrame> {
    let Some(component_instance) = &super::component_instance() else {
        return Default::default();
    };
    let root_element = root_element(component_instance);
    let Some(root_geometry) =
        element_geometries(component_instance, &root_element).first().cloned()
    else {
        return Default::default();
    };

    let position = LogicalPoint::new(x, y);

    let (known_components, mut selected) = preview::PREVIEW_STATE.with(|preview_state| {
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
        .filter(|candidate| {
            filter_nodes_for_selection(component_instance, candidate, true).is_some()
        })
        .map(|candidate| {
            let (type_name, id, is_layout, is_selected, path, offset) = candidate
                .as_element_node(component_instance)
                .map(|element| {
                    let (path, offset) = element.path_and_offset();
                    let path = path.to_path_buf();
                    let offset: u32 = offset.into();

                    let is_selected = if selected.is_none() {
                        select_element_node(component_instance, &element, Some(position));
                        selected = Some(element.clone());
                        true
                    } else {
                        selected.as_ref() == Some(&element)
                    };

                    let (type_name, id, is_layout) = element.with_element_debug(|debug_info| {
                        let id = debug_info
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
                            debug_info
                                .node
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
                                    debug_info
                                        .node
                                        .QualifiedName()
                                        .map(|qn| qn.text().to_string().trim().to_string())
                                })
                                .unwrap_or_default()
                                .trim()
                                .to_string()
                        };

                        (type_name, id, debug_info.layout.is_some())
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

            let root_geo = root_geometry.rect;
            let candidate_geometry = candidate.geometry.rect;
            let width = (candidate_geometry.size.width / root_geo.size.width) * 100.0;
            let height = (candidate_geometry.size.height / root_geo.size.height) * 100.0;
            let x =
                ((candidate_geometry.origin.x + root_geo.origin.x) / root_geo.size.width) * 100.0;
            let y =
                ((candidate_geometry.origin.y + root_geo.origin.y) / root_geo.size.height) * 100.0;

            let is_interactive = known_components
                .iter()
                .position(|kc| kc.name.as_str() == type_name.as_str())
                .map(|index| known_components.get(index).unwrap().is_interactive)
                .unwrap_or_default();

            ui::SelectionStackFrame {
                width,
                height,
                x,
                y,
                is_in_root_component: candidate.is_in_root_component,
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
    model: slint::ModelRc<ui::SelectionStackFrame>,
    filter_text: slint::SharedString,
    filter: ui::SelectionStackFilter,
) -> slint::ModelRc<ui::SelectionStackFrame> {
    use slint::ModelExt;
    use ui::{SelectionStackFilter, SelectionStackFrame};

    fn filter_fn(frame: &SelectionStackFrame, filter: SelectionStackFilter) -> bool {
        match filter {
            SelectionStackFilter::Nothing => false,
            SelectionStackFilter::Layouts => frame.is_layout,
            SelectionStackFilter::Interactive => frame.is_interactive,
            SelectionStackFilter::Others => !frame.is_interactive && !frame.is_layout,
            SelectionStackFilter::LayoutsAndInteractive => frame.is_layout || frame.is_interactive,
            SelectionStackFilter::LayoutsAndOthers => frame.is_layout || !frame.is_interactive,
            SelectionStackFilter::InteractiveAndOthers => frame.is_interactive || !frame.is_layout,
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

pub fn parent_layout_kind(element: &editor_preview::ElementRcNode) -> ui::LayoutKind {
    element.parent().map(|p| p.layout_kind()).unwrap_or(ui::LayoutKind::None)
}

fn filter_nodes_for_selection(
    component_instance: &ComponentInstance,
    selection_candidate: &SelectionCandidate,
    enter_component: bool,
) -> Option<editor_preview::ElementRcNode> {
    if !selection_candidate.is_in_root_component && !enter_component {
        return None;
    }

    selection_candidate.as_element_node(component_instance).filter(|element| {
        element.with_element_node(|node| {
            node.parent().is_none_or(|parent| parent.kind() != SyntaxKind::Component)
        })
    })
}

pub fn select_element_behind_impl(
    component_instance: &ComponentInstance,
    selected_element_node: &editor_preview::ElementRcNode,
    position: LogicalPoint,
    enter_component: bool,
    reverse: bool,
) -> Option<editor_preview::ElementRcNode> {
    let elements = collect_all_element_nodes_covering(position, component_instance);
    let current_selection_position = elements.iter().position(|candidate| {
        candidate.is_selected_element_node(component_instance, selected_element_node)
    })?;

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
        if let Some(element) = filter_nodes_for_selection(
            component_instance,
            elements.get(mapped_index).unwrap(),
            enter_component,
        ) {
            return Some(element);
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

pub fn reselect_element() {
    super::set_selected_element(super::selected_element(), SelectionNotification::Never);
}

#[cfg(test)]
mod tests {
    use crate::editor_preview::test;

    use i_slint_compiler::source_path::SourcePath;

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
        let button_path = SourcePath::new("builtin:/fluent/button.slint");
        let first_non_button = covers_center
            .iter()
            .position(|candidate| {
                candidate
                    .as_element_node(&type_loader)
                    .map(|element| element.path_and_offset().0)
                    .as_ref()
                    != Some(&button_path)
            })
            .unwrap();
        covers_center.drain(0..first_non_button);

        let test_file = test::test_file_name("test_data.slint");

        let expected_offsets = [264_u32, 69, 225, 194, 160, 109];
        assert_eq!(covers_center.len(), expected_offsets.len());

        for (candidate, expected_offset) in covers_center.iter().zip(&expected_offsets) {
            let (path, offset) = candidate.as_element_node(&type_loader).unwrap().path_and_offset();
            assert_eq!(path, SourcePath::new(&test_file));
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
                below.as_element_node(&type_loader).map(|element| element.path_and_offset()),
                center.as_element_node(&type_loader).map(|element| element.path_and_offset())
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
        .flat_map(|candidate| candidate.as_element_node(&component_instance))
        .map(|element| element.path_and_offset())
        .collect::<Vec<_>>();

        tracing::debug!("Covers:");
        for (i, (p, ts)) in covers_center.iter().enumerate() {
            tracing::debug!("   {i}: {p:?}:{ts:?}");
        }
        tracing::debug!("Done");

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
        assert!(
            super::select_element_behind_impl(
                &component_instance,
                &select,
                LogicalPoint::new(100.0, 100.0),
                false,
                true
            )
            .is_none()
        );

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

        assert!(
            super::select_element_behind_impl(
                &component_instance,
                &next,
                LogicalPoint::new(100.0, 100.0),
                false,
                false
            )
            .is_none()
        );

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

        assert!(
            super::select_element_behind_impl(
                &component_instance,
                &next,
                LogicalPoint::new(100.0, 100.0),
                true,
                false
            )
            .is_none()
        );

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

        assert!(
            super::select_element_behind_impl(
                &component_instance,
                &prev,
                LogicalPoint::new(100.0, 100.0),
                true,
                true
            )
            .is_none()
        );
    }

    #[test]
    fn test_select_imported_component_at_use_site() {
        use crate::editor_preview::test::{main_test_file_name, test_file_name};
        use crate::preview::test::interpret_test_with_sources;
        use std::collections::HashMap;

        let main_path = main_test_file_name();
        let controls_path = test_file_name("controls.slint");

        let main_source = format!(
            r#"import {{ MyInput }} from "{controls}";

export component Demo inherits Window {{
    width: 200px;
    height: 200px;

    MyInput {{
        x: 0px; y: 0px;
        width: 200px;
        height: 200px;
    }}
}}
"#,
            controls = controls_path.to_string_lossy()
        );

        let controls_source = r#"component InputBlocker {
    width: 100%;
    height: 100%;
    TouchArea {
        clicked => { }
    }
}

export component MyInput {
    width: 100%;
    height: 100%;
    auth-checker := InputBlocker { }
}
"#;

        let component_instance = interpret_test_with_sources(
            "fluent",
            HashMap::from([
                (main_path.clone(), main_source),
                (controls_path.clone(), controls_source.to_string()),
            ]),
        );

        let selected = super::select_element_at_impl(
            &component_instance,
            LogicalPoint::new(100.0, 100.0),
            /* enter_component */ false,
        )
        .expect("a click on MyInput should select something");

        let (path, _offset) = selected.path_and_offset();
        assert_eq!(
            path,
            SourcePath::new(&main_path),
            "selection without `enter_component` should land on the MyInput use site \
             in the main file, not on a node inside the imported component"
        );
    }

    #[test]
    fn test_selection_distinguishes_inlined_component_uses() {
        let source = r#"import { GroupBox, VerticalBox } from "std-widgets.slint";
component Page inherits VerticalBox {
    Rectangle { height: 40px; }
    @children
}
export component Main inherits Page {
    width: 200px;
    height: 400px;
    first := GroupBox {
        vertical-stretch: 0;
        height: 80px;
        title: "First";
    }
    second := GroupBox {
        vertical-stretch: 0;
        height: 80px;
        title: "Second";
    }
    third := GroupBox {
        vertical-stretch: 0;
        height: 80px;
        title: "Third";
    }
    fourth := GroupBox {
        vertical-stretch: 0;
        height: 80px;
        title: "Fourth";
    }
}
"#;
        let component_instance = crate::preview::test::interpret_test("fluent", source);
        let path = test::main_test_file_name();
        let first_offset =
            u32::try_from(source.find("GroupBox {\n        vertical-stretch").unwrap()).unwrap();
        let fourth_offset = u32::try_from(source.rfind("GroupBox {").unwrap()).unwrap();

        for (expected_offset, unexpected_offset) in
            [(first_offset, fourth_offset), (fourth_offset, first_offset)]
        {
            let geometry = component_instance
                .component_positions(&SourcePath::new(&path), expected_offset)
                .into_iter()
                .next()
                .expect("the GroupBox should have geometry");
            let position = LogicalPoint::new(
                geometry.rect.origin.x + geometry.rect.size.width / 2.0,
                geometry.rect.origin.y + geometry.rect.size.height / 2.0,
            );
            let selected = super::select_element_at_impl(&component_instance, position, false)
                .expect("the GroupBox should be selectable");
            assert_eq!(u32::from(selected.path_and_offset().1), expected_offset);

            let covering_offsets =
                super::collect_all_element_nodes_covering(position, &component_instance)
                    .into_iter()
                    .filter(|candidate| candidate.selection.path == path)
                    .map(|candidate| u32::from(candidate.selection.offset))
                    .collect::<Vec<_>>();
            assert!(covering_offsets.contains(&expected_offset));
            assert!(!covering_offsets.contains(&unexpected_offset));
        }
    }
}
