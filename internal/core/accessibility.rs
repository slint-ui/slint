// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.2 OR LicenseRef-Slint-commercial

// cSpell: ignore descendents

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use crate::{items::ItemRc, SharedString};

use bitflags::bitflags;

/// The property names of the accessible-properties
#[repr(u32)]
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

/// The argument of an accessible action.
#[repr(u32)]
#[derive(PartialEq, Clone)]
pub enum AccessibilityAction {
    Default,
    Decrement,
    Increment,
    /// This is currently unused
    ReplaceSelectedText(SharedString),
    SetValue(SharedString),
}

bitflags! {
    /// Define a accessibility actions that supported by an item.
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct SupportedAccessibilityAction: u32 {
        const Default = 1;
        const Decrement = 1 << 1;
        const Increment = 1 << 2;
        const ReplaceSelectedText = 1 << 3;
        const SetValue = 1 << 4;
    }
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
