// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![allow(unsafe_code)]

use alloc::rc::Rc;
use core::ffi::c_void;

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

/// Set the plain text representation of `d` to a clone of `text`.
///
/// An empty `text` clears the previously-set plain text instead of storing it.
#[unsafe(no_mangle)]
pub extern "C" fn slint_data_transfer_set_plain_text(d: &mut DataTransfer, text: &SharedString) {
    d.set_plain_text(text.clone());
}

/// Set the image representation of `d` to a clone of `image`.
///
/// A default-constructed `image` clears the previously-set image instead of storing it.
#[unsafe(no_mangle)]
pub extern "C" fn slint_data_transfer_set_image(d: &mut DataTransfer, image: &Image) {
    d.set_image(image.clone());
}

/// Returns `true` if `d` advertises a plain text representation.
#[unsafe(no_mangle)]
pub extern "C" fn slint_data_transfer_has_plain_text(d: &DataTransfer) -> bool {
    d.has_plain_text()
}

/// Returns `true` if `d` advertises an image representation.
#[unsafe(no_mangle)]
pub extern "C" fn slint_data_transfer_has_image(d: &DataTransfer) -> bool {
    d.has_image()
}

/// Returns `true` if `d` carries no data: no plain text, no image, and no user data.
#[unsafe(no_mangle)]
pub extern "C" fn slint_data_transfer_is_empty(d: &DataTransfer) -> bool {
    d.is_empty()
}

/// If `d` has a plain text representation, write a clone of it into `out` and
/// return `true`. Otherwise leave `out` unchanged and return `false`.
///
/// `out` must point to an initialized `SharedString`.
#[unsafe(no_mangle)]
pub extern "C" fn slint_data_transfer_plain_text(d: &DataTransfer, out: &mut SharedString) -> bool {
    match d.plain_text() {
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
pub extern "C" fn slint_data_transfer_image(d: &DataTransfer, out: &mut Image) -> bool {
    match d.image() {
        Ok(i) => {
            *out = i;
            true
        }
        Err(_) => false,
    }
}

/// C++-owned user-data handle stored in a `DataTransfer`.
struct CppUserData {
    handle: *mut c_void,
    drop_fn: unsafe extern "C" fn(*mut c_void),
}

impl Drop for CppUserData {
    fn drop(&mut self) {
        unsafe { (self.drop_fn)(self.handle) }
    }
}

/// Store `handle` as the user data of `d`. `drop_fn(handle)` runs when `d` is dropped.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_data_transfer_set_user_data(
    d: &mut DataTransfer,
    handle: *mut c_void,
    drop_fn: unsafe extern "C" fn(*mut c_void),
) {
    d.set_user_data(Rc::new(CppUserData { handle, drop_fn }));
}

/// Write a borrowed pointer to `d`'s C++ user-data handle into `out_handle`.
/// Returns `false` if `d` has no C++ user data.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_data_transfer_user_data(
    d: &DataTransfer,
    out_handle: *mut *const c_void,
) -> bool {
    let Some(any) = d.user_data() else { return false };
    let Some(cpp) = any.downcast_ref::<CppUserData>() else { return false };
    unsafe { *out_handle = cpp.handle as *const c_void };
    true
}

/// Clear the user data of `d`, if any.
#[unsafe(no_mangle)]
pub extern "C" fn slint_data_transfer_clear_user_data(d: &mut DataTransfer) {
    d.user_data = None;
}
