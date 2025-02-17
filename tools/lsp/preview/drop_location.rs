// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::cell::RefCell;
use std::num::NonZeroUsize;

use i_slint_compiler::diagnostics::{BuildDiagnostics, SourceFile};
use i_slint_compiler::object_tree;
use i_slint_compiler::parser::{
    syntax_nodes, SyntaxKind, SyntaxNode, SyntaxToken, TextRange, TextSize,
};
use i_slint_core::lengths::{LogicalPoint, LogicalRect, LogicalSize};
use slint_interpreter::ComponentInstance;

use crate::common::{self, text_edit};
use crate::language::completion;
use crate::preview::{self, element_selection, ui};
use crate::util;

use crate::preview::ext::ElementRcNodeExt;

#[cfg(target_arch = "wasm32")]
use crate::wasm_prelude::*;

pub fn placeholder() -> String {
    format!(
        " Rectangle {{ min-width: 16px; min-height: 16px; /* {} */ }}",
        common::NODE_IGNORE_COMMENT
    )
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

impl Iterator for DropZoneIterator<'_> {
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
                    LogicalSize::new(hit_zone.end - hit_zone.start, geometry.size.height),
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
                    LogicalSize::new(geometry.size.width, hit_zone.end - hit_zone.start),
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
        let closing_brace_offset = closing_brace.text_range().start();

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
                closing_brace_offset - TextSize::new(ws_len),
                ws_len,
            )
        } else {
            let indent = util::find_element_indent(target_element_node).unwrap_or_default();
            (format!("\n{indent}    "), format!("{indent}    "), indent, closing_brace_offset, 0)
        };

        let url = lsp_types::Url::from_file_path(node.source_file.path()).ok()?;
        let (version, _) = preview::get_url_from_cache(&url);

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
            let first_token_offset = first_token.text_range().start();
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
            let (version, _) = preview::get_url_from_cache(&url);

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

/// Insert before the first component (exported or not) or at the very end of the document if no
/// Component is found.
fn insert_position_before_first_component(
    document_cache: &common::DocumentCache,
    document: &syntax_nodes::Document,
) -> Option<InsertInformation> {
    let url = {
        let url = lsp_types::Url::from_file_path(document.source_file.path()).ok()?;
        let version = document_cache.document_version_by_path(document.source_file.path());
        common::VersionedUrl::new(url, version)
    };

    let first_component: Option<SyntaxNode> = document.Component().next().map(|c| c.into());
    let first_exported_component: Option<SyntaxNode> =
        document.ExportsList().find(|el| el.Component().is_some()).map(|el| el.into());

    let first_component_node = if first_component
        .as_ref()
        .map(|sn| u32::from(sn.text_range().start()))
        .unwrap_or(u32::MAX)
        < first_exported_component
            .as_ref()
            .map(|sn| u32::from(sn.text_range().start()))
            .unwrap_or(u32::MAX)
    {
        first_component
    } else {
        first_exported_component
    };

    fn find_pre_indent_and_replacement(token: &SyntaxToken) -> (String, u32) {
        match token.kind() {
            SyntaxKind::Whitespace => {
                if token.prev_token().is_some() {
                    let nl_count = token.text().chars().filter(|c| c == &'\n').count();
                    let replacement_range =
                        token.text().split('\n').last().map(|s| s.len()).unwrap_or(0) as u32;

                    if nl_count >= 2 {
                        (String::new(), replacement_range)
                    } else if nl_count == 1 {
                        ("\n".to_string(), replacement_range)
                    } else {
                        ("\n\n".to_string(), replacement_range)
                    }
                } else {
                    (String::new(), token.text().len() as u32) // Just WS before the component: Replace!
                }
            }
            _ => ("\n\n".to_string(), 0),
        }
    }

    if let Some(component) = first_component_node {
        // have a component node!
        let first_token = component.first_token()?;
        let first_token_offset = first_token.text_range().start();
        if let Some(before_first_token) = first_token.prev_token() {
            let (pre_indent, replacement_range) =
                find_pre_indent_and_replacement(&before_first_token);

            Some(InsertInformation {
                insertion_position: common::VersionedPosition::new(
                    url,
                    first_token_offset - TextSize::new(replacement_range),
                ),
                replacement_range,
                pre_indent,
                indent: "    ".to_string(),
                post_indent: "\n\n".to_string(),
            })
        } else {
            // Component is the first thing in the file!
            Some(InsertInformation {
                insertion_position: common::VersionedPosition::new(url, first_token_offset),
                replacement_range: 0,
                pre_indent: String::new(),
                indent: "     ".to_string(),
                post_indent: "\n\n".to_string(),
            })
        }
    } else if let Some(last_token) = document.last_token().unwrap().prev_token() {
        // The last token is EoF, so insert at the end of a non-empty document

        let (pre_indent, replacement_range) = find_pre_indent_and_replacement(&last_token);
        Some(InsertInformation {
            insertion_position: common::VersionedPosition::new(
                url,
                document.text_range().end() - TextSize::new(replacement_range),
            ),
            replacement_range,
            pre_indent,
            indent: "    ".to_string(),
            post_indent: "\n".to_string(),
        })
    } else {
        // Entire document is empty
        Some(InsertInformation {
            insertion_position: common::VersionedPosition::new(url, document.text_range().end()),
            replacement_range: 0,
            pre_indent: String::new(),
            indent: String::new(),
            post_indent: "\n".to_string(),
        })
    }
}

