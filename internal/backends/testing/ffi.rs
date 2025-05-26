// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::{ElementHandle, ElementRoot};
use i_slint_core::item_tree::ItemTreeRc;
use i_slint_core::slice::Slice;
use i_slint_core::{SharedString, SharedVector};
use std::os::raw::c_void;

struct RootWrapper<'a>(&'a ItemTreeRc);

impl ElementRoot for RootWrapper<'_> {
    fn item_tree(&self) -> ItemTreeRc {
        self.0.clone()
    }
}

impl super::Sealed for RootWrapper<'_> {}

#[unsafe(no_mangle)]
pub extern "C" fn slint_testing_init_backend() {
    crate::init_integration_test_with_mock_time();
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_testing_element_visit_elements(
    root: &ItemTreeRc,
    user_data: *mut c_void,
    visitor: unsafe extern "C" fn(*mut c_void, &ElementHandle) -> bool,
) -> bool {
    RootWrapper(root)
        .root_element()
        .query_descendants()
        .match_predicate(move |element| visitor(user_data, element))
        .find_first()
        .is_some()
}

#[unsafe(no_mangle)]
pub extern "C" fn slint_testing_element_find_by_accessible_label(
    root: &ItemTreeRc,
    label: &Slice<u8>,
    out: &mut SharedVector<ElementHandle>,
) {
    let Ok(label) = core::str::from_utf8(label.as_slice()) else { return };
    out.extend(ElementHandle::find_by_accessible_label(&RootWrapper(root), label))
}

#[unsafe(no_mangle)]
pub extern "C" fn slint_testing_element_find_by_element_id(
    root: &ItemTreeRc,
    element_id: &Slice<u8>,
    out: &mut SharedVector<ElementHandle>,
) {
    let Ok(element_id) = core::str::from_utf8(element_id.as_slice()) else { return };
    out.extend(ElementHandle::find_by_element_id(&RootWrapper(root), element_id));
}

#[unsafe(no_mangle)]
pub extern "C" fn slint_testing_element_find_by_element_type_name(
    root: &ItemTreeRc,
    type_name: &Slice<u8>,
    out: &mut SharedVector<ElementHandle>,
) {
    let Ok(type_name) = core::str::from_utf8(type_name.as_slice()) else { return };
    out.extend(ElementHandle::find_by_element_type_name(&RootWrapper(root), type_name));
}

#[unsafe(no_mangle)]
pub extern "C" fn slint_testing_element_id(
    element: &ElementHandle,
    out: &mut SharedString,
) -> bool {
    if let Some(id) = element.id() {
        *out = id;
        true
    } else {
        false
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn slint_testing_element_type_name(
    element: &ElementHandle,
    out: &mut SharedString,
) -> bool {
    if let Some(type_name) = element.type_name() {
        *out = type_name;
        true
    } else {
        false
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn slint_testing_element_bases(
    element: &ElementHandle,
    out: &mut SharedVector<SharedString>,
) -> bool {
    if let Some(bases_it) = element.bases() {
        out.extend(bases_it);
        true
    } else {
        false
    }
}
