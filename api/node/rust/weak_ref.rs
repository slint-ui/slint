// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Safe wrappers for NAPI weak references.
//!
//! napi-rs doesn't expose refcount=0 references or a way to get an `Object`
//! back from a `WeakReference<T>`.
//! This module fills those gaps and confines all `unsafe` in one place.

use napi::bindgen_prelude::*;
use napi::{Env, JsValue, Result};

/// Create a [`WeakReference<T>`] from a JS `Object` that wraps a NAPI class.
///
/// The returned weak reference doesn't prevent garbage collection.
/// Use [`weak_ref_get_object`] to retrieve the JS object later.
pub fn weak_ref_from_object<T: 'static + TypeTag>(
    env: &Env,
    obj: &Object<'_>,
) -> Result<WeakReference<T>> {
    // Safety: same napi_unwrap + napi_reference_ref that napi-rs generates
    // in #[napi] method bindings.
    let this_ref: Reference<T> = unsafe { Reference::from_napi_value(env.raw(), obj.raw())? };
    let weak = this_ref.downgrade();
    drop(this_ref);
    Ok(weak)
}

/// Retrieve the JS `Object` from a `WeakReference<T>`.
///
/// Returns `None` if the instance was already garbage-collected.
/// Internally upgrades the weak reference for the duration of the call,
/// then drops it so the ref-count returns to its previous value.
pub fn weak_ref_get_object<T: 'static>(weak: &WeakReference<T>, env: Env) -> Option<Object<'_>> {
    let reference = weak.upgrade(env).ok()??;
    // Safety: ToNapiValue extracts the raw JS handle from the Reference.
    let raw = unsafe { ToNapiValue::to_napi_value(env.raw(), reference) }.ok()?;
    // Safety: the raw handle is valid for the current scope.
    unsafe { Object::from_napi_value(env.raw(), raw) }.ok()
}

/// Weak reference to a plain JS value (not a NAPI class).
///
/// napi-rs's [`WeakReference<T>`] only works with `#[napi]` classes.
/// This type uses the raw NAPI C API to create a refcount=0 reference
/// for arbitrary JS values like model objects.
///
/// The reference doesn't prevent garbage collection.
/// If V8 collects the value,
/// [`get_unknown`](WeakValueRef::get_unknown) returns `None`.
pub struct WeakValueRef {
    raw_ref: napi::sys::napi_ref,
    raw_env: napi::sys::napi_env,
}

impl WeakValueRef {
    /// Create a weak reference to any JS value.
    pub fn new<'a, T: JsValue<'a>>(env: &Env, value: &T) -> Result<Self> {
        let raw_env = env.raw();
        let raw_val = value.raw();
        let mut raw_ref = std::ptr::null_mut();
        // Safety: raw_env and raw_val are valid handles from napi-rs.
        let status = unsafe { napi::sys::napi_create_reference(raw_env, raw_val, 0, &mut raw_ref) };
        if status != napi::sys::Status::napi_ok {
            return Err(napi::Error::new(
                napi::Status::from(status),
                "Failed to create weak reference",
            ));
        }
        Ok(Self { raw_ref, raw_env })
    }

    /// Get the value back,
    /// or `None` if it was garbage-collected.
    pub fn get_unknown(&self) -> Option<Unknown<'_>> {
        let mut value = std::ptr::null_mut();
        // Safety: raw_env and raw_ref are valid (created in new, deleted in drop).
        let status =
            unsafe { napi::sys::napi_get_reference_value(self.raw_env, self.raw_ref, &mut value) };
        if status != napi::sys::Status::napi_ok || value.is_null() {
            return None;
        }
        // Safety: value is a live JS handle from the reference.
        unsafe { Unknown::from_napi_value(self.raw_env, value) }.ok()
    }
}

impl Drop for WeakValueRef {
    fn drop(&mut self) {
        // Safety: matches the napi_create_reference in new.
        unsafe {
            napi::sys::napi_delete_reference(self.raw_env, self.raw_ref);
        }
    }
}
