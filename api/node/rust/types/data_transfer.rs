// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::any::Any;
use std::cell::RefCell;
use std::rc::Rc;

use i_slint_core::data_transfer::DataTransfer;
use napi::bindgen_prelude::{JsObjectValue, Object, This, ToNapiValue, Unknown, ValueType};
use napi::{Env, JsValue};

use crate::types::SlintImageData;
use crate::weak_ref::WeakValueRef;

/// Hidden property that anchors the JS user_data value on a `DataTransfer`
/// JS instance. Storing it on the JS object (instead of via a strong NAPI
/// reference) lets V8 see the connection and collect cycles between the
/// transfer and its user_data.
const USER_DATA_PROP: &str = "__slint_user_data";

/// Anchor installed on an owning JS object (e.g. a `JsComponentInstance`
/// holding a Slint property of type `data-transfer`) so the JS user_data
/// stays reachable from V8 even after the originating `DataTransfer`
/// wrapper is collected. Removed in [`JsUserData::drop`] when the last
/// `Rc<JsUserData>` clone goes away, releasing the payload for GC.
struct Anchor {
    key: String,
    owner: crate::JsAnchorOwner,
}

/// What we put inside `DataTransfer.user_data` when set from JavaScript.
///
/// Holds a [`WeakValueRef`] (refcount=0) to the JS value. The value is
/// kept alive by hidden JS properties — at minimum the [`USER_DATA_PROP`]
/// on the originating `DataTransfer` instance, plus any [`Anchor`]s added
/// when the transfer is stored where the original wrapper might outlive
/// it (e.g. a Slint property on a `JsComponentInstance`).
struct JsUserData {
    weak: WeakValueRef,
    env: Env,
    /// Anchors on JS objects beyond the originating `DataTransfer` wrapper.
    /// The wrapper's own anchor is auto-cleaned when V8 collects it,
    /// so it isn't tracked here.
    extra_anchors: RefCell<Vec<Anchor>>,
}

impl Drop for JsUserData {
    fn drop(&mut self) {
        for anchor in self.extra_anchors.borrow().iter() {
            if anchor.owner.seq.upgrade().is_none() {
                continue;
            }
            if let Some(mut obj) = crate::weak_ref::weak_ref_get_object::<crate::JsComponentInstance>(
                &anchor.owner.owner_weak,
                self.env,
            ) {
                let _ = obj.delete_named_property(&anchor.key);
            }
        }
    }
}

/// Represents some form of type-indexed possibly-lazy data transfer.
///
/// Used for accessing the platform clipboard and drag-and-drop APIs.
#[napi(js_name = "DataTransfer")]
pub struct SlintDataTransfer {
    pub(crate) inner: DataTransfer,
}

impl From<DataTransfer> for SlintDataTransfer {
    fn from(inner: DataTransfer) -> Self {
        Self { inner }
    }
}

#[napi]
impl SlintDataTransfer {
    /// Constructs an empty `DataTransfer`.
    #[napi(constructor)]
    pub fn new() -> Self {
        Self { inner: DataTransfer::default() }
    }

    /// The plain text representation of this `DataTransfer`, or `null` if no
    /// plain text is available.
    #[napi(getter)]
    pub fn plain_text(&self) -> Option<String> {
        self.inner.plain_text().ok().map(|s| s.to_string())
    }

    /// Sets the plain text representation of this `DataTransfer`. Assigning
    /// `null`, `undefined`, or the empty string clears any previously-set
    /// plain text; assigning any other string overwrites it.
    #[napi(setter)]
    pub fn set_plain_text(&mut self, text: Option<String>) {
        self.inner.set_plain_text(text.unwrap_or_default().into());
    }

    /// `true` if this `DataTransfer` advertises a plain text representation.
    #[napi(getter)]
    pub fn has_plain_text(&self) -> bool {
        self.inner.has_plain_text()
    }

    /// The image representation of this `DataTransfer`, or `null` if no
    /// image is available.
    #[napi(getter)]
    pub fn image(&self) -> Option<SlintImageData> {
        self.inner.image().ok().map(SlintImageData::from)
    }

    /// Sets the image representation of this `DataTransfer`. Assigning `null`
    /// or `undefined` clears any previously-set image; assigning any other
    /// image overwrites it.
    #[napi(setter)]
    pub fn set_image(&mut self, image: Option<&SlintImageData>) {
        self.inner.set_image(image.map(|i| i.inner.clone()).unwrap_or_default());
    }

    /// `true` if this `DataTransfer` advertises an image representation.
    #[napi(getter)]
    pub fn has_image(&self) -> bool {
        self.inner.has_image()
    }

