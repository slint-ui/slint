// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![allow(unsafe_code)]

use super::DataTransfer;
use crate::SharedString;
use crate::api::Image;

/// Opaque placeholder used by C++ to reserve storage with the same size and
/// alignment as Rust's `DataTransfer`. The actual `DataTransfer` contains
/// `Option<Rc<...>>` fields whose layout cannot be expressed via cbindgen, so
/// C++ never inspects these fields directly: copy/destruction goes through
/// the `slint_data_transfer_*` FFI functions below, which operate on a real
/// `DataTransfer`.
///
/// The three pointer-sized fields correspond to:
/// - `_rc_inner`: thin `Rc` pointer for `Option<Rc<DataTransferInner>>` (null = `None`)
/// - `_rc_any_0`/`_rc_any_1`: data + vtable pointers of the
///   `Option<Rc<dyn Any>>` user data fat pointer (null data = `None`)
#[repr(C)]
pub struct DataTransferOpaque {
    _rc_inner: *mut core::ffi::c_void,
    _rc_any_0: *mut core::ffi::c_void,
    _rc_any_1: *mut core::ffi::c_void,
}

static_assertions::assert_eq_align!(DataTransferOpaque, DataTransfer);
static_assertions::assert_eq_size!(DataTransferOpaque, DataTransfer);

/// Default-construct a `DataTransfer` in place at `out`.
///
/// # Safety
/// `out` must be valid for writes of `DataTransfer` and must not currently
/// hold an initialized `DataTransfer` (otherwise the previous value is leaked).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_data_transfer_init_default(out: *mut DataTransfer) {
    unsafe { core::ptr::write(out, DataTransfer::default()) }
}

/// Drop a `DataTransfer` in place.
///
/// # Safety
/// `d` must point to an initialized `DataTransfer` and must not be used after
/// this call returns.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_data_transfer_drop(d: *mut DataTransfer) {
    unsafe { core::ptr::drop_in_place(d) }
}

/// Clone `src` into the uninitialized memory at `out`.
///
/// # Safety
/// `out` must be valid for writes of `DataTransfer` and must not currently
/// hold an initialized `DataTransfer` (otherwise the previous value is leaked).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_data_transfer_clone(out: *mut DataTransfer, src: &DataTransfer) {
    unsafe { core::ptr::write(out, src.clone()) }
}

/// Compare two `DataTransfer` values for equality (same semantics as
/// `<DataTransfer as PartialEq>::eq`).
#[unsafe(no_mangle)]
pub extern "C" fn slint_data_transfer_eq(a: &DataTransfer, b: &DataTransfer) -> bool {
    a == b
}

/// Set the plaintext representation of `d` to a clone of `text`.
#[unsafe(no_mangle)]
pub extern "C" fn slint_data_transfer_set_plaintext(d: &mut DataTransfer, text: &SharedString) {
    d.set_plaintext(text.clone());
}

/// Set the image representation of `d` to a clone of `image`.
#[unsafe(no_mangle)]
pub extern "C" fn slint_data_transfer_set_image(d: &mut DataTransfer, image: &Image) {
    d.set_image(image.clone());
}

/// Returns `true` if `d` advertises a plaintext representation.
#[unsafe(no_mangle)]
pub extern "C" fn slint_data_transfer_has_plaintext(d: &DataTransfer) -> bool {
    d.has_plaintext()
}

/// Returns `true` if `d` advertises an image representation.
#[unsafe(no_mangle)]
pub extern "C" fn slint_data_transfer_has_image(d: &DataTransfer) -> bool {
    d.has_image()
}

/// If `d` has a plaintext representation, write a clone of it into `out` and
/// return `true`. Otherwise leave `out` unchanged and return `false`.
///
/// `out` must point to an initialized `SharedString`.
#[unsafe(no_mangle)]
pub extern "C" fn slint_data_transfer_fetch_plaintext(
    d: &DataTransfer,
    out: &mut SharedString,
) -> bool {
    match d.fetch_plaintext() {
        Ok(s) => {
            *out = s;
            true
        }
        Err(_) => false,
    }
}

/// If `d` has an image representation, write a clone of it into `out` and
/// return `true`. Otherwise leave `out` unchanged and return `false`.
///
/// `out` must point to an initialized `Image`.
#[unsafe(no_mangle)]
pub extern "C" fn slint_data_transfer_fetch_image(d: &DataTransfer, out: &mut Image) -> bool {
    match d.fetch_image() {
        Ok(i) => {
            *out = i;
            true
        }
        Err(_) => false,
    }
}
