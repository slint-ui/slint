// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::{
    path::{Path, PathBuf},
    rc::Rc,
};

use i_slint_compiler::{
    object_tree::ElementRc,
    parser::{SyntaxKind, TextSize},
};
use i_slint_core::{lengths::LogicalPoint, properties::ChangeTracker};
use slint::{Model, ModelTracker, VecModel};
use slint_interpreter::{ComponentHandle, ComponentInstance, highlight::HighlightedRect};

use crate::preview::{self, SelectionNotification, ext::ElementRcNodeExt, ui};

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

    pub fn as_element_node(&self) -> Option<i_slint_editor_preview::ElementRcNode> {
        let element = self.as_element()?;

        let debug_index = {
            let e = element.borrow();
            e.debug.iter().position(|d| {
                d.node.source_file.path() == self.path && d.node.text_range().start() == self.offset
            })
        };

        debug_index.map(|i| i_slint_editor_preview::ElementRcNode { element, debug_index: i })
    }
}

// Look at an element and if it is a sub component, jump to its root_element()
fn self_or_embedded_component_root(element: &ElementRc) -> ElementRc {
    let elem = element.borrow();
    if elem.repeated.is_some()
        && let i_slint_compiler::langtype::ElementType::Component(base) = &elem.base_type
    {
        return base.root_element.clone();
    }

    element.clone()
}

