// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

use i_slint_core::lengths::{LogicalLength, LogicalPoint};
use slint_interpreter::ComponentInstance;

use crate::common;
use crate::language::completion;
use crate::preview::{self, element_selection};
use crate::util;

#[cfg(target_arch = "wasm32")]
use crate::wasm_prelude::*;

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

pub struct DropInformation {
    pub target_element_node: common::ElementRcNode,
    pub insertion_position: common::VersionedPosition,
}

fn find_drop_location(
    component_instance: &ComponentInstance,
    x: f32,
    y: f32,
    component_type: &str,
) -> Option<DropInformation> {
    let target_element_node = {
        let mut result = None;
        let tl = component_instance.definition().type_loader();
        for sc in &element_selection::collect_all_element_nodes_covering(x, y, component_instance) {
            let Some(en) = sc.as_element_node() else {
                continue;
            };

            if en.with_element_node(preview::is_element_node_ignored) {
                continue;
            }

            let (path, _) = en.path_and_offset();
            let Some(doc) = tl.get_document(&path) else {
                continue;
            };
            if let Some(element_type) = en.with_element_node(|node| {
                util::lookup_current_element_type((node.clone()).into(), &doc.local_registry)
            }) {
                if !en.is_layout()
                    && element_type
                        .accepts_child_element(component_type, &doc.local_registry)
                        .is_err()
                {
                    break;
                }
            }

            if !element_selection::is_same_file_as_root_node(component_instance, &en) {
                continue;
            }

            result = Some(en);
            break;
        }
        result
    }?;

    let insertion_position = target_element_node.with_element_node(|node| {
        let last_token = crate::util::last_non_ws_token(node)?;

        let url = lsp_types::Url::from_file_path(node.source_file.path()).ok()?;
        let (version, _) = preview::get_url_from_cache(&url)?;

        Some(common::VersionedPosition::new(
            crate::common::VersionedUrl::new(url, version),
            Into::<u32>::into(last_token.text_range().end()).saturating_sub(1),
        ))
    })?;

    Some(DropInformation { target_element_node, insertion_position })
}

/// Find the Element to insert into. None means we can not insert at this point.
pub fn can_drop_at(x: f32, y: f32, component: &common::ComponentInformation) -> bool {
    let component_type = component.name.to_string();
    super::component_instance()
        .and_then(|ci| find_drop_location(&ci, x, y, &component_type))
        .is_some()
}

/// Extra data on an added Element, relevant to the Preview side only.
#[derive(Clone, Debug)]
pub struct DropData {
    /// The offset to select next. This is different from the insert position
    /// due to indentation, etc.
    pub selection_offset: u32,
    pub path: std::path::PathBuf,
    pub is_layout: bool,
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
    let component_type = &component.name;
    let component_instance = preview::component_instance()?;
    let tl = component_instance.definition().type_loader();
    let drop_info = find_drop_location(&component_instance, x, y, component_type)?;

    let properties = {
        let mut props = component.default_properties.clone();

        let click_position =
            LogicalPoint::from_lengths(LogicalLength::new(x), LogicalLength::new(y));

        if !drop_info.target_element_node.is_layout() && !component.fills_parent {
            if let Some(area) = component_instance
                .element_positions(&drop_info.target_element_node.element)
                .iter()
                .find(|p| p.contains(click_position))
            {
                props.push(common::PropertyChange::new("x", format!("{}px", x - area.origin.x)));
                props.push(common::PropertyChange::new("y", format!("{}px", y - area.origin.y)));
            }
        }

        props
    };

    let indentation = format!(
        "{}    ",
        crate::util::find_element_indent(&drop_info.target_element_node).unwrap_or_default()
    );

    let new_text = if properties.is_empty() {
        format!("{}{} {{ }}\n", indentation, component_type)
    } else {
        let mut to_insert = format!("{}{} {{\n", indentation, component_type);
        for p in &properties {
            to_insert += &format!("{}    {}: {};\n", indentation, p.name, p.value);
        }
        to_insert += &format!("{}}}\n", indentation);
        to_insert
    };

    let mut selection_offset = drop_info.insertion_position.offset()
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

    let ip = util::map_position(&source_file, drop_info.insertion_position.offset().into());
    edits.push(lsp_types::TextEdit { range: lsp_types::Range::new(ip, ip), new_text });

    Some((
        common::create_workspace_edit_from_source_file(&source_file, edits)?,
        DropData { selection_offset, path, is_layout: component.is_layout },
    ))
}
