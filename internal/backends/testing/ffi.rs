// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_core::item_tree::ItemTreeRc;
use i_slint_core::slice::Slice;
use i_slint_core::{SharedString, SharedVector};

#[no_mangle]
pub extern "C" fn slint_testing_init_backend() {
    crate::init_integration_test();
}

#[no_mangle]
pub extern "C" fn slint_testing_element_find_by_accessible_label(
    root: &ItemTreeRc,
    label: &Slice<u8>,
    out: &mut SharedVector<crate::search_api::ElementHandle>,
) {
    let Ok(label) = core::str::from_utf8(label.as_slice()) else { return };
    *out = crate::search_api::search_item(root, |elem| {
        elem.accessible_label().is_some_and(|x| x == label)
    })
}

#[no_mangle]
pub extern "C" fn slint_testing_element_find_by_element_id(
    root: &ItemTreeRc,
    element_id: &Slice<u8>,
    out: &mut SharedVector<crate::search_api::ElementHandle>,
) {
    let Ok(element_id) = core::str::from_utf8(element_id.as_slice()) else { return };
    *out = crate::search_api::search_item(root, |elem| {
        elem.element_type_names_and_ids().unwrap().any(|(_, eid)| eid == element_id)
    })
}

#[no_mangle]
pub extern "C" fn slint_testing_element_type_names_and_ids(
    element: &crate::search_api::ElementHandle,
    type_names: &mut SharedVector<SharedString>,
    ids: &mut SharedVector<SharedString>,
) {
    if let Some(it) = element.element_type_names_and_ids() {
        for (type_name, id) in it {
            type_names.push(type_name);
            ids.push(id);
        }
    }
}
