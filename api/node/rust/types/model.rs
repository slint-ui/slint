// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::pin::Pin;
use std::rc::Rc;

use core::result::Result as CoreResult;
use i_slint_compiler::langtype::Type;
use i_slint_core::model::{
    FilterModel, MapModel, Model, ModelChangeListener, ModelChangeListenerBox, ModelError,
    ModelNotify, ModelRc, ModelTracker, ReverseModel, SortModel,
};
use napi::bindgen_prelude::*;
use napi::{Env, JsValue, Result, ValueType};

use crate::weak_ref::WeakValueRef;
use crate::{JsAnchorOwner, to_js_unknown, to_value};

#[napi]
#[derive(Clone, Default)]
pub struct SharedModelNotify(Rc<ModelNotify>);

impl core::ops::Deref for SharedModelNotify {
    type Target = Rc<ModelNotify>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Take the pending JavaScript exception off the environment, so that later napi
/// calls succeed and the exception doesn't surface when the runtime returns to JS.
fn take_exception(env: &Env) {
    let mut exception = std::ptr::null_mut();
    // SAFETY: plain FFI call, napi-rs has no safe wrapper for it. The env is the valid
    // environment of the calling thread and the out-pointer is a valid local; the
    // exception value written to it stays owned by the VM and is discarded.
    unsafe { napi::sys::napi_get_and_clear_last_exception(env.raw(), &mut exception) };
}

/// Reports a glue-level error to `console.error`. Clears any pending JS exception
/// first, so the console call works and the exception doesn't surface in JS.
fn report_glue_error(env: Env, message: core::fmt::Arguments<'_>) {
    take_exception(&env);
    crate::print_to_console(env, "error", message);
}

/// An unsupported ModelError naming the JavaScript class of the model, falling
/// back to the name of the RawJsModel wrapper. Clears the exception thrown to signal
/// the rejection, so the class-name lookup and later napi calls work.
fn unsupported_error(js_model: &RawJsModel, model: &Object) -> ModelError {
    take_exception(&js_model.env);
    let class_name = || -> Option<String> {
        // The constructor is a JS function; fetch it untyped and coerce, a typed
        // `get_named_property::<Object>` rejects functions.
        let constructor: Unknown = model.get_named_property("constructor").ok()?;
        let constructor = constructor.coerce_to_object().ok()?;
        let name: String = constructor.get_named_property("name").ok()?;
        (!name.is_empty()).then_some(name)
    };
    match class_name() {
        Some(name) => ModelError::unsupported_by_name(name, i_slint_core::InternalToken),
        None => ModelError::unsupported(js_model),
    }
}

/// A `Model::Data` that carries no lifetime: a raw NAPI handle bundled with the
/// env it belongs to. Instances are only ever produced and consumed within the
/// same synchronous JS -> Rust -> JS call chain, never stored across an
/// event-loop tick — the same validity contract `WeakValueRef` relies on.
#[derive(Clone, Copy)]
struct JsRawValue {
    env: sys::napi_env,
    value: sys::napi_value,
}

/// Detects whether `source` is backed by a native adapter (constructed by one
/// of the `native_*_model_new` factory functions below), via the hidden
/// `__slintNativeModel` property that the TS Reverse/Filter/Sort/MapModel
/// classes expose.
/// Lets chained adapters (e.g. `.filter().sort()`) reuse each other's native
/// backing directly instead of bouncing back through JS at every level.
fn native_model_of<'a>(source: &Object<'a>) -> Option<ClassInstance<'a, NativeModel>> {
    source.get::<ClassInstance<NativeModel>>("__slintNativeModel").ok().flatten()
}

/// Resolves an arbitrary JS `Model<T>`-shaped object to a `ModelRc<JsRawValue>`,
/// reusing its native backing when there is one instead of wrapping it again.
fn resolve_source(env: &Env, source: &Object) -> Result<ModelRc<JsRawValue>> {
    if let Some(native) = native_model_of(source) {
        return Ok((*native).inner.clone());
    }

    let shared_model_notify: ExternalRef<SharedModelNotify> =
        source.get_named_property("modelNotify")?;
    let shared_model_notify: SharedModelNotify = (*shared_model_notify).clone();

    Ok(Rc::new(RawJsModel {
        shared_model_notify,
        env: *env,
        js_impl: WeakValueRef::new(env, source)?,
    })
    .into())
}

