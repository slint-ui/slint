// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::DataTransfer;

/// Completely incorrect to the actual layout of `DataTransfer`, but
/// this at least makes the size and alignment correct. This does _not_
/// copy/move correctly in C++, but due to use of trait objects in
/// `DataTransfer` we cannot bindgen it directly.
// TODO: Implement this correcly.
#[repr(C)]
pub struct DataTransferOpaque {
    _rc_inner: *mut core::ffi::c_void,
    _rc_any_0: *mut core::ffi::c_void,
    _rc_any_1: *mut core::ffi::c_void,
}

static_assertions::assert_eq_align!(DataTransferOpaque, DataTransfer);
static_assertions::assert_eq_size!(DataTransferOpaque, DataTransfer);
