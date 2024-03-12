// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

// cSpell: ignore descendents

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use crate::{
    items::ItemRc,
    lengths::{LogicalPoint, LogicalVector},
    SharedString,
};

use bitflags::bitflags;

// The property names of the accessible-properties
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

// Defines an accessibility action.
pub enum AccessibilityAction {
    Default,
    Focus,
    Blur,
    Collapse,
    Expand,
    CustomAction,
    Decrement,
    Increment,
    ReplaceSelectedText(SharedString),
    ScrollBackward,
    ScrollDown,
    ScrollForward,
    ScrollLeft,
    ScrollRight,
    ScrollUp,
    ScrollIntoView,
    ScrollToPoint(LogicalPoint),
    SetScrollOffset(LogicalVector),
    SetTextSelection(Option<core::ops::Range<i32>>),
    SetValue(f64)
}

bitflags! {
    // Define a accessibility actions that currently supported by Slint.
    pub struct SupportedAccessibilityAction: u32 {
        const Default = 1;
        const Focus = 1 << 1;
        const Blur = 1 << 2;
        const Collapse = 1 << 4;
        const Expand = 1 << 5;
        const CustomAction = 1 << 6;
        const Decrement = 1 << 7;
        const Increment = 1 << 8;
        const ReplaceSelectedText = 1 << 9;
        const ScrollBackward = 1 << 10;
        const ScrollDown = 1 << 11;
        const ScrollForward = 1 << 12;
        const ScrollLeft = 1 << 13;
        const ScrollRight = 1 << 14;
        const ScrollUp = 1 << 15;
        const ScrollIntoView = 1 << 16;
        const ScrollToPoint = 1 << 17;
        const SetScrollOffset = 1 << 18;
        const SetTextSelection = 1 << 19;
        const SetValue = 1 << 20;
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