/// Bridges an arbitrary JS `Model<T>` as a `Model<Data = JsRawValue>`, without
/// any `slint_interpreter::Value` conversion: native adapters stacked on top
/// only ever hand row data back to JS callbacks or pass it through untouched,
/// so no `Type` is needed until the one terminal point that assigns to a
/// `.slint` property (see `TerminalTypedModel`).
struct RawJsModel {
    shared_model_notify: SharedModelNotify,
    env: Env,
    js_impl: WeakValueRef,
}

impl RawJsModel {
    fn report_error(&self, message: core::fmt::Arguments<'_>) {
        report_glue_error(self.env, message);
    }
}

impl Model for RawJsModel {
    type Data = JsRawValue;

    fn row_count(&self) -> usize {
        let Some(model_unknown) = self.js_impl.get_unknown() else {
            self.report_error(format_args!(
                "Node.js: JavaScript Model<T>'s rowCount threw an exception"
            ));
            return 0;
        };
        let Ok(model) = model_unknown.coerce_to_object() else {
            self.report_error(format_args!("Node.js: JavaScript Model<T> is not an object"));
            return 0;
        };
        let row_count_fn: Function<(), Unknown> = match model.get_named_property("rowCount") {
            Ok(f) => f,
            Err(_) => {
                self.report_error(format_args!(
                    "Node.js: JavaScript Model<T> implementation is missing rowCount property"
                ));
                return 0;
            }
        };
        let Ok(row_count_result) = row_count_fn.apply(model, ()) else {
            self.report_error(format_args!(
                "Node.js: JavaScript Model<T>'s rowCount implementation call failed"
            ));
            return 0;
        };
        let Ok(row_count_number) = row_count_result.coerce_to_number() else {
            self.report_error(format_args!(
                "Node.js: JavaScript Model<T>'s rowCount function returned a value that cannot be coerced to a number"
            ));
            return 0;
        };
        let Ok(row_count) = row_count_number.get_uint32() else {
            self.report_error(format_args!(
                "Node.js: JavaScript Model<T>'s rowCount function returned a number that cannot be mapped to a uint32"
            ));
            return 0;
        };
        row_count as usize
    }

    fn row_data(&self, row: usize) -> Option<Self::Data> {
        let Some(model_unknown) = self.js_impl.get_unknown() else {
            self.report_error(format_args!(
                "Node.js: JavaScript Model<T>'s rowData threw an exception"
            ));
            return None;
        };
        let Ok(model) = model_unknown.coerce_to_object() else {
            self.report_error(format_args!("Node.js: JavaScript Model<T> is not an object"));
            return None;
        };
        let row_data_fn: Function<f64, Unknown> = match model.get_named_property("rowData") {
            Ok(f) => f,
            Err(_) => {
                self.report_error(format_args!(
                    "Node.js: JavaScript Model<T> implementation is missing rowData property"
                ));
                return None;
            }
        };
        let Ok(row_data) = row_data_fn.apply(model, row as f64) else {
            self.report_error(format_args!(
                "Node.js: JavaScript Model<T>'s rowData function threw an exception"
            ));
            return None;
        };
        (row_data.get_type().ok()? != ValueType::Undefined)
            .then(|| JsRawValue { env: self.env.raw(), value: row_data.raw() })
    }

