// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.2 OR LicenseRef-Slint-commercial

use i_slint_core::accessibility::AccessibleStringProperty;
use i_slint_core::item_tree::{ItemTreeRc, ItemWeak};
use i_slint_core::slice::Slice;
use i_slint_core::SharedVector;

#[no_mangle]
pub extern "C" fn slint_testing_init_backend() {
    crate::init_integration_test();
}

#[no_mangle]
pub extern "C" fn slint_testing_element_find_by_accessible_label(
    root: &ItemTreeRc,
    label: &Slice<u8>,
    out: &mut SharedVector<ItemWeak>,
) {
    let Ok(label) = core::str::from_utf8(label.as_slice()) else { return };
    *out = crate::search_api::search_item(root, |item| {
        item.accessible_string_property(AccessibleStringProperty::Label).is_some_and(|x| x == label)
    })
}
