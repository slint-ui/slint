// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore descendents

use crate::{
    SharedString,
    item_tree::ItemTreeVTable,
    items::{ItemRc, TextInput},
};
use alloc::vec::Vec;
use bitflags::bitflags;
use vtable::VRcMapped;

/// The property names of the accessible-properties
#[repr(u32)]
#[derive(PartialEq, Eq, Copy, Clone, strum::Display)]
#[strum(serialize_all = "kebab-case")]
pub enum AccessibleStringProperty {
    Checkable,
    Checked,
    DelegateFocus,
    Description,
    Enabled,
    Expandable,
    Expanded,
    Id,
    ItemCount,
    ItemIndex,
    ItemSelectable,
    ItemSelected,
    Label,
    LiveRegion,
    Orientation,
    PlaceholderText,
    ReadOnly,
    Value,
    ValueMaximum,
    ValueMinimum,
    ValueStep,
}

/// The argument of an accessible action.
///
/// Every variant mirrors one of the `accessible-action-*` callbacks declared in the compiler's
/// type register, with one (unnamed) field per callback argument. The generated code relies on
/// that: it binds the fields positionally, so a new action only needs to be added here and in
/// the type register.
#[repr(u32)]
#[derive(PartialEq, Clone)]
pub enum AccessibilityAction {
    Default,
    Decrement,
    Increment,
    Expand,
    /// This is currently unused
    ReplaceSelectedText(SharedString),
    SetValue(SharedString),
    /// Select the text between two UTF-8 offsets into the element's text: the first offset is the
    /// anchor, the end that stays put, the second one the focus, the end being moved.
    SetSelection(i32, i32),
}

bitflags! {
    /// Define a accessibility actions that supported by an item.
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct SupportedAccessibilityAction: u32 {
        const Default = 1;
        const Decrement = 1 << 1;
        const Increment = 1 << 2;
        const Expand = 1 << 3;
        const ReplaceSelectedText = 1 << 4;
        const SetValue = 1 << 5;
        const SetSelection = 1 << 6;
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

    core::iter::from_fn(move || {
        loop {
            let candidate = descendent_candidates.pop()?;

            if let Some(next_candidate) = candidate.next_sibling() {
                descendent_candidates.push(next_candidate);
            }

            if let Some(descendent) = try_candidate_or_find_next_accessible_descendent(
                candidate,
                &mut descendent_candidates,
            ) {
                return Some(descendent);
            }
        }
    })
}

/// Find the first built-in `TextInput` in `item` or its descendents.
pub fn find_text_input(item: &ItemRc) -> Option<VRcMapped<ItemTreeVTable, TextInput>> {
    find_text_input_with_rc(item).map(|(_, input)| input)
}

/// Same as [`find_text_input`], but also returns the `TextInput`'s `ItemRc`.
pub fn find_text_input_with_rc(
    item: &ItemRc,
) -> Option<(ItemRc, VRcMapped<ItemTreeVTable, TextInput>)> {
    if let Some(input) = item.clone().downcast::<TextInput>() {
        return Some((item.clone(), input));
    }

    let mut child = item.first_child();
    while let Some(candidate) = child {
        child = candidate.next_sibling();
        if let Some(found) = find_text_input_with_rc(&candidate) {
            return Some(found);
        }
    }
    None
}
