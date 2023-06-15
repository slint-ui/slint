// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.0 OR LicenseRef-Slint-commercial

// cSpell: ignore descendents

use alloc::vec::Vec;

use crate::items::ItemRc;

// The property names of the accessible-properties
#[repr(C)]
#[derive(PartialEq, Eq, Copy, Clone, strum::Display)]
#[strum(serialize_all = "kebab-case")]
pub enum AccessibleStringProperty {
    Checkable,
    Checked,
    DelegateFocus,
    Description,
    Label,
    Value,
    ValueMaximum,
    ValueMinimum,
    ValueStep,
}

/// Find accessible descendents of `root_item`.
///
/// This will recurse through all children of `root_item`, but will not recurse
/// into nodes that are accessible.
pub fn accessible_descendents(root_item: &ItemRc) -> impl Iterator<Item = ItemRc> {
    fn try_candidate_or_find_next_accessible_descendent(
        candidate: ItemRc,
        descendent_candidates: &mut Vec<ItemRc>,
    ) -> Option<ItemRc> {
        if candidate.is_accessible() {
            return Some(candidate);
        }

        candidate.first_child().and_then(|child| {
            if let Some(next) = child.next_sibling() {
                descendent_candidates.push(next);
            }
            try_candidate_or_find_next_accessible_descendent(child, descendent_candidates)
        })
    }

    // Do not look on the root_item: That is either a component root or an
    // accessible item already handled!
    let mut descendent_candidates = Vec::new();
    if let Some(child) = root_item.first_child() {
        descendent_candidates.push(child);
    }

    core::iter::from_fn(move || loop {
        let candidate = descendent_candidates.pop()?;

        if let Some(next_candidate) = candidate.next_sibling() {
            descendent_candidates.push(next_candidate);
        }

        if let Some(descendent) =
            try_candidate_or_find_next_accessible_descendent(candidate, &mut descendent_candidates)
        {
            return Some(descendent);
        }
    })
}
