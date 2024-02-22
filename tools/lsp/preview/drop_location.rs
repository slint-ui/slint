// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

use i_slint_core::lengths::{LogicalLength, LogicalPoint};
use slint_interpreter::ComponentInstance;

use crate::preview::element_selection::collect_all_element_nodes_covering;

#[cfg(target_arch = "wasm32")]
use crate::wasm_prelude::*;

use super::element_selection::ElementRcNode;

pub struct DropInformation {
    pub target_element_node: ElementRcNode,
    pub insertion_position: crate::common::VersionedPosition,
}

fn find_drop_location(
    component_instance: &ComponentInstance,
    x: f32,
    y: f32,
) -> Option<DropInformation> {
    let target_element_node = {
        let mut result = None;
        for sc in &collect_all_element_nodes_covering(x, y, &component_instance) {
            let Some(en) = sc.as_element_node() else {
                continue;
            };

            if en.on_element_node(|n| super::is_element_node_ignored(n)) {
                continue;
            }

            result = Some(en);
            break;
        }
        result
    }?;

    let insertion_position = {
        let elem = target_element_node.element.borrow();

        let (node, _) = elem.debug.get(target_element_node.debug_index)?;

        let last_token = crate::util::last_non_ws_token(node)?;

        let url = lsp_types::Url::from_file_path(node.source_file.path()).ok()?;
        let Some((version, _)) = crate::preview::get_url_from_cache(&url) else {
            return None;
        };

        crate::common::VersionedPosition::new(
            crate::common::VersionedUrl::new(url, version),
            Into::<u32>::into(last_token.text_range().end()).saturating_sub(1),
        )
    };

    Some(DropInformation { target_element_node, insertion_position })
}

/// Find the Element to insert into. None means we can not insert at this point.
pub fn can_drop_at(x: f32, y: f32) -> bool {
    super::component_instance().and_then(|ci| find_drop_location(&ci, x, y)).is_some()
}

/// Find a location in a file that would be a good place to insert the new component at
///
/// Return a tuple containing the ComponentAddition info for the LSP and extra info for
/// the live preview. Currently that extra info is just the offset at which the new element
/// will be in the source code (!= the insertion position in the ComponentAddition struct).
pub fn drop_at(
    x: f32,
    y: f32,
    component_type: String,
    import_path: String,
) -> Option<(crate::common::ComponentAddition, u32)> {
    let component_instance = super::component_instance()?;
    let drop_info = find_drop_location(&component_instance, x, y)?;

    let properties = {
        let click_position =
            LogicalPoint::from_lengths(LogicalLength::new(x), LogicalLength::new(y));

        if drop_info.target_element_node.is_layout() {
            vec![]
        } else if let Some(area) = component_instance
            .element_position(&drop_info.target_element_node.element)
            .iter()
            .find(|p| p.contains(click_position))
        {
            vec![
                ("x".to_string(), format!("{}px", x - area.origin.x)),
                ("y".to_string(), format!("{}px", y - area.origin.y)),
            ]
        } else {
            vec![]
        }
    };

    let indentation = format!(
        "{}    ",
        crate::util::find_element_node_indent(
            &drop_info.target_element_node.element,
            drop_info.target_element_node.debug_index
        )
        .unwrap_or_default()
    );

    let component_text = if properties.is_empty() {
        format!("{}{} {{ }}\n", indentation, component_type)
    } else {
        let mut to_insert = format!("{}{} {{\n", indentation, component_type);
        for (k, v) in &properties {
            to_insert += &format!("{}    {k}: {v};\n", indentation);
        }
        to_insert += &format!("{}}}\n", indentation);
        to_insert
    };

    let selection_offset = drop_info.insertion_position.offset()
        + component_text
            .chars()
            .take_while(|c| c.is_whitespace())
            .map(|c| c.len_utf8())
            .sum::<usize>() as u32;

    Some((
        crate::common::ComponentAddition {
            component_type,
            component_text,
            import_path: if import_path.is_empty() { None } else { Some(import_path) },
            insert_position: drop_info.insertion_position,
        },
        selection_offset,
    ))
}
