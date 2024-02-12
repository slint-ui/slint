// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

use i_slint_compiler::object_tree::ElementRc;
use i_slint_core::lengths::{LogicalLength, LogicalPoint};
use slint_interpreter::ComponentInstance;

use crate::preview::element_selection::collect_all_elements_covering;

#[cfg(target_arch = "wasm32")]
use crate::wasm_prelude::*;

pub struct DropInformation {
    pub target_element: ElementRc,
    pub node_index: usize,
    pub insertion_position: crate::common::VersionedPosition,
}

fn find_drop_location(
    component_instance: &ComponentInstance,
    x: f32,
    y: f32,
) -> Option<DropInformation> {
    let elements = collect_all_elements_covering(x, y, &component_instance);
    let (node_index, target_element) = elements.iter().find_map(|sc| {
        sc.element
            .borrow()
            .node
            .iter()
            .enumerate()
            .filter(|(_, n)| !crate::common::is_element_node_ignored(n))
            .next()
            .map(|(i, _)| (i, sc.element.clone()))
    })?;

    let insertion_position = {
        let elem = target_element.borrow();

        if elem.layout.is_some() {
            return None;
        }

        let node = elem.node.get(node_index)?;
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

    Some(DropInformation { target_element, node_index, insertion_position })
}

/// Find the Element to insert into. None means we can not insert at this point.
pub fn can_drop_at(x: f32, y: f32) -> bool {
    super::component_instance().and_then(|ci| find_drop_location(&ci, x, y)).is_some()
}

/// Find a location in a file that would be a good place to insert the new component at
pub fn drop_at(
    x: f32,
    y: f32,
    component_type: String,
    import_path: String,
) -> Option<crate::common::ComponentAddition> {
    let component_instance = super::component_instance()?;
    let drop_info = find_drop_location(&component_instance, x, y)?;

    let properties = {
        let click_position =
            LogicalPoint::from_lengths(LogicalLength::new(x), LogicalLength::new(y));

        if let Some(area) = component_instance
            .element_position(&drop_info.target_element)
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

    Some(crate::common::ComponentAddition {
        component_type,
        import_path: if import_path.is_empty() { None } else { Some(import_path) },
        insert_position: drop_info.insertion_position,
        properties,
    })
}