    fn set_row_data(&self, row: usize, data: Self::Data) {
        let Some(model_unknown) = self.js_impl.get_unknown() else {
            self.report_error(format_args!(
                "Node.js: JavaScript Model<T>'s setRowData threw an exception"
            ));
            return;
        };
        let Ok(model) = model_unknown.coerce_to_object() else {
            self.report_error(format_args!("Node.js: JavaScript Model<T> is not an object"));
            return;
        };
        let set_row_data_fn: Function<FnArgs<(f64, Unknown<'_>)>, Unknown> =
            match model.get_named_property("setRowData") {
                Ok(f) => f,
                Err(_) => {
                    self.report_error(format_args!(
                        "Node.js: JavaScript Model<T> implementation is missing setRowData property"
                    ));
                    return;
                }
            };
        let js_data = unsafe { Unknown::from_raw_unchecked(data.env, data.value) };
        // A rejected modification is reported by throwing.
        if set_row_data_fn.apply(model, FnArgs::from((row as f64, js_data))).is_err() {
            let error = unsupported_error(self, &model);
            i_slint_core::debug_log!("setRowData(): {error}");
        }
    }

    fn push_row(&self, data: Self::Data) -> CoreResult<(), ModelError> {
        let Some(model_unknown) = self.js_impl.get_unknown() else {
            self.report_error(format_args!(
                "Node.js: JavaScript Model<T>'s pushRow threw an exception"
            ));
            return Err(ModelError::unsupported(self));
        };
        let Ok(model) = model_unknown.coerce_to_object() else {
            self.report_error(format_args!("Node.js: JavaScript Model<T> is not an object"));
            return Err(ModelError::unsupported(self));
        };
        let push_row_fn: Function<Unknown<'_>, Unknown> = match model.get_named_property("pushRow")
        {
            Ok(f) => f,
            Err(e) => {
                self.report_error(format_args!(
                    "Node.js: JavaScript Model<T> implementation is missing pushRow property: {e}"
                ));
                return Err(ModelError::unsupported(self));
            }
        };
        let js_data = unsafe { Unknown::from_raw_unchecked(data.env, data.value) };
        // A rejected modification is reported by throwing.
        match push_row_fn.apply(model, js_data) {
            Ok(_) => Ok(()),
            Err(_) => Err(unsupported_error(self, &model)),
        }
    }

    fn remove_row(&self, row: usize) -> CoreResult<(), ModelError> {
        let row_count = self.row_count();
        if row >= row_count {
            return Err(ModelError::out_of_bounds(row_count));
        }

        let Some(model_unknown) = self.js_impl.get_unknown() else {
            self.report_error(format_args!(
                "Node.js: JavaScript Model<T>'s removeRow threw an exception"
            ));
            return Err(ModelError::unsupported(self));
        };
        let Ok(model) = model_unknown.coerce_to_object() else {
            self.report_error(format_args!("Node.js: JavaScript Model<T> is not an object"));
            return Err(ModelError::unsupported(self));
        };
        let remove_row_fn: Function<f64, Unknown> = match model.get_named_property("removeRow") {
            Ok(f) => f,
            Err(e) => {
                self.report_error(format_args!(
                    "Node.js: JavaScript Model<T> implementation is missing removeRow property: {e}"
                ));
                return Err(ModelError::unsupported(self));
            }
        };
        // A rejected modification is reported by throwing.
        match remove_row_fn.apply(model, row as f64) {
            Ok(_) => Ok(()),
            Err(_) => Err(unsupported_error(self, &model)),
        }
    }

    fn insert_row(&self, row: usize, data: Self::Data) -> CoreResult<(), ModelError> {
        let row_count = self.row_count();
        if row > row_count {
            return Err(ModelError::out_of_bounds(row_count));
        }

        let Some(model_unknown) = self.js_impl.get_unknown() else {
            self.report_error(format_args!(
                "Node.js: JavaScript Model<T>'s insertRow threw an exception"
            ));
            return Err(ModelError::unsupported(self));
        };
        let Ok(model) = model_unknown.coerce_to_object() else {
            self.report_error(format_args!("Node.js: JavaScript Model<T> is not an object"));
            return Err(ModelError::unsupported(self));
        };
        let insert_row_fn: Function<FnArgs<(f64, Unknown<'_>)>, Unknown> =
            match model.get_named_property("insertRow") {
                Ok(f) => f,
                Err(e) => {
                    self.report_error(format_args!(
                    "Node.js: JavaScript Model<T> implementation is missing insertRow property: {e}"
                ));
                    return Err(ModelError::unsupported(self));
                }
            };
        let js_data = unsafe { Unknown::from_raw_unchecked(data.env, data.value) };
        // A rejected modification is reported by throwing.
        match insert_row_fn.apply(model, FnArgs::from((row as f64, js_data))) {
            Ok(_) => Ok(()),
            Err(_) => Err(unsupported_error(self, &model)),
        }
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        &**self.shared_model_notify
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}

/// Forwards a native adapter's notifications to the `modelNotify` of its TS wrapper.
struct NotifyForwarder(SharedModelNotify);

impl ModelChangeListener for NotifyForwarder {
    fn row_changed(self: Pin<&Self>, row: usize) {
        self.0.row_changed(row);
    }

    fn row_added(self: Pin<&Self>, index: usize, count: usize) {
        self.0.row_added(index, count);
    }

    fn row_removed(self: Pin<&Self>, index: usize, count: usize) {
        self.0.row_removed(index, count);
    }

    fn reset(self: Pin<&Self>) {
        self.0.reset();
    }
}

/// A native adapter whose tracker is the `modelNotify` of its TS wrapper, as for
/// a plain JS `Model`. Views and chained adapters therefore also see a subclass's
/// own `notify*` calls, and a `MapModel` gets a channel apart from its source's,
/// which core `MapModel` would otherwise share.
struct NotifyingAdapter {
    adapter: ModelRc<JsRawValue>,
    notify: SharedModelNotify,
    _forwarder: ModelChangeListenerBox<NotifyForwarder>,
}

impl NotifyingAdapter {
    fn new(adapter: ModelRc<JsRawValue>, notify: SharedModelNotify) -> Self {
        let forwarder = ModelChangeListenerBox::new(NotifyForwarder(notify.clone()));
        adapter.model_tracker().attach_peer(forwarder.as_ref().model_peer());
        Self { adapter, notify, _forwarder: forwarder }
    }
}

impl Model for NotifyingAdapter {
    type Data = JsRawValue;

    fn row_count(&self) -> usize {
        self.adapter.row_count()
    }

    fn row_data(&self, row: usize) -> Option<Self::Data> {
        self.adapter.row_data(row)
    }

    fn set_row_data(&self, row: usize, data: Self::Data) {
        self.adapter.set_row_data(row, data);
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        &**self.notify
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}

/// The sole point where `JsRawValue` gets converted to/from
/// `slint_interpreter::Value`: reached once, when a (possibly natively
/// chained) JS model is assigned to a `.slint` property whose declared item
/// type is known here.
///
/// `js_impl` is a weak handle to the original JS object passed to
/// `js_into_rust_model` — the plain `Model<T>` or the outer
/// Reverse/Filter/Sort/MapModel instance — kept only so `rust_into_js_model`
/// can hand it back unchanged on a property read-back, regardless of whether
/// `inner` is a `RawJsModel` or a native adapter chain.
struct TerminalTypedModel {
    inner: ModelRc<JsRawValue>,
    js_impl: WeakValueRef,
    env: Env,
    row_data_type: Type,
    prop_key: String,
    owner: JsAnchorOwner,
}

impl Drop for TerminalTypedModel {
    fn drop(&mut self) {
        remove_hidden_model_prop(&self.owner, self.env, &self.prop_key);
    }
}

impl Model for TerminalTypedModel {
    type Data = slint_interpreter::Value;

    fn row_count(&self) -> usize {
        self.inner.row_count()
    }

    fn row_data(&self, row: usize) -> Option<Self::Data> {
        let raw = self.inner.row_data(row)?;
        let unknown = unsafe { Unknown::from_raw_unchecked(raw.env, raw.value) };
        to_value(&self.env, unknown, &self.row_data_type, &self.owner).ok()
    }

    fn set_row_data(&self, row: usize, data: Self::Data) {
        let Ok(js_unknown) = to_js_unknown(&self.env, &data) else {
            report_glue_error(
                self.env,
                format_args!(
                    "Node.js: Model<T>'s set_row_data called by Rust with data type that can't be represented in JavaScript"
                ),
            );
            return;
        };
        self.inner.set_row_data(row, JsRawValue { env: self.env.raw(), value: js_unknown.raw() });
    }

    fn push_row(&self, data: Self::Data) -> CoreResult<(), ModelError> {
        let js_unknown =
            to_js_unknown(&self.env, &data).map_err(|_| ModelError::unsupported(self))?;
        self.inner.push_row(JsRawValue { env: self.env.raw(), value: js_unknown.raw() })
    }

    fn remove_row(&self, row: usize) -> CoreResult<(), ModelError> {
        self.inner.remove_row(row)
    }

    fn insert_row(&self, row: usize, data: Self::Data) -> CoreResult<(), ModelError> {
        let js_unknown =
            to_js_unknown(&self.env, &data).map_err(|_| ModelError::unsupported(self))?;
        self.inner.insert_row(row, JsRawValue { env: self.env.raw(), value: js_unknown.raw() })
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        self.inner.model_tracker()
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}

/// Type-erased handle to a native Reverse/Filter/Sort/MapModel adapter,
/// exposed to TypeScript so the `ReverseModel`/`FilterModel`/`SortModel`/
/// `MapModel` classes can delegate `rowCount`/`rowData`/`setRowData`/`reset`
/// to it directly instead of reimplementing the row-mapping bookkeeping in JS.
#[napi]
pub struct NativeModel {
    inner: ModelRc<JsRawValue>,
    // FilterModel/SortModel support re-applying their function; ReverseModel doesn't.
    reset_fn: Option<Box<dyn Fn()>>,
    // FilterModel/SortModel can map a row back to the source model's row index
    // (`unfilteredRow`/`unsortedRow`); ReverseModel doesn't expose this.
    unmap_fn: Option<Box<dyn Fn(usize) -> Option<usize>>>,
}

#[napi]
impl NativeModel {
    #[napi]
    pub fn row_count(&self) -> u32 {
        self.inner.row_count() as u32
    }

    #[napi]
    pub fn row_data<'a>(&self, env: &'a Env, row: u32) -> Result<Unknown<'a>> {
        match self.inner.row_data(row as usize) {
            Some(raw) => Ok(unsafe { Unknown::from_raw_unchecked(raw.env, raw.value) }),
            None => ().into_unknown(env),
        }
    }

    #[napi]
    pub fn set_row_data(&self, env: &Env, row: u32, data: Unknown<'_>) {
        // Some adapters (e.g. FilterModel/SortModel's row-mapping lookup)
        // index directly and panic on an out-of-range row, unlike the JS
        // `array[row]` this replaces, which just yields `undefined`.
        if row as usize >= self.inner.row_count() {
            return;
        }
        self.inner.set_row_data(row as usize, JsRawValue { env: env.raw(), value: data.raw() });
    }

    #[napi]
    pub fn reset(&self) {
        if let Some(reset_fn) = &self.reset_fn {
            reset_fn();
        }
    }

    #[napi]
    pub fn unmapped_row(&self, row: u32) -> Option<u32> {
        self.unmap_fn.as_ref().and_then(|f| f(row as usize)).map(|r| r as u32)
    }
}

impl NativeModel {
    fn new(
        adapter: ModelRc<JsRawValue>,
        notify: &SharedModelNotify,
        reset_fn: Option<Box<dyn Fn()>>,
        unmap_fn: Option<Box<dyn Fn(usize) -> Option<usize>>>,
    ) -> Self {
        let inner = Rc::new(NotifyingAdapter::new(adapter, notify.clone())).into();
        Self { inner, reset_fn, unmap_fn }
    }
}

#[napi]
pub fn native_reverse_model_new(
    env: Env,
    source: Object,
    notify: ExternalRef<SharedModelNotify>,
) -> Result<NativeModel> {
    let wrapped = resolve_source(&env, &source)?;
    let native = Rc::new(ReverseModel::new(wrapped));
    Ok(NativeModel::new(native.into(), &notify, None, None))
}

#[napi]
pub fn native_filter_model_new(
    env: Env,
    source: Object,
    predicate: Function<Unknown, Unknown>,
    notify: ExternalRef<SharedModelNotify>,
) -> Result<NativeModel> {
    let wrapped = resolve_source(&env, &source)?;
    let predicate_weak = WeakValueRef::new(&env, &predicate)?;
    let native = Rc::new(FilterModel::new(wrapped, move |data: &JsRawValue| -> bool {
        let Some(func_unknown) = predicate_weak.get_unknown() else {
            report_glue_error(
                env,
                format_args!("Node.js: FilterModel predicate function has been garbage collected"),
            );
            return false;
        };
        let Ok(predicate_fn) = (unsafe { func_unknown.cast::<Function<Unknown, Unknown>>() })
        else {
            report_glue_error(env, format_args!("Node.js: FilterModel predicate is not callable"));
            return false;
        };
        let js_data = unsafe { Unknown::from_raw_unchecked(data.env, data.value) };
        let Ok(result) = predicate_fn.call(js_data) else {
            report_glue_error(
                env,
                format_args!("Node.js: FilterModel predicate threw an exception"),
            );
            return false;
        };
        result.coerce_to_bool().unwrap_or(false)
    }));

    let reset_native = native.clone();
    let reset_fn: Box<dyn Fn()> = Box::new(move || reset_native.reset());

    let unmap_native = native.clone();
    let unmap_fn: Box<dyn Fn(usize) -> Option<usize>> = Box::new(move |row| {
        (row < unmap_native.row_count()).then(|| unmap_native.unfiltered_row(row))
    });

    Ok(NativeModel::new(native.into(), &notify, Some(reset_fn), Some(unmap_fn)))
}

#[napi]
pub fn native_sort_model_new(
    env: Env,
    source: Object,
    compare: Function<FnArgs<(Unknown, Unknown)>, Unknown>,
    notify: ExternalRef<SharedModelNotify>,
) -> Result<NativeModel> {
    let wrapped = resolve_source(&env, &source)?;
    let compare_weak = WeakValueRef::new(&env, &compare)?;
    let native = Rc::new(SortModel::new(
        wrapped,
        move |a: &JsRawValue, b: &JsRawValue| -> core::cmp::Ordering {
            use core::cmp::Ordering;
            let Some(func_unknown) = compare_weak.get_unknown() else {
                report_glue_error(
                    env,
                    format_args!("Node.js: SortModel compare function has been garbage collected"),
                );
                return Ordering::Equal;
            };
            let Ok(compare_fn) =
                (unsafe { func_unknown.cast::<Function<FnArgs<(Unknown, Unknown)>, Unknown>>() })
            else {
                report_glue_error(
                    env,
                    format_args!("Node.js: SortModel compare function is not callable"),
                );
                return Ordering::Equal;
            };
            let js_a = unsafe { Unknown::from_raw_unchecked(a.env, a.value) };
            let js_b = unsafe { Unknown::from_raw_unchecked(b.env, b.value) };
            let Ok(result) = compare_fn.call(FnArgs::from((js_a, js_b))) else {
                report_glue_error(
                    env,
                    format_args!("Node.js: SortModel compare function threw an exception"),
                );
                return Ordering::Equal;
            };
            let Ok(n) = result.coerce_to_number().and_then(|num| num.get_double()) else {
                report_glue_error(
                    env,
                    format_args!("Node.js: SortModel compare function returned a non-number"),
                );
                return Ordering::Equal;
            };
            n.partial_cmp(&0.0).unwrap_or(Ordering::Equal)
        },
    ));

    let reset_native = native.clone();
    let reset_fn: Box<dyn Fn()> = Box::new(move || reset_native.reset());

    let unmap_native = native.clone();
    let unmap_fn: Box<dyn Fn(usize) -> Option<usize>> = Box::new(move |row| {
        (row < unmap_native.row_count()).then(|| unmap_native.unsorted_row(row))
    });

    Ok(NativeModel::new(native.into(), &notify, Some(reset_fn), Some(unmap_fn)))
}

#[napi]
pub fn native_map_model_new(
    env: Env,
    source: Object,
    map_function: Function<Unknown, Unknown>,
    notify: ExternalRef<SharedModelNotify>,
) -> Result<NativeModel> {
    let wrapped = resolve_source(&env, &source)?;
    let map_fn_weak = WeakValueRef::new(&env, &map_function)?;
    let native = Rc::new(MapModel::new(wrapped, move |data: JsRawValue| -> JsRawValue {
        let Some(func_unknown) = map_fn_weak.get_unknown() else {
            report_glue_error(
                env,
                format_args!("Node.js: MapModel map function has been garbage collected"),
            );
            return data;
        };
        let Ok(map_fn) = (unsafe { func_unknown.cast::<Function<Unknown, Unknown>>() }) else {
            report_glue_error(env, format_args!("Node.js: MapModel map function is not callable"));
            return data;
        };
        let js_data = unsafe { Unknown::from_raw_unchecked(data.env, data.value) };
        let Ok(result) = map_fn.call(js_data) else {
            report_glue_error(
                env,
                format_args!("Node.js: MapModel map function threw an exception"),
            );
            return data;
        };
        JsRawValue { env: data.env, value: result.raw() }
    }));
    Ok(NativeModel::new(native.into(), &notify, None, None))
}

pub(crate) fn js_into_rust_model(
    env: &Env,
    maybe_js_impl: &Object,
    row_data_type: &Type,
    anchor_owner: &JsAnchorOwner,
) -> Result<ModelRc<slint_interpreter::Value>> {
    // `resolve_source` recognizes a TS Reverse/Filter/Sort/MapModel backed by a
    // native adapter and reuses its native backing directly, instead of
    // wrapping it as an opaque RawJsModel: same behavior, without per-row JS
    // callbacks for row mapping.
    let inner = resolve_source(env, maybe_js_impl)?;
    let prop_key = register_hidden_model_prop(env, anchor_owner, maybe_js_impl)?;

    Ok(Rc::new(TerminalTypedModel {
        inner,
        js_impl: WeakValueRef::new(env, maybe_js_impl)?,
        env: *env,
        row_data_type: row_data_type.clone(),
        prop_key,
        owner: anchor_owner.clone(),
    })
    .into())
}

/// Registers `maybe_js_impl` as a hidden JS property on the owning component
/// instance, keyed by a fresh per-anchor property name, so V8 keeps it alive
/// without creating an independent GC root. Returns the property key, to be
/// deleted again via `remove_hidden_model_prop` once the Rust-side model
/// wrapping it is dropped.
fn register_hidden_model_prop(
    env: &Env,
    anchor_owner: &JsAnchorOwner,
    maybe_js_impl: &Object,
) -> Result<String> {
    let anchor_id = anchor_owner.next_anchor_id();
    let prop_key = format!("__slint_model#{anchor_id}");
    if let Some(mut obj) = crate::weak_ref::weak_ref_get_object::<crate::JsComponentInstance>(
        &anchor_owner.owner_weak,
        *env,
    ) {
        crate::set_hidden_property(&mut obj, &prop_key, maybe_js_impl)?;
    }
    Ok(prop_key)
}

pub(crate) fn rust_into_js_model<'a>(
    env: &'a Env,
    model: &ModelRc<slint_interpreter::Value>,
) -> Option<Result<Unknown<'a>>> {
    model.as_any().downcast_ref::<TerminalTypedModel>().map(|rust_model| {
        rust_model
            .js_impl
            .get_unknown()
            .ok_or_else(|| napi::Error::from_reason("JS model has been garbage collected"))?
            .into_unknown(env)
    })
}

/// Deletes the hidden anchor property registered by
/// `register_hidden_model_prop`, guarding against use-after-free of the
/// owning component instance.
fn remove_hidden_model_prop(owner: &JsAnchorOwner, env: Env, prop_key: &str) {
    // Pure Rust check (no NAPI calls).
    // Returns None once the owning JsComponentInstance's anchor_seq
    // Rc has been dropped,
    // which happens before the model that's dropping (field declaration order).
    if owner.seq.upgrade().is_none() {
        return;
    }
    if let Some(mut obj) =
        crate::weak_ref::weak_ref_get_object::<crate::JsComponentInstance>(&owner.owner_weak, env)
    {
        let _ = obj.delete_named_property(prop_key);
    }
}

#[napi]
pub fn js_model_notify_new() -> Result<External<SharedModelNotify>> {
    Ok(External::new(Default::default()))
}

#[napi]
pub fn js_model_notify_row_data_changed(notify: ExternalRef<SharedModelNotify>, row: u32) {
    notify.row_changed(row as usize);
}

#[napi]
pub fn js_model_notify_row_added(notify: ExternalRef<SharedModelNotify>, row: u32, count: u32) {
    notify.row_added(row as usize, count as usize);
}

#[napi]
pub fn js_model_notify_row_removed(notify: ExternalRef<SharedModelNotify>, row: u32, count: u32) {
    notify.row_removed(row as usize, count as usize);
}

#[napi]
pub fn js_model_notify_reset(notify: ExternalRef<SharedModelNotify>) {
    notify.reset();
}

#[napi]
pub struct ReadOnlyRustModel(ModelRc<slint_interpreter::Value>);

impl From<ModelRc<slint_interpreter::Value>> for ReadOnlyRustModel {
    fn from(model: ModelRc<slint_interpreter::Value>) -> Self {
        Self(model)
    }
}

// Implement minimal Model<T> interface
#[napi]
impl ReadOnlyRustModel {
    #[napi]
    pub fn row_count(&self) -> u32 {
        self.0.row_count() as u32
    }