pub fn add_new_component(
    document_cache: &common::DocumentCache,
    component_name: &str,
    document: &syntax_nodes::Document,
) -> Option<(lsp_types::WorkspaceEdit, DropData)> {
    let insert_position = insert_position_before_first_component(document_cache, document)?;
    let new_text = format!(
        "{}component {component_name} {{ }}{}",
        insert_position.pre_indent, insert_position.post_indent
    );

    let selection_offset = insert_position.insertion_position.offset()
        + TextSize::new(
            new_text
                .chars()
                .take_while(|c| c.is_whitespace())
                .map(|c| c.len_utf8() as u32)
                .sum::<u32>()
                + "component ".len() as u32,
        );

    let source_file = document.source_file.clone();
    let path = source_file.path().to_path_buf();

    let start_pos =
        util::text_size_to_lsp_position(&source_file, insert_position.insertion_position.offset());
    let end_pos = util::text_size_to_lsp_position(
        &source_file,
        insert_position.insertion_position.offset()
            + TextSize::new(insert_position.replacement_range),
    );
    let edit = lsp_types::TextEdit { range: lsp_types::Range::new(start_pos, end_pos), new_text };

    Some((
        common::create_workspace_edit_from_path(document_cache, source_file.path(), vec![edit])?,
        DropData { selection_offset, path },
    ))
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

        if en.with_element_node(common::is_element_node_ignored) {
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
                    .and_then(syntax_nodes::Component::new)
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
        .map(|info| info.node.source_file.path().to_owned());
    let filter = Box::new(move |e: &common::ElementRcNode| {
        e.with_element_node(|n| Some(n.source_file.path()) != root_node_path.as_deref())
    });
    let mark = Box::new(move |_: &common::ElementRcNode| false);
    find_filtered_location(component_instance, position, filter, mark, component_type)
}

fn find_move_location(
    component_instance: &ComponentInstance,
    position: LogicalPoint,
    selected_element: &common::ElementRcNode,
    component_type: &str,
) -> Option<DropInformation> {
    let se = selected_element.clone();
    let filter =
        Box::new(move |e: &common::ElementRcNode| *e == se || !e.is_same_component_as(&se));
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

    let layout_kind = drop_target_node.layout_kind();
    if layout_kind != ui::LayoutKind::None {
        let geometry = drop_target_node.geometry_at(component_instance, position)?;
        let children_geometries: Vec<_> = drop_target_node
            .children()
            .iter()
            .filter(|c| !c.with_element_node(common::is_element_node_ignored))
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
pub fn can_drop_at(
    document_cache: &common::DocumentCache,
    position: LogicalPoint,
    component: &common::ComponentInformation,
) -> bool {
    // let dm = &preview::component_instance()
    //     .and_then(|ci| find_drop_location(&ci, position, component_type));

    // preview::set_drop_mark(&dm.as_ref().and_then(|dm| dm.drop_mark.clone()));
    // dm.is_some()

    let Some(component_instance) = preview::component_instance() else {
        return false;
    };

    let dm = find_drop_location(&component_instance, position, &component.name);

    let can_drop = if let Some(dm) = &dm {
        // Cache compilation results:
        #[derive(Clone, Debug, Hash, Eq, PartialEq)]
        struct CacheEntry {
            component_type: String,
            target_element: by_address::ByAddress<object_tree::ElementRc>,
            target_node_index: usize,
            child_index: usize,
        }
        let cache_entry = CacheEntry {
            component_type: component.name.to_string(),
            target_element: by_address::ByAddress(dm.target_element_node.element.clone()),
            target_node_index: dm.target_element_node.debug_index,
            child_index: dm.child_index,
        };

        thread_local!(static CACHE: RefCell<clru::CLruCache<CacheEntry, bool>> = RefCell::new(clru::CLruCache::new(NonZeroUsize::new(10).unwrap())));
        CACHE.with_borrow_mut(|cache| {
            if let Some(does_compile) = cache.get(&cache_entry) {
                *does_compile
            } else {
                let does_compile = if let Some((edit, _)) =
                    create_drop_element_workspace_edit(document_cache, component, dm)
                {
                    workspace_edit_compiles(document_cache, &edit)
                } else {
                    false
                };
                cache.put(cache_entry, does_compile);
                does_compile
            }
        })
    } else {
        false
    };

    if can_drop {
        preview::set_drop_mark(&dm.unwrap().drop_mark);
    } else {
        preview::set_drop_mark(&None);
    }

    can_drop
}

pub fn workspace_edit_compiles(
    document_cache: &common::DocumentCache,
    workspace_edit: &lsp_types::WorkspaceEdit,
) -> bool {
    let Ok(mut result) = text_edit::apply_workspace_edit(document_cache, workspace_edit) else {
        return false;
    };

    let mut diag = BuildDiagnostics::default();

    let mut document_cache = document_cache.snapshot().expect("This is not loading anything!");

    // Fill in changed sources:
    for (u, c) in result.drain(..).map(|mut r| {
        let contents = std::mem::take(&mut r.contents);
        (r.url.clone(), contents)
    }) {
        diag = BuildDiagnostics::default(); // reset errors that might be due to missing changes elsewhere

        let _ = preview::poll_once(document_cache.load_url(&u, None, c, &mut diag));
    }

    !diag.has_errors()
}

/// Find the Element to insert into. None means we can not insert at this point.
pub fn can_move_to(
    document_cache: &common::DocumentCache,
    position: LogicalPoint,
    mouse_position: LogicalPoint,
    element_node: common::ElementRcNode,
    instance_index: usize,
) -> bool {
    let Some(component_instance) = preview::component_instance() else {
        return false;
    };

    let component_type = element_node.component_type();
    let dm =
        find_move_location(&component_instance, mouse_position, &element_node, &component_type);

    let can_move = if let Some(dm) = &dm {
        // Cache compilation results:
        #[derive(Clone, Debug, Hash, Eq, PartialEq)]
        struct CacheEntry {
            source_element: by_address::ByAddress<object_tree::ElementRc>,
            source_node_index: usize,
            target_element: by_address::ByAddress<object_tree::ElementRc>,
            target_node_index: usize,
            child_index: usize,
        }
        let cache_entry = CacheEntry {
            source_element: by_address::ByAddress(element_node.element.clone()),
            source_node_index: element_node.debug_index,
            target_element: by_address::ByAddress(dm.target_element_node.element.clone()),
            target_node_index: dm.target_element_node.debug_index,
            child_index: dm.child_index,
        };

        thread_local!(static CACHE: RefCell<clru::CLruCache<CacheEntry, bool>> = RefCell::new(clru::CLruCache::new(NonZeroUsize::new(10).unwrap())));
        CACHE.with_borrow_mut(|cache| {
            if let Some(does_compile) = cache.get(&cache_entry) {
                *does_compile
            } else {
                let does_compile = if let Some((edit, _)) = create_move_element_workspace_edit(
                    &component_instance,
                    dm,
                    &element_node,
                    instance_index,
                    position,
                ) {
                    workspace_edit_compiles(document_cache, &edit)
                } else {
                    false
                };
                cache.put(cache_entry, does_compile);
                does_compile
            }
        })
    } else {
        false
    };

    if can_move {
        preview::set_drop_mark(&dm.unwrap().drop_mark);
    } else {
        preview::set_drop_mark(&None);
    }

    can_move
}

/// Extra data on an added Element, relevant to the Preview side only.
#[derive(Clone, Debug)]
pub struct DropData {
    /// The offset to select next. This is different from the insert position
    /// due to indentation, etc.
    pub selection_offset: TextSize,
    pub path: std::path::PathBuf,
}

fn pretty_node_removal_range(node: &SyntaxNode) -> Option<TextRange> {
    let first_et = node.first_token()?;
    let before_et = first_et.prev_token()?;
    let start_pos = if before_et.kind() == SyntaxKind::Whitespace && before_et.text().contains('\n')
    {
        before_et.text_range().end()
            - TextSize::from(
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
            + TextSize::from(
                after_et.text().split('\n').next().map(|s| s.len() + 1).unwrap_or_default() as u32,
            )
    } else {
        last_et.text_range().end() // Use existing WS or not WS as appropriate
    };

    Some(TextRange::new(start_pos, end_pos))
}

fn drop_ignored_elements_from_node(
    node: &common::ElementRcNode,
    source_file: &SourceFile,
) -> Vec<lsp_types::TextEdit> {
    node.with_element_node(|node| {
        node.children()
            .filter_map(|c| {
                let e = common::extract_element(c.clone())?;
                if common::is_element_node_ignored(&e) {
                    pretty_node_removal_range(&e)
                        .map(|range| util::text_range_to_lsp_range(source_file, range))
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
    document_cache: &common::DocumentCache,
    position: LogicalPoint,
    component: &common::ComponentInformation,
) -> Option<(lsp_types::WorkspaceEdit, DropData)> {
    let component_instance = preview::component_instance()?;

    let drop_info = find_drop_location(&component_instance, position, &component.name)?;

    create_drop_element_workspace_edit(document_cache, component, &drop_info)
}

fn property_ranges(element: &common::ElementRcNode, remove_properties: &[&str]) -> Vec<TextRange> {
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
    document_cache: &common::DocumentCache,
    node: &SyntaxNode,
    replace_with: String,
) -> Option<common::SingleTextEdit> {
    let range =
        util::text_range_to_lsp_range(&node.source_file.clone(), pretty_node_removal_range(node)?);
    common::SingleTextEdit::from_path(
        document_cache,
        node.source_file.path(),
        lsp_types::TextEdit::new(range, replace_with),
    )
}

pub fn create_drop_element_workspace_edit(
    document_cache: &common::DocumentCache,
    component: &common::ComponentInformation,
    drop_info: &DropInformation,
) -> Option<(lsp_types::WorkspaceEdit, DropData)> {
    let placeholder = if component.is_layout { placeholder() } else { String::new() };

    let new_text = if component.default_properties.is_empty() {
        format!(
            "{}{} {{{placeholder} }}\n{}",
            drop_info.insert_info.pre_indent, component.name, drop_info.insert_info.post_indent
        )
    } else {
        let mut to_insert =
            format!("{}{} {{{placeholder}\n", drop_info.insert_info.pre_indent, component.name);
        for p in &component.default_properties {
            to_insert += &format!("{}    {}: {};\n", drop_info.insert_info.indent, p.name, p.value);
        }
        to_insert +=
            &format!("{}}}\n{}", drop_info.insert_info.indent, drop_info.insert_info.post_indent);
        to_insert
    };

    let mut selection_offset = drop_info.insert_info.insertion_position.offset()
        + TextSize::new(
            new_text.chars().take_while(|c| c.is_whitespace()).map(|c| c.len_utf8()).sum::<usize>()
                as u32,
        );

    let (path, _) = drop_info.target_element_node.path_and_offset();

    let doc = document_cache.get_document_by_path(&path)?;
    let source_file = doc.node.as_ref().unwrap().source_file.clone();

    let mut edits = Vec::with_capacity(3);
    let import_file = component.import_file_name(&lsp_types::Url::from_file_path(&path).ok());
    if let Some(edit) = completion::create_import_edit(doc, &component.name, &import_file) {
        if let Some(sf) = doc.node.as_ref().map(|n| &n.source_file) {
            selection_offset =
                text_edit::TextOffsetAdjustment::new(&edit, sf).adjust(selection_offset);
        }
        edits.push(edit);
    }

    edits.extend(
        drop_ignored_elements_from_node(&drop_info.target_element_node, &source_file)
            .drain(..)
            .inspect(|te| {
                selection_offset =
                    text_edit::TextOffsetAdjustment::new(te, &source_file).adjust(selection_offset);
            }),
    );

    let start_pos = util::text_size_to_lsp_position(
        &source_file,
        drop_info.insert_info.insertion_position.offset(),
    );
    let end_pos = util::text_size_to_lsp_position(
        &source_file,
        drop_info.insert_info.insertion_position.offset()
            + TextSize::new(drop_info.insert_info.replacement_range),
    );
    edits.push(lsp_types::TextEdit { range: lsp_types::Range::new(start_pos, end_pos), new_text });

    Some((
        common::create_workspace_edit_from_path(document_cache, source_file.path(), edits)?,
        DropData { selection_offset, path },
    ))
}

pub fn create_move_element_workspace_edit(
    component_instance: &ComponentInstance,
    drop_info: &DropInformation,
    element: &common::ElementRcNode,
    instance_index: usize,
    position: LogicalPoint,
) -> Option<(lsp_types::WorkspaceEdit, DropData)> {
    let component_type = element.component_type();
    let parent_of_element = element.parent();

    let placeholder_text = if Some(&drop_info.target_element_node) == parent_of_element.as_ref() {
        // We are moving within ourselves!

        let size = element.geometries(component_instance).get(instance_index).map(|g| g.size)?;

        if drop_info.target_element_node.layout_kind() == ui::LayoutKind::None {
            let (edit, _) = preview::resize_selected_element_impl(
                element,
                instance_index,
                LogicalRect::new(position, size),
            )?;
            let (path, selection_offset) = element.path_and_offset();
            return Some((edit, DropData { selection_offset, path }));
        } else {
            let children = &drop_info.target_element_node.children();
            let drop_index = if drop_info.child_index == usize::MAX {
                children.len() - 1
            } else {
                drop_info.child_index
            };
            if children.get(drop_index).is_some_and(|c| c == element) {
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
        let element_text_lines = extract_text_of_element(element, &["x", "y"]);

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

    let document_cache = preview::document_cache()?;
    let doc = document_cache.get_document_by_path(&path)?;
    let source_file = doc.node.as_ref().unwrap().source_file.clone();

    let mut selection_offset = drop_info.insert_info.insertion_position.offset()
        + TextSize::new(
            new_text.chars().take_while(|c| c.is_whitespace()).map(|c| c.len_utf8()).sum::<usize>()
                as u32,
        );

    let mut edits = Vec::with_capacity(3);

    let remove_me = element.with_decorated_node(|node| {
        node_removal_text_edit(&document_cache, &node, placeholder_text.clone())
    })?;
    if remove_me.url.to_file_path().as_ref().map(|p| p.as_path()) == Ok(source_file.path()) {
        selection_offset = text_edit::TextOffsetAdjustment::new(&remove_me.edit, &source_file)
            .adjust(selection_offset);
    }
    edits.push(remove_me);

    if let Some(component_info) = preview::get_component_info(&component_type) {
        let import_file =
            component_info.import_file_name(&lsp_types::Url::from_file_path(&path).ok());
        if let Some(edit) = completion::create_import_edit(doc, &component_type, &import_file) {
            if let Some(sf) = doc.node.as_ref().map(|n| &n.source_file) {
                selection_offset =
                    text_edit::TextOffsetAdjustment::new(&edit, sf).adjust(selection_offset);
            }
            edits.push(common::SingleTextEdit::from_path(
                &document_cache,
                source_file.path(),
                edit,
            )?);
        }
    }

    edits.extend(
        drop_ignored_elements_from_node(&drop_info.target_element_node, &source_file)
            .drain(..)
            .filter_map(|te| {
                // Abuse map somewhat...
                selection_offset = text_edit::TextOffsetAdjustment::new(&te, &source_file)
                    .adjust(selection_offset);
                common::SingleTextEdit::from_path(&document_cache, source_file.path(), te)
            }),
    );

    let start_pos = util::text_size_to_lsp_position(
        &source_file,
        drop_info.insert_info.insertion_position.offset(),
    );
    let end_pos = util::text_size_to_lsp_position(
        &source_file,
        drop_info.insert_info.insertion_position.offset()
            + TextSize::new(drop_info.insert_info.replacement_range),
    );
    edits.push(common::SingleTextEdit::from_path(
        &document_cache,
        source_file.path(),
        lsp_types::TextEdit { range: lsp_types::Range::new(start_pos, end_pos), new_text },
    )?);

    Some((
        common::create_workspace_edit_from_single_text_edits(edits),
        DropData { selection_offset, path },
    ))
}

/// Find a location in a file that would be a good place to insert the new component at
///
/// Return a WorkspaceEdit to send to the editor and extra info for the live preview in
/// the DropData struct.
pub fn move_element_to(
    document_cache: &common::DocumentCache,
    element: common::ElementRcNode,
    instance_index: usize,
    position: LogicalPoint,
    mouse_position: LogicalPoint,
) -> Option<(lsp_types::WorkspaceEdit, DropData)> {
    let component_instance = preview::component_instance()?;
    let Some(drop_info) = find_move_location(
        &component_instance,
        mouse_position,
        &element,
        &element.component_type(),
    ) else {
        // Can not drop here: Ignore the move
        return None;
    };
    create_move_element_workspace_edit(
        &component_instance,
        &drop_info,
        &element,
        instance_index,
        position,
    )
    .and_then(|(e, d)| workspace_edit_compiles(document_cache, &e).then_some((e, d)))
}

#[cfg(test)]
mod tests {
    use i_slint_compiler::parser::{TextRange, TextSize};
    use lsp_types::Url;

    use std::collections::HashMap;

    use crate::{
        common::{self, test, text_edit},
        util,
    };

    pub const DEMO_CODE: &str = r#"import { Button } from "std-widgets.slint";

component SomeComponent { // 69
    @children
}

component Main { // 109
    width: 200px;
    height: 200px;

    HorizontalLayout { // 160
        Rectangle { // 194
            SomeComponent { // 225
                property <length> button-width: 80px;
                Button { // 318
                    width: parent.button-width;
                    text: "Press me";
                }
            }
        }
        Rectangle { // 470
            background: Colors.blue;
        }
    }
}

export component Entry inherits Main { /* @lsp:ignore-node */ } // 582
"#;

    fn workspace_edit_setup(
        edits: Vec<(usize, usize, &str)>,
    ) -> (common::DocumentCache, lsp_types::WorkspaceEdit) {
        let document_cache = test::compile_test_with_sources(
            "fluent",
            HashMap::from([(
                Url::from_file_path(test::main_test_file_name()).unwrap(),
                DEMO_CODE.to_string(),
            )]),
            false,
        );
        let doc = document_cache.get_document_by_path(&test::main_test_file_name()).unwrap();
        let source_file = &doc.node.as_ref().unwrap().source_file;

        let edits = edits
            .iter()
            .filter_map(|(so, eo, t)| {
                let range = util::text_range_to_lsp_range(
                    source_file,
                    TextRange::new(TextSize::new(*so as u32), TextSize::new(*eo as u32)),
                );
                common::SingleTextEdit::from_path(
                    &document_cache,
                    source_file.path(),
                    lsp_types::TextEdit { range, new_text: t.to_string() },
                )
            })
            .collect();

        let workspace_edit = crate::common::create_workspace_edit_from_single_text_edits(edits);

        (document_cache, workspace_edit)
    }

    #[test]
    fn test_workspace_edit_compiles_ok() {
        let (document_cache, workspace_edit) = workspace_edit_setup(vec![(194, 194, "foo := ")]);

        assert!(super::workspace_edit_compiles(&document_cache, &workspace_edit));
    }

    #[test]
    fn test_workspace_edit_compiles_parse_fails() {
        let (document_cache, workspace_edit) = workspace_edit_setup(vec![(194, 194, "FOOBAR ")]);

        assert!(!super::workspace_edit_compiles(&document_cache, &workspace_edit));
    }

    #[test]
    fn test_workspace_edit_compiles_passes_fail() {
        let (document_cache, workspace_edit) = workspace_edit_setup(vec![(
            194,
            194,
            "property <bool> foobar: root.foobar;\n        ",
        )]);

        assert!(!super::workspace_edit_compiles(&document_cache, &workspace_edit));
    }

    #[test]
    fn test_workspace_edit_compiles_move_element_fail() {
        let (document_cache, workspace_edit) = workspace_edit_setup(vec![(
            314,
            450,
            "",
        ),
        (
            460,
            461,
            "    Button { // 318\n                width: parent.button_width;\n                text: \"Press me\";\n            }\n        "
        )]);

        assert!(!super::workspace_edit_compiles(&document_cache, &workspace_edit));
    }

    #[test]
    fn test_workspace_edit_compiles_move_element_ok() {
        let (document_cache, workspace_edit) =
            workspace_edit_setup(vec![(
            466,
            540,
            "",
        ),
        (
            194,
            194,
            "Rectangle { // 470\n              background: Colors.blue;\n        }\n        "
        ),]);

        assert!(super::workspace_edit_compiles(&document_cache, &workspace_edit));
    }

    #[test]
    fn test_workspace_edit_compiles_move_element_inside_component_ok() {
        let (document_cache, workspace_edit) =
            workspace_edit_setup(vec![(
            314,
            450,
            "",
        ),
        (
            264,
            264,
            "Button { // 318\n                    width: parent.button-width;\n                    text: \"Press me\";\n                }"
        ),]);

        assert!(super::workspace_edit_compiles(&document_cache, &workspace_edit));
    }

    #[test]
    fn test_workspace_edit_compiles_edit_button_text_ok() {
        let (document_cache, workspace_edit) = workspace_edit_setup(vec![(409, 417, "xxx")]);

        assert!(super::workspace_edit_compiles(&document_cache, &workspace_edit));
    }

    // #[track_caller]
    fn add_component_test(input: &str, output: &str, selection_offset: u32) {
        let document_cache = test::compile_test_with_sources(
            "fluent",
            HashMap::from([(
                Url::from_file_path(test::main_test_file_name()).unwrap(),
                input.to_string(),
            )]),
            true,
        );
        let doc = document_cache.get_document_by_path(&test::main_test_file_name()).unwrap();
        let doc_node = doc.node.as_ref().unwrap();

        let (workspace_edit, drop_data) =
            super::add_new_component(&document_cache, "TestComponent", doc_node).unwrap();

        let result = text_edit::apply_workspace_edit(&document_cache, &workspace_edit).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].url.to_file_path().unwrap(), test::main_test_file_name());
        assert_eq!(&result[0].contents, output);

        assert_eq!(drop_data.path, test::main_test_file_name());
        assert_eq!(drop_data.selection_offset, selection_offset.into());

        assert!(super::workspace_edit_compiles(&document_cache, &workspace_edit));
    }

    #[test]
    fn test_add_new_component_into_empty() {
        add_component_test("", "component TestComponent { }\n", 10);
    }

    #[test]
    fn test_add_new_component_into_ws() {
        add_component_test("    \n    \n \n \t \n", "component TestComponent { }\n", 10);
    }

    #[test]
    fn test_add_new_component_into_no_component_struct() {
        add_component_test("struct S { }", "struct S { }\n\ncomponent TestComponent { }\n", 24);
    }

    #[test]
    fn test_add_new_component_into_no_component_struct_nl() {
        add_component_test(
            "struct S { }\n    ",
            "struct S { }\n\ncomponent TestComponent { }\n",
            24,
        );
    }

    #[test]
    fn test_add_new_component_into_no_component_struct_nl_sp_nl() {
        add_component_test(
            "struct S { }\n  \n",
            "struct S { }\n  \ncomponent TestComponent { }\n",
            26,
        );
    }

    #[test]
    fn test_add_new_component_into_no_component_nl_sp_nl_sp() {
        add_component_test(
            "struct S { }\n  \n    ",
            "struct S { }\n  \ncomponent TestComponent { }\n",
            26,
        );
    }

    #[test]
    fn test_add_new_component_into_no_component_nl_sp_nl_nl() {
        add_component_test(
            "struct S { }\n  \n\n",
            "struct S { }\n  \n\ncomponent TestComponent { }\n",
            27,
        );
    }

    #[test]
    fn test_add_new_component_into_no_component_struct_nl_nl_nl_sp() {
        add_component_test(
            "struct S { }\n  \n\n    ",
            "struct S { }\n  \n\ncomponent TestComponent { }\n",
            27,
        );
    }

    #[test]
    fn test_add_new_component_into_component() {
        add_component_test("component C { }", "component TestComponent { }\n\ncomponent C { }", 10);
    }

    #[test]
    fn test_add_new_component_into_export_component() {
        add_component_test(
            "export component C { }",
            "component TestComponent { }\n\nexport component C { }",
            10,
        );
    }

    #[test]
    fn test_add_new_component_into_export_component_component() {
        add_component_test(
            "export component CE { }\n\ncomponent C { }",
            "component TestComponent { }\n\nexport component CE { }\n\ncomponent C { }",
            10,
        );
    }

    #[test]
    fn test_add_new_component_into_component_export_component() {
        add_component_test(
            "component C { }\n\nexport component CE { }",
            "component TestComponent { }\n\ncomponent C { }\n\nexport component CE { }",
            10,
        );
    }

    #[test]
    fn test_add_new_component_into_struct_export_component() {
        add_component_test(
            "struct S { }export component C { }",
            "struct S { }\n\ncomponent TestComponent { }\n\nexport component C { }",
            24,
        );
    }

    #[test]
    fn test_add_new_component_into_struct_export_sp_component() {
        add_component_test(
            "struct S { }     export component C { }",
            "struct S { }\n\ncomponent TestComponent { }\n\nexport component C { }",
            24,
        );
    }

    #[test]
    fn test_add_new_component_into_ws_component() {
        add_component_test(
            "\n     \n  \n\t  \n          component C { }",
            "component TestComponent { }\n\ncomponent C { }",
            10,
        );
    }

    #[test]
    fn test_add_new_component_into_struct_sp_nl_sp_component() {
        add_component_test(
            "struct S { }  \n  component C { }",
            "struct S { }  \n\ncomponent TestComponent { }\n\ncomponent C { }",
            26,
        );
    }
}
