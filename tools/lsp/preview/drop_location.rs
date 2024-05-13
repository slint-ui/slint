// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.2 OR LicenseRef-Slint-commercial

use i_slint_compiler::diagnostics::SourceFile;
use i_slint_compiler::parser::{syntax_nodes, SyntaxKind, SyntaxNode};
use i_slint_core::lengths::{LogicalPoint, LogicalRect, LogicalSize};
use slint_interpreter::ComponentInstance;

use crate::common;
use crate::language::completion;
use crate::preview::{self, element_selection, ui};
use crate::util;

use crate::preview::ext::ElementRcNodeExt;

#[cfg(target_arch = "wasm32")]
use crate::wasm_prelude::*;

pub fn placeholder() -> String {
    format!(
        " Rectangle {{ min-width: 16px; min-height: 16px; /* {} */ }}",
        preview::NODE_IGNORE_COMMENT
    )
}

#[derive(Clone, Debug)]
pub struct TextOffsetAdjustment {
    pub start_offset: u32,
    pub end_offset: u32,
    pub new_text_length: u32,
}

impl TextOffsetAdjustment {
    pub fn new(
        edit: &lsp_types::TextEdit,
        source_file: &i_slint_compiler::diagnostics::SourceFile,
    ) -> Self {
        let new_text_length = edit.new_text.len() as u32;
        let (start_offset, end_offset) = {
            let so = source_file
                .offset(edit.range.start.line as usize, edit.range.start.character as usize);
            let eo =
                source_file.offset(edit.range.end.line as usize, edit.range.end.character as usize);
            (std::cmp::min(so, eo) as u32, std::cmp::max(so, eo) as u32)
        };

        Self { start_offset, end_offset, new_text_length }
    }

    pub fn adjust(&self, offset: u32) -> u32 {
        // This is a bit simplistic... Worst case: Some unexpected element gets selected. We can live with that.

        debug_assert!(self.end_offset >= self.start_offset);
        let old_length = self.end_offset - self.start_offset;

        if offset >= self.end_offset {
            offset + self.new_text_length - old_length
        } else if offset >= self.start_offset {
            (offset as i64 + self.new_text_length as i64 - old_length as i64).clamp(
                self.start_offset as i64,
                self.end_offset.min(self.start_offset + self.new_text_length) as i64,
            ) as u32
        } else {
            offset
        }
    }
}

#[derive(Clone, Default)]
pub struct TextOffsetAdjustments(Vec<TextOffsetAdjustment>);

impl TextOffsetAdjustments {
    pub fn add_adjustment(&mut self, adjustment: TextOffsetAdjustment) {
        self.0.push(adjustment);
    }

    pub fn adjust(&self, input: u32) -> u32 {
        let input_ = i64::from(input);
        let total_adjustment =
            self.0.iter().fold(0_i64, |acc, a| acc + i64::from(a.adjust(input)) - input_);
        (input_ + total_adjustment) as u32
    }
}

#[derive(Clone)]
enum EditIteratorState<'a> {
    Changes { urls: Vec<&'a lsp_types::Url>, main_index: usize, index: usize },
    DocumentChanges { main_index: usize, index: usize },
    Done,
}

#[derive(Clone)]
struct EditIterator<'a> {
    workspace_edit: &'a lsp_types::WorkspaceEdit,
    state: EditIteratorState<'a>,
}

impl<'a> EditIterator<'a> {
    pub fn new(workspace_edit: &'a lsp_types::WorkspaceEdit) -> Self {
        Self {
            workspace_edit,
            state: EditIteratorState::Changes {
                urls: workspace_edit
                    .changes
                    .as_ref()
                    .map(|hm| hm.keys().collect::<Vec<_>>())
                    .unwrap_or_default(),
                main_index: 0,
                index: 0,
            },
        }
    }
}

