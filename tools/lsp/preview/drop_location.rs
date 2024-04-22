// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.2 OR LicenseRef-Slint-commercial

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
        // This is a bit simplistic: We ignore special cases like the offset
        // being in the area that gets removed.
        // Worst case: Some unexpected element gets selected. We can live with that.
        if offset >= self.start_offset {
            let old_length = self.end_offset - self.start_offset;
            offset + self.new_text_length - old_length
        } else {
            offset
        }
    }
}

#[derive(Clone, Debug)]
pub struct DropInformation {
    pub target_element_node: common::ElementRcNode,
    pub insert_info: InsertInformation,
    pub drop_mark: Option<DropMark>,
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

// calculate where to draw the `DropMark`
fn calculate_drop_information_for_layout(
    geometry: &LogicalRect,
    position: LogicalPoint,
    layout_kind: &crate::preview::ui::LayoutKind,
    children_geometries: &[LogicalRect],
) -> (Option<DropMark>, usize) {
    match layout_kind {
        ui::LayoutKind::None => unreachable!("We are in a layout"),
        ui::LayoutKind::Horizontal => {
            if children_geometries.is_empty() {
                // No children: Draw a drop mark in the middle
                // TODO: Take padding into account: We have no way to get that though
                let start = (geometry.origin.x + (geometry.size.width / 2.0)).floor();

                (
                    Some(DropMark {
                        start: LogicalPoint::new(start, geometry.origin.y),
                        end: LogicalPoint::new(
                            start + 1.0,
                            geometry.origin.y + geometry.size.height,
                        ),
                    }),
                    usize::MAX,
                )
            } else {
                let mut last_midpoint = geometry.origin.x;
                let mut last_endpoint = geometry.origin.x;
                for (pos, c) in children_geometries.iter().enumerate() {
                    let new_midpoint = c.origin.x + c.size.width / 2.0;
                    let hit_rect = LogicalRect::new(
                        LogicalPoint::new(last_midpoint, geometry.origin.y),
                        LogicalSize::new(new_midpoint - last_midpoint, geometry.size.height),
                    );
                    if hit_rect.contains(position) {
                        let start = (c.origin.x - last_endpoint) / 2.0;
                        let start_pos = last_endpoint
                            + if start.floor() < geometry.origin.x {
                                geometry.origin.x
                            } else {
                                start
                            };
                        let end_pos = start_pos + 1.0;

                        return (
                            Some(DropMark {
                                start: LogicalPoint::new(start_pos, geometry.origin.y),
                                end: LogicalPoint::new(
                                    end_pos,
                                    geometry.origin.y + geometry.size.height,
                                ),
                            }),
                            pos,
                        );
                    }
                    last_midpoint = new_midpoint;
                    last_endpoint = c.origin.x + c.size.width;
                }
                (
                    Some(DropMark {
                        start: LogicalPoint::new(
                            geometry.origin.x + geometry.size.width - 1.0,
                            geometry.origin.y,
                        ),
                        end: LogicalPoint::new(
                            geometry.origin.x + geometry.size.width,
                            geometry.origin.y + geometry.size.height,
                        ),
                    }),
                    usize::MAX,
                )
            }
        }
        ui::LayoutKind::Vertical => {
            if children_geometries.is_empty() {
                // No children: Draw a drop mark in the middle
                // TODO: Take padding into account: We have no way to get that though
                let start = (geometry.origin.y + (geometry.size.height / 2.0)).floor();
                (
                    Some(DropMark {
                        start: LogicalPoint::new(geometry.origin.x, start),
                        end: LogicalPoint::new(
                            geometry.origin.x + geometry.size.width,
                            start + 1.0,
                        ),
                    }),
                    usize::MAX,
                )
            } else {
                let mut last_midpoint = geometry.origin.y;
                let mut last_endpoint = geometry.origin.y;
                for (pos, c) in children_geometries.iter().enumerate() {
                    let new_midpoint = c.origin.y + c.size.height / 2.0;
                    let hit_rect = LogicalRect::new(
                        LogicalPoint::new(geometry.origin.y, last_midpoint),
                        LogicalSize::new(geometry.size.width, new_midpoint - last_midpoint),
                    );
                    if hit_rect.contains(position) {
                        let start = (c.origin.y - last_endpoint) / 2.0;
                        let start_pos = last_endpoint
                            + if start.floor() < geometry.origin.y {
                                geometry.origin.y
                            } else {
                                start
                            };
                        let end_pos = start_pos + 1.0;

                        return (
                            Some(DropMark {
                                start: LogicalPoint::new(geometry.origin.x, start_pos),
                                end: LogicalPoint::new(
                                    geometry.origin.x + geometry.size.width,
                                    end_pos,
                                ),
                            }),
                            pos,
                        );
                    }
                    last_midpoint = new_midpoint;
                    last_endpoint = c.origin.y + c.size.height;
                }
                (
                    Some(DropMark {
                        start: LogicalPoint::new(
                            geometry.origin.x,
                            geometry.origin.y + geometry.size.height - 1.0,
                        ),
                        end: LogicalPoint::new(
                            geometry.origin.x + geometry.size.width,
                            geometry.origin.y + geometry.size.height,
                        ),
                    }),
                    usize::MAX,
                )
            }
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
) -> Vec<common::ElementRcNode> {
    let mut result = Vec::with_capacity(3);

    for sc in &element_selection::collect_all_element_nodes_covering(position, component_instance) {
        let Some(en) = sc.as_element_node() else {
            continue;
        };

        if en.with_element_node(preview::is_element_node_ignored) {
            continue;
        }

        if !element_selection::is_same_file_as_root_node(component_instance, &en) {
            continue;
        }

        result.push(en);
    }

    result
}

fn extract_element(node: SyntaxNode) -> Option<syntax_nodes::Element> {
    match node.kind() {
        SyntaxKind::Element => Some(node.into()),
        SyntaxKind::SubElement => extract_element(node.child_node(SyntaxKind::Element)?),
        SyntaxKind::ConditionalElement | SyntaxKind::RepeatedElement => {
            extract_element(node.child_node(SyntaxKind::SubElement)?)
        }
        _ => None,
    }
}

fn find_element_to_drop_into(
    component_instance: &ComponentInstance,
    position: LogicalPoint,
) -> Option<common::ElementRcNode> {
    let all_element_nodes = drop_target_element_nodes(component_instance, position);

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
    let drop_target_node = find_element_to_drop_into(component_instance, position)?;

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
        let children_information: Vec<_> = drop_target_node.with_element_node(|node| {
            let mut children_info = Vec::new();
            for c in node.children() {
                if let Some(element) = extract_element(c.clone()) {
                    let e_path = element.source_file.path().to_path_buf();
                    let e_offset = u32::from(element.text_range().start());

                    let Some(child_node) = common::ElementRcNode::find_in_or_below(
                        drop_target_node.as_element().clone(),
                        &e_path,
                        e_offset,
                    ) else {
                        continue;
                    };
                    let Some(c_geometry) = child_node.geometry_in(component_instance, &geometry)
                    else {
                        continue;
                    };
                    children_info.push(c_geometry);
                }
            }

            children_info
        });

        let (drop_mark, child_index) = calculate_drop_information_for_layout(
            &geometry,
            position,
            &layout_kind,
            &children_information,
        );

        let insert_info = {
            if child_index == usize::MAX {
                insert_position_at_end(&drop_target_node)
            } else {
                insert_position_before_child(&drop_target_node, child_index)
            }
        }?;

        Some(DropInformation { target_element_node: drop_target_node, insert_info, drop_mark })
    } else {
        let insert_info = insert_position_at_end(&drop_target_node)?;
        Some(DropInformation {
            target_element_node: drop_target_node,
            insert_info,
            drop_mark: None,
        })
    }
}

/// Find the Element to insert into. None means we can not insert at this point.
pub fn can_drop_at(x: f32, y: f32, component: &common::ComponentInformation) -> bool {
    let component_type = component.name.to_string();
    let position = LogicalPoint::new(x, y);
    let dm = &super::component_instance()
        .and_then(|ci| find_drop_location(&ci, position, &component_type));

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

/// Find a location in a file that would be a good place to insert the new component at
///
/// Return a WorkspaceEdit to send to the editor and extra info for the live preview in
/// the DropData struct.
pub fn drop_at(
    x: f32,
    y: f32,
    component: &common::ComponentInformation,
) -> Option<(lsp_types::WorkspaceEdit, DropData)> {
    let position = LogicalPoint::new(x, y);
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
                props.push(common::PropertyChange::new("x", format!("{}px", x - area.origin.x)));
                props.push(common::PropertyChange::new("y", format!("{}px", y - area.origin.y)));
            }
        }

        props
    };

    let new_text = if properties.is_empty() {
        format!(
            "{}{} {{ }}\n{}",
            drop_info.insert_info.pre_indent, component_type, drop_info.insert_info.post_indent
        )
    } else {
        let mut to_insert = format!("{}{} {{\n", drop_info.insert_info.pre_indent, component_type);
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
    let mut edits = Vec::with_capacity(2);
    let import_file = component.import_file_name(&lsp_types::Url::from_file_path(&path).ok());
    if let Some(edit) = completion::create_import_edit(doc, component_type, &import_file) {
        if let Some(sf) = doc.node.as_ref().map(|n| &n.source_file) {
            selection_offset = TextOffsetAdjustment::new(&edit, sf).adjust(selection_offset);
        }
        edits.push(edit);
    }

    let source_file = doc.node.as_ref().unwrap().source_file.clone();

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