    /// `true` if this `DataTransfer` carries no data: no plain text, no image, and no
    /// user data.
    #[napi(getter)]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Application-internal user data attached to this `DataTransfer`. Use this
    /// when the drag-and-drop or clipboard operation stays inside the current
    /// JavaScript application and you want to avoid serializing to plain text or
    /// an image.
    ///
    /// Reading returns the JavaScript value previously assigned, or `null` if
    /// none was set (or the user data was set by a non-JavaScript binding).
    /// Assigning `null` or `undefined` clears any previously attached JS user
    /// data.
    #[napi(getter)]
    pub fn user_data<'env>(&self, env: &'env Env) -> napi::Result<Unknown<'env>> {
        let Some(js) = self.inner.user_data().and_then(|rc| rc.downcast::<JsUserData>().ok())
        else {
            return napi::bindgen_prelude::Null.into_unknown(env);
        };
        match js.weak.get_unknown() {
            Some(v) => v.into_unknown(env),
            None => napi::bindgen_prelude::Null.into_unknown(env),
        }
    }

    #[napi(setter)]
    pub fn set_user_data(
        &mut self,
        env: &Env,
        mut this: This<Object<'_>>,
        value: Unknown<'_>,
    ) -> napi::Result<()> {
        let value_type = value.get_type()?;
        if matches!(value_type, ValueType::Null | ValueType::Undefined) {
            // The underlying field is private; install a sentinel that fails
            // the `JsUserData` downcast so the property reads back as null.
            self.inner.set_user_data(Rc::new(()) as Rc<dyn Any>);
            let _ = this.object.delete_named_property(USER_DATA_PROP);
            return Ok(());
        }
        let object = value.coerce_to_object()?;
        let weak = WeakValueRef::new(env, &object)?;
        // Anchor the JS value on `this` so V8 sees the reference graph and
        // can collect cycles between the transfer and its user_data.
        crate::set_hidden_property(&mut this.object, USER_DATA_PROP, &object)?;
        self.inner.set_user_data(Rc::new(JsUserData {
            weak,
            env: *env,
            extra_anchors: RefCell::new(Vec::new()),
        }) as Rc<dyn Any>);
        Ok(())
    }

    /// Returns `true` if this `DataTransfer` equals `other`. Two transfers
    /// compare equal when one is an unmodified clone of the other; any
    /// modification (including overwriting plain text, image, or user data with
    /// the same value) makes them unequal.
    #[napi]
    pub fn equals(&self, other: &SlintDataTransfer) -> bool {
        self.inner == other.inner
    }
}

impl SlintDataTransfer {
    /// If this `DataTransfer` carries JS user_data, anchor it as a hidden
    /// property on `instance` so V8 keeps it alive while `instance` is
    /// reachable. Used when handing a Rust [`DataTransfer`] back to
    /// JavaScript so the user_data stays accessible from clones.
    pub(crate) fn anchor_js_user_data(&self, instance: &mut Object<'_>) -> napi::Result<()> {
        let Some(js) = self.inner.user_data().and_then(|rc| rc.downcast::<JsUserData>().ok())
        else {
            return Ok(());
        };
        let Some(value) = js.weak.get_unknown() else { return Ok(()) };
        crate::set_hidden_property(instance, USER_DATA_PROP, &value)
    }

    /// Pin this transfer's JS user_data on `owner` so it stays reachable
    /// from V8 while `owner` is alive — necessary when a `DataTransfer`
    /// is handed to a Slint property and the originating JS wrapper may
    /// be collected before the property is read again. The anchor is
    /// removed by [`JsUserData::drop`] once no `DataTransfer` clone holds
    /// the user_data anymore.
    pub(crate) fn pin_user_data_on(
        &self,
        env: &Env,
        owner: &crate::JsAnchorOwner,
    ) -> napi::Result<()> {
        let Some(js) = self.inner.user_data().and_then(|rc| rc.downcast::<JsUserData>().ok())
        else {
            return Ok(());
        };
        {
            let mut anchors = js.extra_anchors.borrow_mut();
            anchors.retain(|a| a.owner.seq.upgrade().is_some());
            if anchors.iter().any(|a| std::rc::Weak::ptr_eq(&a.owner.seq, &owner.seq)) {
                return Ok(());
            }
        }
        let Some(value) = js.weak.get_unknown() else { return Ok(()) };
        let Some(mut owner_obj) = crate::weak_ref::weak_ref_get_object::<crate::JsComponentInstance>(
            &owner.owner_weak,
            *env,
        ) else {
            return Ok(());
        };
        let key = format!("__slint_dt_user_data#{}", owner.next_anchor_id());
        crate::set_hidden_property(&mut owner_obj, &key, &value)?;
        js.extra_anchors.borrow_mut().push(Anchor { key, owner: owner.clone() });
        Ok(())
    }
}