fn lsp_element_node_position(
    element: &i_slint_editor_preview::ElementRcNode,
    format: i_slint_editor_preview::ByteFormat,
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

fn element_covers_point(
    position: LogicalPoint,
    component_instance: &ComponentInstance,
    selected_element: &ElementRc,
) -> Option<(HighlightedRect, usize)> {
    slint_interpreter::highlight::element_positions(
        &component_instance.clone_strong().into(),
        selected_element,
        slint_interpreter::highlight::ElementPositionFilter::ExcludeClipped,
    )
    .into_iter()
    .enumerate()
    .find(|(_, p)| p.contains(position))
    .map(|(instance_index, geometry)| (geometry, instance_index))
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
    let positions = component_instance.component_positions(&path, offset.into());

    let instance_index = position
        .and_then(|p| positions.iter().enumerate().find_map(|(i, g)| g.contains(p).then_some(i)))
        .unwrap_or_default();

    super::set_selected_element(
        Some(ElementSelection { path, offset, instance_index }),
        editor_notification,
    );
}

pub fn restore_selection(
    mut selection: ElementSelection,
    editor_notification: SelectionNotification,
) {
    let Some(component_instance) = super::component_instance() else {
        return;
    };
    if component_instance
        .component_positions(&selection.path, selection.offset.into())
        .get(selection.instance_index)
        .is_none()
    {
        selection.instance_index = 0;
    }
    super::set_selected_element(Some(selection), editor_notification);
}

struct HighlightPositionsModel {
    rows: VecModel<ui::SelectionRectangle>,
    change_tracker: ChangeTracker,
}

impl HighlightPositionsModel {
    fn new(component_instance: ComponentInstance, path: PathBuf, offset: u32) -> Rc<Self> {
        let model = Rc::new(Self { rows: Default::default(), change_tracker: Default::default() });
        let model_weak = Rc::downgrade(&model);
        let component_instance = component_instance.as_weak();
        model.change_tracker.init_delayed(
            (model_weak, component_instance, path, offset),
            |(_, component_instance, path, offset)| {
                component_instance
                    .upgrade()
                    .map(|component_instance| {
                        selection_rectangles(&component_instance, path, *offset)
                    })
                    .unwrap_or_default()
            },
            |(model_weak, _, _, _), positions| {
                if let Some(model) = model_weak.upgrade() {
                    model.update(positions);
                }
            },
        );
        model
    }

    fn update(&self, positions: &[ui::SelectionRectangle]) {
        if self.rows.row_count() != positions.len() {
            self.rows.set_vec(positions.to_vec());
            return;
        }

        for (row, position) in positions.iter().enumerate() {
            if self.rows.row_data(row).as_ref() == Some(position) {
                continue;
            }
            self.rows.set_row_data(row, position.clone());
        }
    }
}

impl Model for HighlightPositionsModel {
    type Data = ui::SelectionRectangle;

    fn row_count(&self) -> usize {
        self.rows.row_count()
    }

    fn row_data(&self, row: usize) -> Option<Self::Data> {
        self.rows.row_data(row)
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        self.rows.model_tracker()
    }
}

fn selection_rectangles(
    component_instance: &ComponentInstance,
    path: &Path,
    offset: u32,
) -> Vec<ui::SelectionRectangle> {
    component_instance
        .component_positions(path, offset)
        .iter()
        .map(|geometry| ui::SelectionRectangle {
            width: geometry.rect.size.width,
            height: geometry.rect.size.height,
            x: geometry.rect.origin.x,
            y: geometry.rect.origin.y,
            angle: geometry.angle,
        })
        .collect()
}

pub fn highlight_positions(
    source_uri: slint::SharedString,
    offset: i32,
) -> slint::ModelRc<ui::SelectionRectangle> {
    let Some(component_instance) = super::component_instance() else {
        return Default::default();
    };

    let Some(path) = lsp_types::Url::parse(source_uri.as_str())
        .ok()
        .and_then(|u| i_slint_editor_preview::uri_to_file(&u))
    else {
        return Default::default();
    };
    HighlightPositionsModel::new(component_instance, path, offset as u32).into()
}

fn select_element_node(
    component_instance: &ComponentInstance,
    selected_element: &i_slint_editor_preview::ElementRcNode,
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
    pub element: ElementRc,
    pub debug_index: usize,
    pub geometry: HighlightedRect,
    pub instance_index: usize,
    pub is_in_root_component: bool,
}

impl SelectionCandidate {
    pub fn is_selected_element_node(
        &self,
        selection: &i_slint_editor_preview::ElementRcNode,
    ) -> bool {
        self.as_element_node().map(|en| en.path_and_offset()) == Some(selection.path_and_offset())
    }

    pub fn as_element_node(&self) -> Option<i_slint_editor_preview::ElementRcNode> {
        i_slint_editor_preview::ElementRcNode::new(self.element.clone(), self.debug_index)
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

    if let Some((geometry, instance_index)) =
        element_covers_point(position, component_instance, current_element)
    {
        for (i, d) in ce.borrow().debug.iter().enumerate().rev() {
            if !i_slint_editor_preview::is_element_node_ignored(&d.node)
                && !d.node.source_file.path().starts_with("builtin:/")
            {
                // All nodes have the same geometry
                result.push(SelectionCandidate {
                    element: ce.clone(),
                    debug_index: i,
                    is_in_root_component: false,
                    geometry,
                    instance_index,
                });
            }
        }
    }
}

fn assign_is_in_root_component(candidates: &mut [SelectionCandidate]) {
    let mut root_anchor: Option<(PathBuf, i_slint_compiler::parser::TextRange)> = None;
    for sc in candidates.iter_mut().rev() {
        let Some(en) = sc.as_element_node() else {
            continue;
        };

        let (node_path, node_text_range) =
            en.with_element_node(|n| (n.source_file.path().to_path_buf(), n.text_range()));
        if let Some((rp, rtr)) = &root_anchor {
            sc.is_in_root_component = &node_path == rp && rtr.contains_range(node_text_range);
        } else {
            root_anchor = Some((node_path, node_text_range));
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

fn selection_candidate_at_impl(
    component_instance: &ComponentInstance,
    position: LogicalPoint,
    enter_component: bool,
) -> Option<SelectionCandidate> {
    collect_all_element_nodes_covering(position, component_instance)
        .into_iter()
        .find(|candidate| filter_nodes_for_selection(candidate, enter_component).is_some())
}

fn select_element_at_impl(
    component_instance: &ComponentInstance,
    position: LogicalPoint,
    enter_component: bool,
) -> Option<i_slint_editor_preview::ElementRcNode> {
    selection_candidate_at_impl(component_instance, position, enter_component)
        .and_then(|candidate| candidate.as_element_node())
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

fn type_name(element_node: &i_slint_editor_preview::ElementRcNode) -> String {
    element_node.with_element_debug(|debug_info| {
        debug_info
            .node
            .parent()
            .and_then(|parent| {
                if parent.kind() == SyntaxKind::Component {
                    parent
                        .child_node(SyntaxKind::DeclaredIdentifier)
                        .map(|identifier| identifier.text().to_string())
                } else {
                    None
                }
            })
            .or_else(|| {
                debug_info
                    .node
                    .QualifiedName()
                    .map(|qualified_name| qualified_name.text().to_string().trim().to_string())
            })
            .unwrap_or_default()
            .trim()
            .to_string()
    })
}

fn hovered_element_at_impl(
    component_instance: &ComponentInstance,
    position: LogicalPoint,
    enter_component: bool,
    selected: Option<&ElementSelection>,
) -> ui::HoveredElement {
    let Some(candidate) =
        selection_candidate_at_impl(component_instance, position, enter_component)
    else {
        return Default::default();
    };
    let Some(element_node) = candidate.as_element_node() else {
        return Default::default();
    };

    let (path, offset) = element_node.path_and_offset();
    let is_selected = selected.is_some_and(|selection| {
        selection.path == path
            && selection.offset == offset
            && selection.instance_index == candidate.instance_index
    });
    let is_over_selected_element = selected.is_some_and(|selection| {
        component_instance
            .component_positions(&selection.path, selection.offset.into())
            .get(selection.instance_index)
            .is_some_and(|geometry| geometry.contains(position))
    });

    ui::HoveredElement {
        valid: true,
        is_selected,
        is_over_selected_element,
        element_path: path.to_string_lossy().to_string().into(),
        element_offset: i32::try_from(u32::from(offset)).unwrap_or_default(),
        type_name: type_name(&element_node).into(),
        geometry: Rc::new(VecModel::from(selection_rectangles(
            component_instance,
            &path,
            offset.into(),
        )))
        .into(),
    }
}

pub fn hovered_element_at(x: f32, y: f32, enter_component: bool) -> ui::HoveredElement {
    let Some(component_instance) = super::component_instance() else {
        return Default::default();
    };

    hovered_element_at_impl(
        &component_instance,
        LogicalPoint::new(x, y),
        enter_component,
        super::selected_element().as_ref(),
    )
}

pub fn selection_stack_at(x: f32, y: f32) -> slint::ModelRc<ui::SelectionStackFrame> {
    let Some(component_instance) = &super::component_instance() else {
        return Default::default();
    };
    let root_element = root_element(component_instance);
    let Some(root_geometry) = component_instance.element_positions(&root_element).first().cloned()
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

            let root_geo = root_geometry.rect;
            let sc_geom = sc.geometry.rect;
            let width = (sc_geom.size.width / root_geo.size.width) * 100.0;
            let height = (sc_geom.size.height / root_geo.size.height) * 100.0;
            let x = ((sc_geom.origin.x + root_geo.origin.x) / root_geo.size.width) * 100.0;
            let y = ((sc_geom.origin.y + root_geo.origin.y) / root_geo.size.height) * 100.0;

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

pub fn parent_layout_kind(element: &i_slint_editor_preview::ElementRcNode) -> ui::LayoutKind {
    element.parent().map(|p| p.layout_kind()).unwrap_or(ui::LayoutKind::None)
}

fn filter_nodes_for_selection(
    selection_candidate: &SelectionCandidate,
    enter_component: bool,
) -> Option<i_slint_editor_preview::ElementRcNode> {
    if !selection_candidate.is_in_root_component && !enter_component {
        return None;
    }

    selection_candidate.as_element_node().filter(|en| {
        en.with_element_node(|n| n.parent().is_none_or(|p| p.kind() != SyntaxKind::Component))
    })
}

pub fn select_element_behind_impl(
    component_instance: &ComponentInstance,
    selected_element_node: &i_slint_editor_preview::ElementRcNode,
    position: LogicalPoint,
    enter_component: bool,
    reverse: bool,
) -> Option<i_slint_editor_preview::ElementRcNode> {
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

pub fn reselect_element() {
    super::set_selected_element(super::selected_element(), SelectionNotification::Never);
}

#[cfg(test)]
mod tests {
    use i_slint_editor_preview::test;

    use std::path::PathBuf;

    use i_slint_compiler::parser::TextSize;
    use i_slint_core::lengths::LogicalPoint;
    use slint::Model;
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
        use crate::preview::test::interpret_test_with_sources;
        use i_slint_editor_preview::test::{main_test_file_name, test_file_name};
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
            path, main_path,
            "selection without `enter_component` should land on the MyInput use site \
             in the main file, not on a node inside the imported component"
        );
    }

    #[test]
    fn test_hovered_element_matches_click_selection_and_geometry() {
        let component_instance = demo_app();
        let position = LogicalPoint::new(100.0, 100.0);
        let selected = super::select_element_at_impl(&component_instance, position, false)
            .expect("a click on the center should select an element");
        let expected_path_and_offset = selected.path_and_offset();
        let candidate = super::selection_candidate_at_impl(&component_instance, position, false)
            .expect("a selectable element should cover the center");

        let hovered = super::hovered_element_at_impl(&component_instance, position, false, None);

        assert!(hovered.valid);
        assert_eq!(hovered.element_path, expected_path_and_offset.0.to_string_lossy());
        assert_eq!(
            hovered.element_offset,
            i32::try_from(u32::from(expected_path_and_offset.1)).unwrap()
        );
        let hovered_geometry = hovered
            .geometry
            .row_data(candidate.instance_index)
            .expect("the hovered instance should have geometry");
        assert_eq!(hovered_geometry.x, candidate.geometry.rect.origin.x);
        assert_eq!(hovered_geometry.y, candidate.geometry.rect.origin.y);
        assert_eq!(hovered_geometry.width, candidate.geometry.rect.size.width);
        assert_eq!(hovered_geometry.height, candidate.geometry.rect.size.height);
        assert_eq!(hovered_geometry.angle, candidate.geometry.angle);

        let outside = super::hovered_element_at_impl(
            &component_instance,
            LogicalPoint::new(201.0, 100.0),
            false,
            None,
        );
        assert!(!outside.valid);
    }

    #[test]
    fn test_hovered_element_respects_component_boundary() {
        use crate::preview::test::interpret_test_with_sources;
        use i_slint_editor_preview::test::{main_test_file_name, test_file_name};
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
        let controls_source = r#"export component MyInput {
    width: 100%;
    height: 100%;
    Rectangle { }
}
"#;
        let component_instance = interpret_test_with_sources(
            "fluent",
            HashMap::from([
                (main_path.clone(), main_source),
                (controls_path, controls_source.to_string()),
            ]),
        );
        let position = LogicalPoint::new(100.0, 100.0);
        let hovered = super::hovered_element_at_impl(&component_instance, position, false, None);

        assert!(hovered.valid);
        assert_eq!(PathBuf::from(hovered.element_path.to_string()), main_path);
    }

    #[test]
    fn test_hovered_element_distinguishes_repeated_instances() {
        let component_instance = crate::preview::test::interpret_test(
            "fluent",
            r#"export component Main inherits Window {
    width: 120px;
    height: 60px;

    for i in [0, 1]: Rectangle {
        x: i * 60px;
        width: 50px;
        height: 50px;
    }
}
"#,
        );

        let first = super::hovered_element_at_impl(
            &component_instance,
            LogicalPoint::new(10.0, 10.0),
            false,
            None,
        );
        let second = super::hovered_element_at_impl(
            &component_instance,
            LogicalPoint::new(70.0, 10.0),
            false,
            None,
        );
        assert!(first.valid && second.valid);
        assert_eq!(first.element_path, second.element_path);
        assert_eq!(first.element_offset, second.element_offset);
        assert_eq!(first.geometry.row_count(), 2);
        assert_eq!(second.geometry.row_count(), 2);
        assert_ne!(first.geometry.row_data(0).unwrap().x, first.geometry.row_data(1).unwrap().x);

        let selection = super::ElementSelection {
            path: PathBuf::from(first.element_path.to_string()),
            offset: TextSize::from(first.element_offset as u32),
            instance_index: 1,
        };
        let selected_second = super::hovered_element_at_impl(
            &component_instance,
            LogicalPoint::new(70.0, 10.0),
            false,
            Some(&selection),
        );
        let selected_first = super::hovered_element_at_impl(
            &component_instance,
            LogicalPoint::new(10.0, 10.0),
            false,
            Some(&selection),
        );
        assert!(selected_second.is_selected);
        assert!(!selected_first.is_selected);
        assert!(selected_second.is_over_selected_element);
        assert!(!selected_first.is_over_selected_element);
    }
}
