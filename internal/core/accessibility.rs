// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.0 OR LicenseRef-Slint-commercial

// cSpell: ignore descendents

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
pub fn accessible_descendents(root_item: &ItemRc, descendents: &mut impl Extend<ItemRc>) {
    // Do not look on the root_item: That is either a component root or an
    // accessible item already handled!
    let mut child = root_item.first_child();
    while let Some(c) = &child {
        if c.is_accessible() {
            descendents.extend(core::iter::once(c.clone()));
        } else {
            accessible_descendents(c, descendents)
        }
        child = c.next_sibling();
    }
}
