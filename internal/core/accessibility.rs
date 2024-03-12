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
    // FIXME: Can it be currently supported?
    HideToolTip,
    // FIXME: Can it be currently supported?
    ShowToolTip,
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
    // FIXME: Do we need this?
    SetSequentialFocusNavigationStartingPoint,
    SetValue(f64),
    // FIXME: Can it be currently supported?
    ShowContextMenu,
}

bitflags! {
    // Define a accessibility actions that currently supported by Slint.
    pub struct SupportedAccessibilityAction: u32 {
        const Default = 0b00000001;
        const Focus = 0b00000010;
        const Blur = 0b00000011;
        const Collapse = 0b00000100;
        const Expand = 0b00000110;
        const CustomAction = 0b00000111;
        const Decrement = 0b00001000;
        const Increment = 0b00001001;
        const HideToolTip = 0b00001010;
        const ShowToolTip = 0b00001011;
        const ReplaceSelectedText = 0b00001100;
        const ScrollBackward = 0b00001101;
        const ScrollDown = 0b00001110;
        const ScrollForward = 0b00001110;
        const ScrollLeft = 0b00001111;
        const ScrollRight = 0b00010000;
        const ScrollUp = 0b00010001;
        const ScrollIntoView = 0b00010010;
        const ScrollToPoint = 0b00010011;
        const SetScrollOffset = 0b00010100;
        const SetTextSelection = 0b00010101;
        const SetSequentialFocusNavigationStartingPoint = 0b00010110;
        const SetValue = 0b00010111;
        const ShowContextMenu = 0b00011000;
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
