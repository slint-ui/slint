// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::highlight::{SourceOccurrences, geometry_anchor};
use crate::prelude::*;
use i_slint_compiler::source_path::SourcePath;
use i_slint_core::lengths::{ItemTransform, LogicalPoint};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OccurrenceRelationship {
    Parent,
    Sibling,
    Other,
}

#[derive(Clone, Debug)]
pub struct OccurrenceGeometry {
    pub path: SourcePath,
    pub offset: u32,
    pub instance_index: usize,
    /// The corners of the item in the coordinate space of the selected items parent.
    /// So e.g. setting the selected items x/y to any of these values would make it snap to them.
    pub corners: [LogicalPoint; 4],
    pub relationship: OccurrenceRelationship,
}

#[derive(Clone, Debug)]
pub struct SnappingGeometry {
    /// This allows mapping the corners in the OccurrenceGeometry back to preview root coordinates
    pub parent_to_root: ItemTransform,
    pub occurrences: Vec<OccurrenceGeometry>,
}

pub(crate) fn snapping_geometry(
    root: &VRc<ItemTreeVTable, Instance>,
    path: &SourcePath,
    offset: u32,
    instance_index: usize,
) -> Option<SnappingGeometry> {
    use i_slint_core::item_tree::ParentItemTraversalMode::StopAtPopups;

    let sources = SourceOccurrences::new(root);
    let selected_source = sources.elements.get(&(path.clone(), offset as usize))?;
    let (selected_index, selected_geometry) =
        selected_source.occurrences(&sources.items, root).nth(instance_index)?;
    let (selected_anchor, _) = geometry_anchor(&sources.items[selected_index].as_item_rc());
    let selected_parent = selected_anchor.parent_item(StopAtPopups);
    let parent_anchor = selected_parent.as_ref().map(|parent| geometry_anchor(parent).0);
    let root_to_parent = selected_geometry.parent_transform.inverse()?;
    let mut seen = vec![selected_anchor];
    let mut occurrences = Vec::new();
    for ((path, offset), source) in &sources.elements {
        for (instance_index, (item_index, _)) in
            source.occurrences(&sources.items, root).enumerate()
        {
            let item = sources.items[item_index].as_item_rc();
            let (anchor, _) = geometry_anchor(&item);
            if !item.is_visible() || seen.contains(&anchor) {
                continue;
            }
            seen.push(anchor.clone());
            let relationship = if Some(&anchor) == parent_anchor.as_ref() {
                OccurrenceRelationship::Parent
            } else if anchor.parent_item(StopAtPopups) == selected_parent {
                OccurrenceRelationship::Sibling
            } else {
                OccurrenceRelationship::Other
            };
            occurrences.push(OccurrenceGeometry {
                path: path.clone(),
                offset: *offset as u32,
                instance_index,
                corners: sources.items[item_index]
                    .corners(root)
                    .map(|corner| root_to_parent.transform_point(corner.cast()).cast()),
                relationship,
            });
        }
    }
    Some(SnappingGeometry { parent_to_root: selected_geometry.parent_transform, occurrences })
}