impl<'a> Iterator for EditIterator<'a> {
    type Item = (lsp_types::OptionalVersionedTextDocumentIdentifier, &'a lsp_types::TextEdit);

    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.state {
            EditIteratorState::Changes { urls, main_index, index } => {
                if let Some(changes) = &self.workspace_edit.changes {
                    if let Some(uri) = urls.get(*main_index) {
                        if let Some(edits) = changes.get(uri) {
                            if let Some(edit) = edits.get(*index) {
                                *index += 1;
                                return Some((
                                    lsp_types::OptionalVersionedTextDocumentIdentifier {
                                        uri: (*uri).clone(),
                                        version: None,
                                    },
                                    edit,
                                ));
                            } else {
                                *index = 0;
                                *main_index += 1;
                                return self.next();
                            }
                        }
                    }
                }

                self.state = EditIteratorState::DocumentChanges { main_index: 0, index: 0 };
                self.next()
            }
            EditIteratorState::DocumentChanges { main_index, index } => {
                if let Some(lsp_types::DocumentChanges::Edits(edits)) =
                    &self.workspace_edit.document_changes
                {
                    if let Some(doc_edit) = edits.get(*main_index) {
                        if let Some(edit) = doc_edit.edits.get(*index) {
                            *index += 1;
                            let te = match edit {
                                lsp_types::OneOf::Left(te) => te,
                                lsp_types::OneOf::Right(ate) => &ate.text_edit,
                            };
                            return Some((doc_edit.text_document.clone(), te));
                        } else {
                            *index = 0;
                            *main_index += 1;
                            return self.next();
                        }
                    }
                }

                self.state = EditIteratorState::Done;
                None
            }
            EditIteratorState::Done => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct DropInformation {
    pub target_element_node: common::ElementRcNode,
    pub insert_info: InsertInformation,
    pub drop_mark: Option<DropMark>,
    /// Child to insert *before* (or usize::MAX)
    pub child_index: usize,
}

#[derive(Clone, Debug)]
pub struct InsertInformation {
    pub insertion_position: common::VersionedPosition,
    pub replacement_range: u32,
    pub pre_indent: String,
    pub indent: String,
    pub post_indent: String,
}

#[derive(Clone, Debug)]
enum DropAccept {
    Yes,   // This element will definitely handle the drop event
    Maybe, // This element can handle the drop event, but maybe someone else is better suited
    No,
}

fn border_size(dimension: f32) -> f32 {
    let bs = (dimension / 4.0).floor();
    if bs > 8.0 {
        8.0
    } else {
        bs
    }
}

// We calculate the area where the drop event will be handled for certain and those where
// we might want to delegate to something else.
//
// The idea is to delegate to lower elements when we hit a `Maybe`.
// Changing the conditions of when to stop the delegation allows to fine-tune
// the results. I expect this to happen based on the kind of layout seen in the
// stack of `ElementRcNode`s.
fn calculate_drop_acceptance(
    geometry: &LogicalRect,
    position: LogicalPoint,
    layout_kind: &crate::preview::ui::LayoutKind,
) -> DropAccept {
    assert!(geometry.contains(position)); // Just checked that before calling this

    let horizontal = border_size(geometry.size.width);
    let vertical = border_size(geometry.size.height);

    let certain_rect = match layout_kind {
        ui::LayoutKind::None => LogicalRect::new(
            LogicalPoint::new(geometry.origin.x + horizontal, geometry.origin.y + vertical),
            LogicalSize::new(
                geometry.size.width - (2.0 * horizontal),
                geometry.size.height - (2.0 * vertical),
            ),
        ),
        ui::LayoutKind::Horizontal => LogicalRect::new(
            LogicalPoint::new(geometry.origin.x, geometry.origin.y + vertical),
            LogicalSize::new(geometry.size.width, geometry.size.height - (2.0 * vertical)),
        ),
        ui::LayoutKind::Vertical => LogicalRect::new(
            LogicalPoint::new(geometry.origin.x + horizontal, geometry.origin.y),
            LogicalSize::new(geometry.size.width - (2.0 * horizontal), geometry.size.height),
        ),
        ui::LayoutKind::Grid => *geometry,
    };

    if certain_rect.contains(position) {
        DropAccept::Yes
    } else {
        DropAccept::Maybe
    }
}

#[derive(Debug)]
struct Zone {
    start: f32,
    end: f32,
}

struct DropZoneIterator<'a> {
    input: Box<dyn Iterator<Item = (usize, (bool, (f32, f32)))> + 'a>,
    last_mid: f32,
    last_end: f32,
    start: f32,
    end: f32,
    state: DropZoneIteratorState,
}

#[derive(Debug)]
enum DropZoneIteratorState {
    NotStarted,
    InProgress,
    AtEnd,
}
impl<'a> DropZoneIterator<'a> {
    fn new(start: f32, end: f32, input: impl Iterator<Item = (bool, (f32, f32))> + 'a) -> Self {
        Self {
            input: Box::new(input.enumerate()),
            last_mid: start,
            last_end: start,
            start,
            end,
            state: DropZoneIteratorState::NotStarted,
        }
    }
}

impl<'a> Iterator for DropZoneIterator<'a> {
    type Item = (usize, Zone, Zone);

    fn next(&mut self) -> Option<Self::Item> {
        if let Some((current_index, (is_selected, (cur_start, cur_end)))) = self.input.next() {
            let cur_mid = cur_start + (cur_end - cur_start) / 2.0;

            let last_mid = self.last_mid;
            let last_end = self.last_end;

            self.last_mid = cur_mid;
            self.last_end = cur_end;

            if is_selected {
                if let Some((_, (next_is_selected, (next_start, next_end)))) = self.input.next() {
                    assert!(!next_is_selected); // We can not handle the same element twice in the same layout:-)
                    self.state = DropZoneIteratorState::InProgress;

                    let next_mid = next_start + (next_end - next_start) / 2.0;

                    self.last_mid = next_mid;
                    self.last_end = next_end;

                    return Some((
                        current_index,
                        Zone { start: last_mid, end: next_mid },
                        Zone { start: cur_start, end: cur_end },
                    ));
                } else {
                    self.state = DropZoneIteratorState::AtEnd;

                    return Some((
                        current_index,
                        Zone { start: last_mid, end: self.end },
                        Zone { start: cur_start, end: cur_end },
                    ));
                }
            }

            match self.state {
                DropZoneIteratorState::NotStarted => {
                    self.state = DropZoneIteratorState::InProgress;
                    Some((
                        current_index,
                        Zone { start: self.start, end: cur_mid },
                        Zone { start: self.start, end: self.start + 1.0 },
                    ))
                }
                DropZoneIteratorState::InProgress => {
                    self.state = DropZoneIteratorState::InProgress;
                    let drop_loc = last_end + (cur_start - last_end) / 2.0;
                    Some((
                        current_index,
                        Zone { start: last_mid, end: cur_mid },
                        Zone { start: drop_loc, end: drop_loc + 1.0 },
                    ))
                }
                DropZoneIteratorState::AtEnd => None,
            }
        } else {
            match self.state {
                DropZoneIteratorState::NotStarted => {
                    self.state = DropZoneIteratorState::AtEnd;
                    Some((
                        usize::MAX,
                        Zone { start: self.start, end: self.end },
                        Zone {
                            start: self.start + (self.end - self.start) / 2.0,
                            end: self.start + 1.0 + (self.end - self.start) / 2.0,
                        },
                    ))
                }
                DropZoneIteratorState::InProgress => {
                    self.state = DropZoneIteratorState::AtEnd;
                    Some((
                        usize::MAX,
                        Zone { start: self.last_mid, end: self.end },
                        Zone { start: self.end - 1.0, end: self.end },
                    ))
                }
                DropZoneIteratorState::AtEnd => None,
            }
        }
    }
}

// calculate where to draw the `DropMark`
fn calculate_drop_information_for_layout(
    geometry: &LogicalRect,
    position: LogicalPoint,
    layout_kind: &crate::preview::ui::LayoutKind,
    children_geometries: &[(bool, LogicalRect)],
) -> (Option<DropMark>, usize) {
    match layout_kind {
        ui::LayoutKind::None => unreachable!("We are in a layout"),
        ui::LayoutKind::Horizontal => {
            for (index, hit_zone, drop_zone) in DropZoneIterator::new(
                geometry.origin.x,
                geometry.origin.x + geometry.size.width,
                children_geometries
                    .iter()
                    .map(|(is_sel, g)| (*is_sel, (g.origin.x, g.origin.x + g.size.width))),
            ) {
                let hit_rect = LogicalRect::new(
                    LogicalPoint::new(hit_zone.start, geometry.origin.y),
                    LogicalSize::new(
                        hit_zone.end - hit_zone.start,
                        geometry.origin.y + geometry.size.height,
                    ),
                );
                if hit_rect.contains(position) {
                    return (
                        Some(DropMark {
                            start: LogicalPoint::new(drop_zone.start, geometry.origin.y),
                            end: LogicalPoint::new(
                                drop_zone.end,
                                geometry.origin.y + geometry.size.height,
                            ),
                        }),
                        index,
                    );
                }
            }
            unreachable!("We missed the target layout")
        }
        ui::LayoutKind::Vertical => {
            for (index, hit_zone, drop_zone) in DropZoneIterator::new(
                geometry.origin.y,
                geometry.origin.y + geometry.size.height,
                children_geometries
                    .iter()
                    .map(|(is_sel, g)| (*is_sel, (g.origin.y, g.origin.y + g.size.height))),
            ) {
                let hit_rect = LogicalRect::new(
                    LogicalPoint::new(geometry.origin.x, hit_zone.start),
                    LogicalSize::new(
                        geometry.origin.x + geometry.size.width,
                        hit_zone.end - hit_zone.start,
                    ),
                );
                if hit_rect.contains(position) {
                    return (
                        Some(DropMark {
                            start: LogicalPoint::new(geometry.origin.x, drop_zone.start),
                            end: LogicalPoint::new(
                                geometry.origin.x + geometry.size.width,
                                drop_zone.end,
                            ),
                        }),
                        index,
                    );
                }
            }
            unreachable!("We missed the target layout")
        }
        ui::LayoutKind::Grid => {
            // TODO: Do something here
            (None, usize::MAX)
        }
    }
}

fn accept_drop_at(
    element_node: &common::ElementRcNode,
    component_instance: &ComponentInstance,
    position: LogicalPoint,
) -> DropAccept {
    let layout_kind = element_node.layout_kind();
    let Some(geometry) = element_node.geometry_at(component_instance, position) else {
        return DropAccept::No;
    };
    calculate_drop_acceptance(&geometry, position, &layout_kind)
}

#[derive(Clone, Debug)]
pub struct DropMark {
    pub start: i_slint_core::lengths::LogicalPoint,
    pub end: i_slint_core::lengths::LogicalPoint,
}

fn insert_position_at_end(
    target_element_node: &common::ElementRcNode,
) -> Option<InsertInformation> {
    target_element_node.with_element_node(|node| {
        let closing_brace = crate::util::last_non_ws_token(node)?;
        let closing_brace_offset = Into::<u32>::into(closing_brace.text_range().start());

        let before_closing = closing_brace.prev_token()?;

        let (pre_indent, indent, post_indent, offset, replacement_range) = if before_closing.kind()
            == SyntaxKind::Whitespace
            && before_closing.text().contains('\n')
        {
            let bracket_indent = before_closing.text().split('\n').last().unwrap(); // must exist in this branch
            (
                "    ".to_string(),
                format!("{bracket_indent}    "),
                bracket_indent.to_string(),
                closing_brace_offset,
                0,
            )
        } else if before_closing.kind() == SyntaxKind::Whitespace
            && !before_closing.text().contains('\n')
        {
            let indent = util::find_element_indent(target_element_node).unwrap_or_default();
            let ws_len = before_closing.text().len() as u32;
            (
                format!("\n{indent}    "),
                format!("{indent}    "),
                indent,
                closing_brace_offset - ws_len,
                ws_len,
            )
        } else {
            let indent = util::find_element_indent(target_element_node).unwrap_or_default();
            (format!("\n{indent}    "), format!("{indent}    "), indent, closing_brace_offset, 0)
        };

        let url = lsp_types::Url::from_file_path(node.source_file.path()).ok()?;
        let (version, _) = preview::get_url_from_cache(&url)?;

        Some(InsertInformation {
            insertion_position: common::VersionedPosition::new(
                crate::common::VersionedUrl::new(url, version),
                offset,
            ),
            replacement_range,
            pre_indent,
            indent,
            post_indent,
        })
    })
}

fn insert_position_before_child(
    target_element_node: &common::ElementRcNode,
    child_index: usize,
) -> Option<InsertInformation> {
    target_element_node.with_element_node(|node| {
        for (index, child_node) in node
            .children()
            .filter(|n| {
                [
                    SyntaxKind::SubElement,
                    SyntaxKind::RepeatedElement,
                    SyntaxKind::ConditionalElement,
                ]
                .contains(&n.kind())
            })
            .enumerate()
        {
            if index < child_index {
                continue;
            }

            assert!(index == child_index);

            let first_token = child_node.first_token()?;
            let first_token_offset = u32::from(first_token.text_range().start());
            let before_first_token = first_token.prev_token()?;

            let (pre_indent, indent) = if before_first_token.kind() == SyntaxKind::Whitespace
                && before_first_token.text().contains('\n')
            {
                let element_indent = before_first_token.text().split('\n').last().unwrap(); // must exist in this branch
                ("".to_string(), element_indent.to_string())
            } else if before_first_token.kind() == SyntaxKind::Whitespace
                && !before_first_token.text().contains('\n')
            {
                let indent = util::find_element_indent(target_element_node).unwrap_or_default();
                ("".to_string(), format!("{indent}    "))
            } else {
                let indent = util::find_element_indent(target_element_node).unwrap_or_default();
                (format!("\n{indent}    "), format!("{indent}    "))
            };

            let url = lsp_types::Url::from_file_path(child_node.source_file.path()).ok()?;
            let (version, _) = preview::get_url_from_cache(&url)?;

            return Some(InsertInformation {
                insertion_position: common::VersionedPosition::new(
                    crate::common::VersionedUrl::new(url, version),
                    first_token_offset,
                ),
                replacement_range: 0,
                pre_indent,
                indent: indent.clone(),
                post_indent: indent,
            });
        }

        // We should never get here...
        None
    })
}

// find all elements covering the given `position`.
fn drop_target_element_nodes(
    component_instance: &ComponentInstance,
    position: LogicalPoint,
    filter: Box<dyn Fn(&common::ElementRcNode) -> bool>,
) -> Vec<common::ElementRcNode> {
    let mut result = Vec::with_capacity(3);

    for sc in &element_selection::collect_all_element_nodes_covering(position, component_instance) {
        let Some(en) = sc.as_element_node() else {
            continue;
        };

        if en.with_element_node(preview::is_element_node_ignored) {
            continue;
        }

        if (filter)(&en) {
            continue;
        }

        result.push(en);
    }

    result
}

fn is_recursive_inclusion(
    root_node: &Option<&common::ElementRcNode>,
    component_type: &str,
) -> bool {
    let declared_identifier = root_node
        .and_then(|rn| {
            rn.with_element_node(|node| {
                node.parent()
                    .map(|p| Into::<syntax_nodes::Component>::into(p))
                    .map(|c| c.DeclaredIdentifier().text().to_string())
            })
        })
        .unwrap_or_default();

    declared_identifier == component_type
}

fn find_element_to_drop_into(
    component_instance: &ComponentInstance,
    position: LogicalPoint,
    filter: Box<dyn Fn(&common::ElementRcNode) -> bool>,
    component_type: &str,
) -> Option<common::ElementRcNode> {
    let all_element_nodes = drop_target_element_nodes(component_instance, position, filter);
    if is_recursive_inclusion(&all_element_nodes.last(), component_type) {
        return None;
    }

    let mut tmp = None;
    for element_node in &all_element_nodes {
        let drop_at = accept_drop_at(element_node, component_instance, position);
        match drop_at {
            DropAccept::Yes => {
                return Some(element_node.clone());
            }
            DropAccept::Maybe => {
                tmp = tmp.or(Some(element_node.clone()));
            }
            DropAccept::No => unreachable!("All elements intersect with position"),
        }
    }

    tmp
}

fn find_drop_location(
    component_instance: &ComponentInstance,
    position: LogicalPoint,
    component_type: &str,
) -> Option<DropInformation> {
    let root_node_path = element_selection::root_element(component_instance)
        .borrow()
        .debug
        .first()
        .map(|(n, _)| n.source_file.path().to_owned());
    let filter = Box::new(move |e: &common::ElementRcNode| {
        e.with_element_node(|n| Some(n.source_file.path()) != root_node_path.as_deref())
    });
    let mark = Box::new(move |_: &common::ElementRcNode| false);
    find_filtered_location(component_instance, position, filter, mark, component_type)
}

fn find_move_location(
    component_instance: &ComponentInstance,
    position: LogicalPoint,
    selected_element: common::ElementRcNode,
    component_type: &str,
) -> Option<DropInformation> {
    let se = selected_element.clone();
    let filter =
        Box::new(move |e: &common::ElementRcNode| *e == se && !e.is_same_component_as(&se));
    let se = selected_element.clone();
    let mark = Box::new(move |e: &common::ElementRcNode| *e == se);

    find_filtered_location(component_instance, position, filter, mark, component_type)
}

fn find_filtered_location(
    component_instance: &ComponentInstance,
    position: LogicalPoint,
    filter: Box<dyn Fn(&common::ElementRcNode) -> bool>,
    mark: Box<dyn Fn(&common::ElementRcNode) -> bool>,
    component_type: &str,
) -> Option<DropInformation> {
    let drop_target_node =
        find_element_to_drop_into(component_instance, position, filter, component_type)?;

    let (path, _) = drop_target_node.path_and_offset();
    let tl = component_instance.definition().type_loader();
    let doc = tl.get_document(&path)?;
    if let Some(element_type) = drop_target_node.with_element_node(|node| {
        util::lookup_current_element_type((node.clone()).into(), &doc.local_registry)
    }) {
        if drop_target_node.layout_kind() == ui::LayoutKind::None
            && element_type.accepts_child_element(component_type, &doc.local_registry).is_err()
        {
            return None;
        }
    }

    let layout_kind = drop_target_node.layout_kind();
    if layout_kind != ui::LayoutKind::None {
        let geometry = drop_target_node.geometry_at(component_instance, position)?;
        let children_geometries: Vec<_> = drop_target_node
            .children()
            .iter()
            .filter(|c| !c.with_element_node(preview::is_element_node_ignored))
            .filter_map(|c| c.geometry_in(component_instance, &geometry).map(|g| ((mark)(c), g)))
            .collect();

        let (drop_mark, child_index) = calculate_drop_information_for_layout(
            &geometry,
            position,
            &layout_kind,
            &children_geometries,
        );

        let insert_info = {
            if child_index == usize::MAX {
                insert_position_at_end(&drop_target_node)
            } else {
                insert_position_before_child(&drop_target_node, child_index)
            }
        }?;

        Some(DropInformation {
            target_element_node: drop_target_node,
            insert_info,
            drop_mark,
            child_index,
        })
    } else {
        let insert_info = insert_position_at_end(&drop_target_node)?;
        Some(DropInformation {
            target_element_node: drop_target_node,
            insert_info,
            drop_mark: None,
            child_index: usize::MAX,
        })
    }
}

/// Find the Element to insert into. None means we can not insert at this point.
pub fn can_drop_at(position: LogicalPoint, component_type: &str) -> bool {
    let dm = &super::component_instance()
        .and_then(|ci| find_drop_location(&ci, position, component_type));

    preview::set_drop_mark(&dm.as_ref().and_then(|dm| dm.drop_mark.clone()));
    dm.is_some()
}

/// Find the Element to insert into. None means we can not insert at this point.
pub fn can_move_to(mouse_position: LogicalPoint, element_node: common::ElementRcNode) -> bool {
    let component_type = element_node.component_type();
    let dm = &super::component_instance()
        .and_then(|ci| find_move_location(&ci, mouse_position, element_node, &component_type));

    preview::set_drop_mark(&dm.as_ref().and_then(|dm| dm.drop_mark.clone()));
    dm.is_some()
}

/// Extra data on an added Element, relevant to the Preview side only.
#[derive(Clone, Debug)]
pub struct DropData {
    /// The offset to select next. This is different from the insert position
    /// due to indentation, etc.
    pub selection_offset: u32,
    pub path: std::path::PathBuf,
}

fn pretty_node_removal_range(node: &SyntaxNode) -> Option<rowan::TextRange> {
    let first_et = node.first_token()?;
    let before_et = first_et.prev_token()?;
    let start_pos = if before_et.kind() == SyntaxKind::Whitespace && before_et.text().contains('\n')
    {
        before_et.text_range().end()
            - rowan::TextSize::from(
                before_et.text().split('\n').last().map(|s| s.len()).unwrap_or_default() as u32,
            )
    } else if before_et.kind() == SyntaxKind::Whitespace {
        before_et.text_range().start() // Cut away all WS!
    } else {
        first_et.text_range().start() // Nothing to cut away
    };

    let last_et = util::last_non_ws_token(node)?;
    let after_et = last_et.next_token()?;
    let end_pos = if after_et.kind() == SyntaxKind::Whitespace && after_et.text().contains('\n') {
        after_et.text_range().start()
            + rowan::TextSize::from(
                after_et.text().split('\n').next().map(|s| s.len() + 1).unwrap_or_default() as u32,
            )
    } else {
        last_et.text_range().end() // Use existing WS or not WS as appropriate
    };

    Some(rowan::TextRange::new(start_pos, end_pos))
}

fn drop_ignored_elements_from_node(
    node: &common::ElementRcNode,
    source_file: &SourceFile,
) -> Vec<lsp_types::TextEdit> {
    node.with_element_node(|node| {
        node.children()
            .filter_map(|c| {
                let e = common::extract_element(c.clone())?;
                if preview::is_element_node_ignored(&e) {
                    pretty_node_removal_range(&e)
                        .map(|range| util::map_range(source_file, range))
                        .map(|range| lsp_types::TextEdit::new(range, String::new()))
                } else {
                    None
                }
            })
            .collect()
    })
}

/// Find a location in a file that would be a good place to insert the new component at
///
/// Return a WorkspaceEdit to send to the editor and extra info for the live preview in
/// the DropData struct.
pub fn drop_at(
    position: LogicalPoint,
    component: &common::ComponentInformation,
) -> Option<(lsp_types::WorkspaceEdit, DropData)> {
    let component_type = &component.name;
    let component_instance = preview::component_instance()?;
    let tl = component_instance.definition().type_loader();
    let drop_info = find_drop_location(&component_instance, position, component_type)?;

    let properties = {
        let mut props = component.default_properties.clone();

        if drop_info.target_element_node.layout_kind() == ui::LayoutKind::None
            && !component.fills_parent
        {
            if let Some(area) =
                drop_info.target_element_node.geometry_at(&component_instance, position)
            {
                props.push(common::PropertyChange::new(
                    "x",
                    format!("{}px", position.x - area.origin.x),
                ));
                props.push(common::PropertyChange::new(
                    "y",
                    format!("{}px", position.y - area.origin.y),
                ));
            }
        }

        props
    };
    let placeholder = if component.is_layout { placeholder() } else { String::new() };

    let new_text = if properties.is_empty() {
        format!(
            "{}{} {{{placeholder} }}\n{}",
            drop_info.insert_info.pre_indent, component_type, drop_info.insert_info.post_indent
        )
    } else {
        let mut to_insert =
            format!("{}{} {{{placeholder}\n", drop_info.insert_info.pre_indent, component_type);
        for p in &properties {
            to_insert += &format!("{}    {}: {};\n", drop_info.insert_info.indent, p.name, p.value);
        }
        to_insert +=
            &format!("{}}}\n{}", drop_info.insert_info.indent, drop_info.insert_info.post_indent);
        to_insert
    };

    let mut selection_offset = drop_info.insert_info.insertion_position.offset()
        + new_text.chars().take_while(|c| c.is_whitespace()).map(|c| c.len_utf8()).sum::<usize>()
            as u32;

    let (path, _) = drop_info.target_element_node.path_and_offset();

    let doc = tl.get_document(&path)?;
    let source_file = doc.node.as_ref().unwrap().source_file.clone();

    let mut edits = Vec::with_capacity(3);
    let import_file = component.import_file_name(&lsp_types::Url::from_file_path(&path).ok());
    if let Some(edit) = completion::create_import_edit(doc, component_type, &import_file) {
        if let Some(sf) = doc.node.as_ref().map(|n| &n.source_file) {
            selection_offset = TextOffsetAdjustment::new(&edit, sf).adjust(selection_offset);
        }
        edits.push(edit);
    }

    edits.extend(
        drop_ignored_elements_from_node(&drop_info.target_element_node, &source_file)
            .drain(..)
            .map(|te| {
                // Abuse map somewhat...
                selection_offset =
                    TextOffsetAdjustment::new(&te, &source_file).adjust(selection_offset);
                te
            }),
    );

    let start_pos =
        util::map_position(&source_file, drop_info.insert_info.insertion_position.offset().into());
    let end_pos = util::map_position(
        &source_file,
        (drop_info.insert_info.insertion_position.offset()
            + drop_info.insert_info.replacement_range)
            .into(),
    );
    edits.push(lsp_types::TextEdit { range: lsp_types::Range::new(start_pos, end_pos), new_text });

    Some((
        common::create_workspace_edit_from_source_file(&source_file, edits)?,
        DropData { selection_offset, path },
    ))
}

fn property_ranges(
    element: &common::ElementRcNode,
    remove_properties: &[&str],
) -> Vec<rowan::TextRange> {
    element.with_element_node(|node| {
        let mut result = vec![];

        for b in node.Binding() {
            let name = b.first_token().map(|t| t.text().to_string()).unwrap_or_default();
            if remove_properties.contains(&name.as_str()) {
                let Some(r) = pretty_node_removal_range(&b) else {
                    continue;
                };
                result.push(r);
            }
        }

        result
    })
}

fn extract_text_of_element(
    element: &common::ElementRcNode,
    remove_properties: &[&str],
) -> Vec<String> {
    let (start_offset, mut text) = element.with_decorated_node(|node| {
        (usize::from(node.text_range().start()), node.text().to_string())
    });

    let mut to_delete_ranges = property_ranges(element, remove_properties);
    to_delete_ranges.sort_by(|a, b| u32::from(a.start()).cmp(&u32::from(b.start())));
    let mut offset = start_offset;
    for dr in to_delete_ranges {
        let start = usize::from(dr.start()) - offset;
        let end = usize::from(dr.end()) - offset;

        offset += end - start;

        text.drain(start..end);
    }

    // Trim leading WS to get "raw" lines
    let lines = text.split('\n').collect::<Vec<_>>();
    let indent = util::find_element_indent(element).unwrap_or_else(|| {
        lines
            .last()
            .expect("There is always one line")
            .chars()
            .take_while(|c| c.is_whitespace())
            .collect()
    });
    let lines = lines
        .iter()
        .map(|l| if l.starts_with(&indent) { l[indent.len()..].to_string() } else { l.to_string() })
        .collect::<Vec<_>>();

    lines
}

fn node_removal_text_edit(
    node: &SyntaxNode,
    replace_with: String,
) -> Option<(SourceFile, lsp_types::TextEdit)> {
    let source_file = node.source_file.clone();
    let range = util::map_range(&source_file, pretty_node_removal_range(node)?);
    Some((source_file, lsp_types::TextEdit::new(range, replace_with)))
}

/// Find a location in a file that would be a good place to insert the new component at
///
/// Return a WorkspaceEdit to send to the editor and extra info for the live preview in
/// the DropData struct.
pub fn move_element_to(
    element: common::ElementRcNode,
    position: LogicalPoint,
    mouse_position: LogicalPoint,
) -> Option<(lsp_types::WorkspaceEdit, DropData)> {
    let component_type = element.component_type();
    let component_instance = preview::component_instance()?;
    let tl = component_instance.definition().type_loader();
    let Some(drop_info) =
        find_move_location(&component_instance, mouse_position, element.clone(), &component_type)
    else {
        element_selection::reselect_element();
        // Can not drop here: Ignore the move
        return None;
    };

    let parent_of_element = element.parent();

    let placeholder_text = if Some(&drop_info.target_element_node) == parent_of_element.as_ref() {
        // We are moving within ourselves!

        let size = element.geometries(&component_instance).first().map(|g| g.size)?;

        if drop_info.target_element_node.layout_kind() == ui::LayoutKind::None {
            preview::resize_selected_element_impl(LogicalRect::new(position, size));
            return None;
        } else {
            let children = drop_info.target_element_node.children();
            let child_index = {
                let tmp =
                    children.iter().position(|c| c == &element).expect("We have the same parent");
                if tmp == children.len() {
                    usize::MAX
                } else {
                    tmp
                }
            };

            if child_index == drop_info.child_index {
                element_selection::reselect_element();
                // Dropped onto myself: Ignore the move
                return None;
            }
        }

        /* fall trough to the general case here */
        String::new()
    } else if parent_of_element.map(|p| p.children().len()).unwrap_or_default() == 1 {
        placeholder()
    } else {
        String::new()
    };

    let new_text = {
        let element_text_lines = extract_text_of_element(&element, &["x", "y"]);

        if element_text_lines.is_empty() {
            String::new()
        } else {
            let mut tmp = format!(
                "{}{}\n",
                drop_info.insert_info.pre_indent,
                element_text_lines.first().expect("Not empty")
            );

            for l in element_text_lines.iter().take(element_text_lines.len() - 1).skip(1) {
                tmp.push_str(&format!("{}{l}\n", drop_info.insert_info.indent));
            }

            if element_text_lines.len() >= 2 {
                tmp.push_str(&format!(
                    "{}{}\n{}",
                    drop_info.insert_info.indent,
                    element_text_lines.last().expect("Length was checked"),
                    drop_info.insert_info.post_indent
                ));
            }

            tmp
        }
    };

    let (path, _) = drop_info.target_element_node.path_and_offset();

    let doc = tl.get_document(&path)?;
    let source_file = doc.node.as_ref().unwrap().source_file.clone();

    let mut selection_offset = drop_info.insert_info.insertion_position.offset()
        + new_text.chars().take_while(|c| c.is_whitespace()).map(|c| c.len_utf8()).sum::<usize>()
            as u32;

    let mut edits = Vec::with_capacity(3);

    let remove_me = element
        .with_decorated_node(|node| node_removal_text_edit(&node, placeholder_text.clone()))?;
    if remove_me.0.path() == source_file.path() {
        selection_offset =
            TextOffsetAdjustment::new(&remove_me.1, &source_file).adjust(selection_offset);
    }
    edits.push(remove_me);

    if let Some(component_info) = preview::get_component_info(&component_type) {
        let import_file =
            component_info.import_file_name(&lsp_types::Url::from_file_path(&path).ok());
        if let Some(edit) = completion::create_import_edit(doc, &component_type, &import_file) {
            if let Some(sf) = doc.node.as_ref().map(|n| &n.source_file) {
                selection_offset = TextOffsetAdjustment::new(&edit, sf).adjust(selection_offset);
            }
            edits.push((source_file.clone(), edit));
        }
    }

    edits.extend(
        drop_ignored_elements_from_node(&drop_info.target_element_node, &source_file)
            .drain(..)
            .map(|te| {
                // Abuse map somewhat...
                selection_offset =
                    TextOffsetAdjustment::new(&te, &source_file).adjust(selection_offset);
                (source_file.clone(), te)
            }),
    );

    let start_pos =
        util::map_position(&source_file, drop_info.insert_info.insertion_position.offset().into());
    let end_pos = util::map_position(
        &source_file,
        (drop_info.insert_info.insertion_position.offset()
            + drop_info.insert_info.replacement_range)
            .into(),
    );
    edits.push((
        source_file,
        lsp_types::TextEdit { range: lsp_types::Range::new(start_pos, end_pos), new_text },
    ));

    Some((
        common::create_workspace_edit_from_source_files(edits)?,
        DropData { selection_offset, path },
    ))
}

#[test]
fn test_text_offset_adjustments() {
    let mut a = TextOffsetAdjustments::default();
    // same length change
    a.add_adjustment(TextOffsetAdjustment {
        start_offset: 10,
        end_offset: 20,
        new_text_length: 10,
    });
    // insert
    a.add_adjustment(TextOffsetAdjustment { start_offset: 25, end_offset: 25, new_text_length: 1 });
    // smaller replacment
    a.add_adjustment(TextOffsetAdjustment { start_offset: 30, end_offset: 40, new_text_length: 5 });
    // longer replacement
    a.add_adjustment(TextOffsetAdjustment {
        start_offset: 50,
        end_offset: 60,
        new_text_length: 20,
    });
    // deletion
    a.add_adjustment(TextOffsetAdjustment { start_offset: 70, end_offset: 80, new_text_length: 0 });

    assert_eq!(a.adjust(0), 0);
    assert_eq!(a.adjust(20), 20);
    assert_eq!(a.adjust(25), 26);
    assert_eq!(a.adjust(30), 31);
    assert_eq!(a.adjust(40), 36);
    assert_eq!(a.adjust(60), 66);
    assert_eq!(a.adjust(70), 76);
    assert_eq!(a.adjust(80), 76);
}

#[test]
fn test_text_offset_adjustments_reverse() {
    let mut a = TextOffsetAdjustments::default();
    // deletion
    a.add_adjustment(TextOffsetAdjustment { start_offset: 70, end_offset: 80, new_text_length: 0 });
    // longer replacement
    a.add_adjustment(TextOffsetAdjustment {
        start_offset: 50,
        end_offset: 60,
        new_text_length: 20,
    });
    // smaller replacment
    a.add_adjustment(TextOffsetAdjustment { start_offset: 30, end_offset: 40, new_text_length: 5 });
    // insert
    a.add_adjustment(TextOffsetAdjustment { start_offset: 25, end_offset: 25, new_text_length: 1 });
    // same length change
    a.add_adjustment(TextOffsetAdjustment {
        start_offset: 10,
        end_offset: 20,
        new_text_length: 10,
    });

    assert_eq!(a.adjust(0), 0);
    assert_eq!(a.adjust(20), 20);
    assert_eq!(a.adjust(25), 26);
    assert_eq!(a.adjust(30), 31);
    assert_eq!(a.adjust(40), 36);
    assert_eq!(a.adjust(60), 66);
    assert_eq!(a.adjust(70), 76);
    assert_eq!(a.adjust(80), 76);
}

#[test]
fn test_edit_iterator_empty() {
    let workspace_edit = lsp_types::WorkspaceEdit {
        changes: None,
        document_changes: None,
        change_annotations: None,
    };

    let mut it = EditIterator::new(&workspace_edit);
    assert!(it.next().is_none());
    assert!(it.next().is_none());
}

#[test]
fn test_edit_iterator_changes_one_empty() {
    let workspace_edit = lsp_types::WorkspaceEdit {
        changes: Some(std::collections::HashMap::from([(
            lsp_types::Url::parse("file://foo/bar.slint").unwrap(),
            vec![],
        )])),
        document_changes: None,
        change_annotations: None,
    };

    let mut it = EditIterator::new(&workspace_edit);
    assert!(it.next().is_none());
    assert!(it.next().is_none());
}

#[test]
fn test_edit_iterator_changes_one_one() {
    let workspace_edit = lsp_types::WorkspaceEdit {
        changes: Some(std::collections::HashMap::from([(
            lsp_types::Url::parse("file://foo/bar.slint").unwrap(),
            vec![lsp_types::TextEdit {
                range: lsp_types::Range::new(
                    lsp_types::Position::new(23, 42),
                    lsp_types::Position::new(42, 23),
                ),
                new_text: "Replacement".to_string(),
            }],
        )])),
        document_changes: None,
        change_annotations: None,
    };

    let mut it = EditIterator::new(&workspace_edit);
    let r = it.next().unwrap();
    assert_eq!(&r.0.uri.to_string(), "file://foo/bar.slint");
    assert_eq!(r.0.version, None);
    assert_eq!(&r.1.new_text, "Replacement");
    assert_eq!(&r.1.range.start, &lsp_types::Position::new(23, 42));
    assert_eq!(&r.1.range.end, &lsp_types::Position::new(42, 23));
    assert!(it.next().is_none());
    assert!(it.next().is_none());
}

#[test]
fn test_edit_iterator_changes_one_two() {
    let workspace_edit = lsp_types::WorkspaceEdit {
        changes: Some(std::collections::HashMap::from([(
            lsp_types::Url::parse("file://foo/bar.slint").unwrap(),
            vec![
                lsp_types::TextEdit {
                    range: lsp_types::Range::new(
                        lsp_types::Position::new(23, 42),
                        lsp_types::Position::new(42, 23),
                    ),
                    new_text: "Replacement".to_string(),
                },
                lsp_types::TextEdit {
                    range: lsp_types::Range::new(
                        lsp_types::Position::new(44, 12),
                        lsp_types::Position::new(44, 13),
                    ),
                    new_text: "Foo".to_string(),
                },
            ],
        )])),
        document_changes: None,
        change_annotations: None,
    };

    let mut it = EditIterator::new(&workspace_edit);

    let r = it.next().unwrap();
    assert_eq!(&r.0.uri.to_string(), "file://foo/bar.slint");
    assert_eq!(r.0.version, None);
    assert_eq!(&r.1.new_text, "Replacement");
    assert_eq!(&r.1.range.start, &lsp_types::Position::new(23, 42));
    assert_eq!(&r.1.range.end, &lsp_types::Position::new(42, 23));

    let r = it.next().unwrap();
    assert_eq!(&r.0.uri.to_string(), "file://foo/bar.slint");
    assert_eq!(r.0.version, None);
    assert_eq!(&r.1.new_text, "Foo");
    assert_eq!(&r.1.range.start, &lsp_types::Position::new(44, 12));
    assert_eq!(&r.1.range.end, &lsp_types::Position::new(44, 13));

    assert!(it.next().is_none());
}

#[test]
fn test_edit_iterator_changes_two() {
    let workspace_edit = lsp_types::WorkspaceEdit {
        changes: Some(std::collections::HashMap::from([
            (
                lsp_types::Url::parse("file://foo/bar.slint").unwrap(),
                vec![lsp_types::TextEdit {
                    range: lsp_types::Range::new(
                        lsp_types::Position::new(23, 42),
                        lsp_types::Position::new(42, 23),
                    ),
                    new_text: "Replacement".to_string(),
                }],
            ),
            (
                lsp_types::Url::parse("file://foo/baz.slint").unwrap(),
                vec![lsp_types::TextEdit {
                    range: lsp_types::Range::new(
                        lsp_types::Position::new(44, 12),
                        lsp_types::Position::new(44, 13),
                    ),
                    new_text: "Foo".to_string(),
                }],
            ),
        ])),
        document_changes: None,
        change_annotations: None,
    };

    let mut seen1 = false;
    let mut seen2 = false;

    for r in EditIterator::new(&workspace_edit) {
        // random order!
        if r.0.uri.to_string() == "file://foo/bar.slint" {
            assert_eq!(seen1, false);
            assert_eq!(&r.0.uri.to_string(), "file://foo/bar.slint");
            assert_eq!(r.0.version, None);
            assert_eq!(&r.1.new_text, "Replacement");
            assert_eq!(&r.1.range.start, &lsp_types::Position::new(23, 42));
            assert_eq!(&r.1.range.end, &lsp_types::Position::new(42, 23));
            seen1 = true;
        } else {
            assert_eq!(seen2, false);
            assert_eq!(&r.0.uri.to_string(), "file://foo/baz.slint");
            assert_eq!(r.0.version, None);
            assert_eq!(&r.1.new_text, "Foo");
            assert_eq!(&r.1.range.start, &lsp_types::Position::new(44, 12));
            assert_eq!(&r.1.range.end, &lsp_types::Position::new(44, 13));
            seen2 = true;
        }
    }
    assert!(seen1 && seen2);
}

#[test]
fn test_edit_iterator_document_changes_empty() {
    let workspace_edit = lsp_types::WorkspaceEdit {
        changes: None,
        document_changes: Some(lsp_types::DocumentChanges::Edits(vec![])),
        change_annotations: None,
    };

    let mut it = EditIterator::new(&workspace_edit);
    assert!(it.next().is_none());
    assert!(it.next().is_none());
}

#[test]
fn test_edit_iterator_document_changes_operations() {
    let workspace_edit = lsp_types::WorkspaceEdit {
        changes: None,
        document_changes: Some(lsp_types::DocumentChanges::Operations(vec![])),
        change_annotations: None,
    };

    let mut it = EditIterator::new(&workspace_edit);
    assert!(it.next().is_none());
    assert!(it.next().is_none());
}

#[test]
fn test_edit_iterator_document_changes_one_empty() {
    let workspace_edit = lsp_types::WorkspaceEdit {
        changes: None,
        document_changes: Some(lsp_types::DocumentChanges::Edits(vec![
            lsp_types::TextDocumentEdit {
                edits: vec![],
                text_document: lsp_types::OptionalVersionedTextDocumentIdentifier {
                    uri: lsp_types::Url::parse("file://foo/bar.slint").unwrap(),
                    version: Some(99),
                },
            },
        ])),
        change_annotations: None,
    };

    let mut it = EditIterator::new(&workspace_edit);
    assert!(it.next().is_none());
    assert!(it.next().is_none());
}

#[test]
fn test_edit_iterator_document_changes_one_one() {
    let workspace_edit = lsp_types::WorkspaceEdit {
        changes: None,
        document_changes: Some(lsp_types::DocumentChanges::Edits(vec![
            lsp_types::TextDocumentEdit {
                edits: vec![lsp_types::OneOf::Left(lsp_types::TextEdit {
                    range: lsp_types::Range::new(
                        lsp_types::Position::new(23, 42),
                        lsp_types::Position::new(42, 23),
                    ),
                    new_text: "Replacement".to_string(),
                })],
                text_document: lsp_types::OptionalVersionedTextDocumentIdentifier {
                    uri: lsp_types::Url::parse("file://foo/bar.slint").unwrap(),
                    version: Some(99),
                },
            },
        ])),
        change_annotations: None,
    };

    let mut it = EditIterator::new(&workspace_edit);
    let r = it.next().unwrap();
    assert_eq!(&r.0.uri.to_string(), "file://foo/bar.slint");
    assert_eq!(r.0.version, Some(99));
    assert_eq!(&r.1.new_text, "Replacement");
    assert_eq!(&r.1.range.start, &lsp_types::Position::new(23, 42));
    assert_eq!(&r.1.range.end, &lsp_types::Position::new(42, 23));
    assert!(it.next().is_none());
    assert!(it.next().is_none());
}

#[test]
fn test_edit_iterator_document_changes_one_two() {
    let workspace_edit = lsp_types::WorkspaceEdit {
        changes: None,
        document_changes: Some(lsp_types::DocumentChanges::Edits(vec![
            lsp_types::TextDocumentEdit {
                edits: vec![
                    lsp_types::OneOf::Left(lsp_types::TextEdit {
                        range: lsp_types::Range::new(
                            lsp_types::Position::new(23, 42),
                            lsp_types::Position::new(42, 23),
                        ),
                        new_text: "Replacement".to_string(),
                    }),
                    lsp_types::OneOf::Right(lsp_types::AnnotatedTextEdit {
                        text_edit: lsp_types::TextEdit {
                            range: lsp_types::Range::new(
                                lsp_types::Position::new(44, 12),
                                lsp_types::Position::new(44, 13),
                            ),
                            new_text: "Foo".to_string(),
                        },
                        annotation_id: "CID".to_string(),
                    }),
                ],
                text_document: lsp_types::OptionalVersionedTextDocumentIdentifier {
                    uri: lsp_types::Url::parse("file://foo/bar.slint").unwrap(),
                    version: Some(99),
                },
            },
        ])),
        change_annotations: None,
    };

    let mut it = EditIterator::new(&workspace_edit);
    let r = it.next().unwrap();
    assert_eq!(&r.0.uri.to_string(), "file://foo/bar.slint");
    assert_eq!(r.0.version, Some(99));
    assert_eq!(&r.1.new_text, "Replacement");
    assert_eq!(&r.1.range.start, &lsp_types::Position::new(23, 42));
    assert_eq!(&r.1.range.end, &lsp_types::Position::new(42, 23));

    let r = it.next().unwrap();
    assert_eq!(&r.0.uri.to_string(), "file://foo/bar.slint");
    assert_eq!(r.0.version, Some(99));
    assert_eq!(&r.1.new_text, "Foo");
    assert_eq!(&r.1.range.start, &lsp_types::Position::new(44, 12));
    assert_eq!(&r.1.range.end, &lsp_types::Position::new(44, 13));
    assert!(it.next().is_none());
    assert!(it.next().is_none());
}

#[test]
fn test_edit_iterator_document_changes_two() {
    let workspace_edit = lsp_types::WorkspaceEdit {
        changes: None,
        document_changes: Some(lsp_types::DocumentChanges::Edits(vec![
            lsp_types::TextDocumentEdit {
                edits: vec![lsp_types::OneOf::Left(lsp_types::TextEdit {
                    range: lsp_types::Range::new(
                        lsp_types::Position::new(23, 42),
                        lsp_types::Position::new(42, 23),
                    ),
                    new_text: "Replacement".to_string(),
                })],
                text_document: lsp_types::OptionalVersionedTextDocumentIdentifier {
                    uri: lsp_types::Url::parse("file://foo/bar.slint").unwrap(),
                    version: Some(99),
                },
            },
            lsp_types::TextDocumentEdit {
                edits: vec![lsp_types::OneOf::Right(lsp_types::AnnotatedTextEdit {
                    text_edit: lsp_types::TextEdit {
                        range: lsp_types::Range::new(
                            lsp_types::Position::new(44, 12),
                            lsp_types::Position::new(44, 13),
                        ),
                        new_text: "Foo".to_string(),
                    },
                    annotation_id: "CID".to_string(),
                })],
                text_document: lsp_types::OptionalVersionedTextDocumentIdentifier {
                    uri: lsp_types::Url::parse("file://foo/baz.slint").unwrap(),
                    version: Some(98),
                },
            },
        ])),
        change_annotations: None,
    };

    let mut it = EditIterator::new(&workspace_edit);
    let r = it.next().unwrap();
    assert_eq!(&r.0.uri.to_string(), "file://foo/bar.slint");
    assert_eq!(r.0.version, Some(99));
    assert_eq!(&r.1.new_text, "Replacement");
    assert_eq!(&r.1.range.start, &lsp_types::Position::new(23, 42));
    assert_eq!(&r.1.range.end, &lsp_types::Position::new(42, 23));

    let r = it.next().unwrap();
    assert_eq!(&r.0.uri.to_string(), "file://foo/baz.slint");
    assert_eq!(r.0.version, Some(98));
    assert_eq!(&r.1.new_text, "Foo");
    assert_eq!(&r.1.range.start, &lsp_types::Position::new(44, 12));
    assert_eq!(&r.1.range.end, &lsp_types::Position::new(44, 13));
    assert!(it.next().is_none());
    assert!(it.next().is_none());
}

#[test]
fn test_edit_iterator_document_mixed() {
    let workspace_edit = lsp_types::WorkspaceEdit {
        changes: Some(std::collections::HashMap::from([
            (
                lsp_types::Url::parse("file://foo/bar.slint").unwrap(),
                vec![lsp_types::TextEdit {
                    range: lsp_types::Range::new(
                        lsp_types::Position::new(23, 42),
                        lsp_types::Position::new(42, 23),
                    ),
                    new_text: "Replacement".to_string(),
                }],
            ),
            (
                lsp_types::Url::parse("file://foo/baz.slint").unwrap(),
                vec![lsp_types::TextEdit {
                    range: lsp_types::Range::new(
                        lsp_types::Position::new(44, 12),
                        lsp_types::Position::new(44, 13),
                    ),
                    new_text: "Foo".to_string(),
                }],
            ),
        ])),
        document_changes: Some(lsp_types::DocumentChanges::Edits(vec![
            lsp_types::TextDocumentEdit {
                edits: vec![lsp_types::OneOf::Left(lsp_types::TextEdit {
                    range: lsp_types::Range::new(
                        lsp_types::Position::new(23, 42),
                        lsp_types::Position::new(42, 23),
                    ),
                    new_text: "Doc Replacement".to_string(),
                })],
                text_document: lsp_types::OptionalVersionedTextDocumentIdentifier {
                    uri: lsp_types::Url::parse("file://doc/bar.slint").unwrap(),
                    version: Some(99),
                },
            },
            lsp_types::TextDocumentEdit {
                edits: vec![lsp_types::OneOf::Right(lsp_types::AnnotatedTextEdit {
                    text_edit: lsp_types::TextEdit {
                        range: lsp_types::Range::new(
                            lsp_types::Position::new(44, 12),
                            lsp_types::Position::new(44, 13),
                        ),
                        new_text: "Doc Foo".to_string(),
                    },
                    annotation_id: "CID".to_string(),
                })],
                text_document: lsp_types::OptionalVersionedTextDocumentIdentifier {
                    uri: lsp_types::Url::parse("file://doc/baz.slint").unwrap(),
                    version: Some(98),
                },
            },
        ])),
        change_annotations: None,
    };

    let mut seen = [false; 4];

    for r in EditIterator::new(&workspace_edit) {
        // random order!
        if r.0.uri.to_string() == "file://foo/bar.slint" {
            assert_eq!(seen[0], false);
            assert_eq!(seen[2], false);
            assert_eq!(seen[3], false);
            assert_eq!(&r.0.uri.to_string(), "file://foo/bar.slint");
            assert_eq!(r.0.version, None);
            assert_eq!(&r.1.new_text, "Replacement");
            assert_eq!(&r.1.range.start, &lsp_types::Position::new(23, 42));
            assert_eq!(&r.1.range.end, &lsp_types::Position::new(42, 23));
            seen[0] = true;
        } else if r.0.uri.to_string() == "file://foo/baz.slint" {
            assert_eq!(seen[1], false);
            assert_eq!(seen[2], false);
            assert_eq!(seen[3], false);
            assert_eq!(&r.0.uri.to_string(), "file://foo/baz.slint");
            assert_eq!(r.0.version, None);
            assert_eq!(&r.1.new_text, "Foo");
            assert_eq!(&r.1.range.start, &lsp_types::Position::new(44, 12));
            assert_eq!(&r.1.range.end, &lsp_types::Position::new(44, 13));
            seen[1] = true;
        } else if r.0.uri.to_string() == "file://doc/bar.slint" {
            assert_eq!(seen[0], true);
            assert_eq!(seen[1], true);
            assert_eq!(seen[2], false);
            assert_eq!(seen[3], false);
            assert_eq!(&r.0.uri.to_string(), "file://doc/bar.slint");
            assert_eq!(r.0.version, Some(99));
            assert_eq!(&r.1.new_text, "Doc Replacement");
            assert_eq!(&r.1.range.start, &lsp_types::Position::new(23, 42));
            assert_eq!(&r.1.range.end, &lsp_types::Position::new(42, 23));
            seen[2] = true;
        } else {
            assert_eq!(seen[0], true);
            assert_eq!(seen[1], true);
            assert_eq!(seen[2], true);
            assert_eq!(seen[3], false);
            assert_eq!(&r.0.uri.to_string(), "file://doc/baz.slint");
            assert_eq!(r.0.version, Some(98));
            assert_eq!(&r.1.new_text, "Doc Foo");
            assert_eq!(&r.1.range.start, &lsp_types::Position::new(44, 12));
            assert_eq!(&r.1.range.end, &lsp_types::Position::new(44, 13));
            seen[3] = true;
        }
    }
}