    #[napi]
    pub fn row_data<'a>(&self, env: &'a Env, row: u32) -> Result<Unknown<'a>> {
        let Some(data) = self.0.row_data(row as usize) else {
            return ().into_unknown(env);
        };
        crate::to_js_unknown(env, &data)
    }

    #[napi]
    pub fn set_row_data(&self, env: &Env, _row: u32, _data: Unknown<'_>) {
        crate::console_err!(
            *env,
            "setRowData called on a model which does not re-implement this method. This happens when trying to modify a read-only model"
        )
    }

    pub fn into_js<'a>(self, env: &'a Env) -> Result<Unknown<'a>> {
        let model = self.0.clone();

        let mut obj = self.into_instance(env)?.as_object(env);

        // Implement Iterator protocol by hand until it's stable in napi-rs
        let global = env.get_global()?;
        let symbol_function: Unknown = global.get_named_property("Symbol")?;
        let symbol_obj = symbol_function.coerce_to_object()?;
        let iterator_symbol: napi::JsSymbol = symbol_obj.get_named_property("iterator")?;

        obj.set_property(
            iterator_symbol,
            env.create_function_from_closure::<(), ModelIterator, _>(
                "rust model iterator",
                move |ctx| Ok(ModelIterator { model: model.clone(), row: 0, env: *ctx.env }),
            )?,
        )?;

        obj.into_unknown(env)
    }
}

#[napi]
pub struct ModelIterator {
    model: ModelRc<slint_interpreter::Value>,
    row: usize,
    env: Env,
}

#[napi]
impl ModelIterator {
    // Implements the JS iterator protocol — name must be `next`.
    #[allow(clippy::should_implement_trait)]
    #[napi]
    pub fn next(&mut self) -> Result<Unknown<'_>> {
        let mut result = Object::new(&self.env)?;
        if self.row >= self.model.row_count() {
            result.set_named_property("done", true)?;
        } else {
            let row = self.row;
            self.row += 1;
            result.set_named_property(
                "value",
                self.model.row_data(row).and_then(|value| to_js_unknown(&self.env, &value).ok()),
            )?
        }
        result.into_unknown(&self.env)
    }
}
