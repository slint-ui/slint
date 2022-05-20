// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

// cSpell: ignore descendents

use crate::items::ItemRc;
use alloc::vec::Vec;

#[repr(C)]
#[derive(PartialEq, Eq, Copy, Clone, strum::Display)]
#[strum(serialize_all = "kebab-case")]
pub enum AccessibleStringProperty {
    Label,
    DelegateFocus,
    Description,
    Checked,
    Value,
    ValueMinimum,
    ValueMaximum,
    ValueStep,
}

/// Find accessible descendents of `root_item`.
///
/// This will recurse through all children of `root_item`, but will not recurse
/// into nodes that are accessible.
pub fn accessible_descendents(root_item: &ItemRc) -> Vec<ItemRc> {
    let mut result = Vec::new();

    // Do not look on the root_item: That is either a component root or an
    // accessible item already handled!
    let mut child = root_item.first_child();
    while let Some(c) = &child {
        if c.is_accessible() {
            result.push(c.clone());
        } else {
            result.append(&mut accessible_descendents(c))
        }
        child = c.next_sibling();
    }

    result
}
