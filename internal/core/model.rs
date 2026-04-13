// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore vecmodel

//! Model and Repeater

use crate::items::StandardListViewItem;
use crate::{Property, SharedString, SharedVector};
pub use adapters::{FilterModel, MapModel, ReverseModel, SortModel};
use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::vec::Vec;
use core::cell::{Cell, RefCell};
use core::pin::Pin;
#[allow(unused)]
use euclid::num::{Ceil, Floor};
pub use model_peer::*;
use once_cell::unsync::OnceCell;
use pin_project::pin_project;

mod adapters;
mod model_peer;
mod repeater;

pub use repeater::{Conditional, ListViewProperties, RepeatedItemTree, Repeater, RepeaterTracker};

/// This trait defines the interface that users of a model can use to track changes
/// to a model. It is supplied via [`Model::model_tracker`] and implementation usually
/// return a reference to its field of [`ModelNotify`].
pub trait ModelTracker {
    /// Attach one peer. The peer will be notified when the model changes
    fn attach_peer(&self, peer: ModelPeer);
    /// Register the model as a dependency to the current binding being evaluated, so
    /// that it will be notified when the model changes its size.
    fn track_row_count_changes(&self);
    /// Register a row as a dependency to the current binding being evaluated, so that
    /// it will be notified when the value of that row changes.
    fn track_row_data_changes(&self, row: usize);
}

impl ModelTracker for () {
    fn attach_peer(&self, _peer: ModelPeer) {}

    fn track_row_count_changes(&self) {}
    fn track_row_data_changes(&self, _row: usize) {}
}

/// A Model is providing Data for the repeated elements with `for` in the `.slint` language
///
/// If the model can be changed, the type implementing the Model trait should hold
/// a [`ModelNotify`], and is responsible to call functions on it to let the UI know that
/// something has changed.
///
/// Properties of type array will be mapped to a [`ModelRc<T>`], which wraps a `Rc<Model<Data = T>>.`
/// The [`ModelRc`] documentation has examples on how to set models to array properties.
///
/// It is more efficient to operate on the model and send changes through the `ModelNotify` rather than
/// resetting the property with a different model.
///
/// ## Example
///
/// As an example, let's see the implementation of [`VecModel`].
///
/// ```
/// # use i_slint_core::model::{Model, ModelNotify, ModelPeer, ModelTracker};
/// pub struct VecModel<T> {
///     // the backing data, stored in a `RefCell` as this model can be modified
///     array: std::cell::RefCell<Vec<T>>,
///     // the ModelNotify will allow to notify the UI that the model changes
///     notify: ModelNotify,
/// }
///
/// impl<T: Clone + 'static> Model for VecModel<T> {
///     type Data = T;
///
///     fn row_count(&self) -> usize {
///         self.array.borrow().len()
///     }
///
///     fn row_data(&self, row: usize) -> Option<Self::Data> {
///         self.array.borrow().get(row).cloned()
///     }
///
///     fn set_row_data(&self, row: usize, data: Self::Data) {
///         self.array.borrow_mut()[row] = data;
///         // don't forget to call row_changed
///         self.notify.row_changed(row);
///     }
///
///     fn model_tracker(&self) -> &dyn ModelTracker {
///         &self.notify
///     }
///
///     fn as_any(&self) -> &dyn core::any::Any {
///         // a typical implementation just return `self`
///         self
///     }
/// }
///
/// // when modifying the model, we call the corresponding function in
/// // the ModelNotify
/// impl<T> VecModel<T> {
///     /// Add a row at the end of the model
///     pub fn push(&self, value: T) {
///         self.array.borrow_mut().push(value);
///         self.notify.row_added(self.array.borrow().len() - 1, 1)
///     }
///
///     /// Remove the row at the given index from the model
///     pub fn remove(&self, index: usize) {
///         self.array.borrow_mut().remove(index);
///         self.notify.row_removed(index, 1)
///     }
/// }
/// ```
pub trait Model {
    /// The model data: A model is a set of rows and each row has this data
    type Data;
    /// The number of rows in the model
    fn row_count(&self) -> usize;
    /// Returns the data for a particular row.
    ///
    /// This function should normally be called with `row < row_count()` and should return None otherwise.
    ///
    /// This function does not register dependencies on the current binding. For an equivalent
    /// function that tracks dependencies, see [`ModelExt::row_data_tracked`]
    fn row_data(&self, row: usize) -> Option<Self::Data>;
    /// Sets the data for a particular row.
    ///
    /// This function should be called with `row < row_count()`, otherwise the implementation can panic.
    ///
    /// If the model cannot support data changes, then it is ok to do nothing.
    /// The default implementation will print a warning to stderr.
    ///
    /// If the model can update the data, it should also call [`ModelNotify::row_changed`] on its
    /// internal [`ModelNotify`].
    fn set_row_data(&self, _row: usize, _data: Self::Data) {
        #[cfg(feature = "std")]
        crate::debug_log!(
            "Model::set_row_data called on a model of type {} which does not re-implement this method. \
            This happens when trying to modify a read-only model",
            core::any::type_name::<Self>(),
        );
    }

    /// The implementation should return a reference to its [`ModelNotify`] field.
    ///
    /// You can return `&()` if you your `Model` is constant and does not have a ModelNotify field.
    fn model_tracker(&self) -> &dyn ModelTracker;

    /// Returns an iterator visiting all elements of the model.
    fn iter(&self) -> ModelIterator<'_, Self::Data>
    where
        Self: Sized,
    {
        ModelIterator::new(self)
    }

    /// Return something that can be downcast'ed (typically self).
    ///
    /// Use this to retrieve the concrete model from a [`ModelRc`] stored
    /// in your tree of UI elements.
    ///
    /// ```
    /// # use i_slint_core::model::*;
    /// # use std::rc::Rc;
    /// let handle = ModelRc::new(VecModel::from(vec![1i32, 2, 3]));
    /// // later:
    /// handle.as_any().downcast_ref::<VecModel<i32>>().unwrap().push(4);
    /// assert_eq!(handle.row_data(3).unwrap(), 4);
    /// ```
    ///
    /// Note: Custom models must implement this method for the cast to succeed.
    /// A valid implementation is to return `self`:
    /// ```ignore
    ///     fn as_any(&self) -> &dyn core::any::Any { self }
    /// ```
    ///
    /// ## Troubleshooting
    /// A common reason why the dowcast fails at run-time is because of a type-mismatch
    /// between the model created and the model downcasted. To debug this at compile time,
    /// try matching the model type used for the downcast explicitly at model creation time.
    /// In the following example, the downcast fails at run-time:
    ///
    /// ```
    /// # use i_slint_core::model::*;
    /// # use std::rc::Rc;
    /// let model = VecModel::from_slice(&[3i32, 2, 1])
    ///     .filter(Box::new(|v: &i32| *v >= 2) as Box<dyn Fn(&i32) -> bool>);
    /// let model_rc = ModelRc::new(model);
    /// assert!(model_rc.as_any()
    ///     .downcast_ref::<FilterModel<VecModel<i32>, Box<dyn Fn(&i32) -> bool>>>()
    ///     .is_none());
    /// ```
    ///
    /// To debug this, let's make the type explicit. It fails to compile.
    ///
    /// ```compile_fail
    /// # use i_slint_core::model::*;
    /// # use std::rc::Rc;
    /// let model: FilterModel<VecModel<i32>, Box<dyn Fn(&i32) -> bool>>
    ///     = VecModel::from_slice(&[3i32, 2, 1])
    ///       .filter(Box::new(|v: &i32| *v >= 2) as Box<dyn Fn(&i32) -> bool>);
    /// let model_rc = ModelRc::new(model);
    /// assert!(model_rc.as_any()
    ///     .downcast_ref::<FilterModel<VecModel<i32>, Box<dyn Fn(&i32) -> bool>>>()
    ///     .is_none());
    /// ```
    ///
    /// The compiler tells us that the type of model is not `FilterModel<VecModel<..>>`,
    /// but instead `from_slice()` already returns a `ModelRc`, so the correct type to
    /// use for the downcast is wrapped in `ModelRc`:
    ///
    /// ```
    /// # use i_slint_core::model::*;
    /// # use std::rc::Rc;
    /// let model: FilterModel<ModelRc<i32>, Box<dyn Fn(&i32) -> bool>>
    ///     = VecModel::from_slice(&[3i32, 2, 1])
    ///       .filter(Box::new(|v: &i32| *v >= 2) as Box<dyn Fn(&i32) -> bool>);
    /// let model_rc = ModelRc::new(model);
    /// assert!(model_rc.as_any()
    ///     .downcast_ref::<FilterModel<ModelRc<i32>, Box<dyn Fn(&i32) -> bool>>>()
    ///     .is_some());
    /// ```
    fn as_any(&self) -> &dyn core::any::Any {
        &()
    }
}

/// Extension trait with extra methods implemented on types that implement [`Model`]
pub trait ModelExt: Model {
    /// Convenience function that calls [`ModelTracker::track_row_data_changes`]
    /// before returning [`Model::row_data`].
    ///
    /// Calling [`row_data(row)`](Model::row_data) does not register the row as a dependency when calling it while
    /// evaluating a property binding. This function calls [`track_row_data_changes(row)`](ModelTracker::track_row_data_changes)
    /// on the [`self.model_tracker()`](Model::model_tracker) to enable tracking.
    fn row_data_tracked(&self, row: usize) -> Option<Self::Data> {
        self.model_tracker().track_row_data_changes(row);
        self.row_data(row)
    }

    /// Returns a new Model where all elements are mapped by the function `map_function`.
    /// This is a shortcut for [`MapModel::new()`].
    fn map<F, U>(self, map_function: F) -> MapModel<Self, F>
    where
        Self: Sized + 'static,
        F: Fn(Self::Data) -> U + 'static,
    {
        MapModel::new(self, map_function)
    }

    /// Returns a new Model where the elements are filtered by the function `filter_function`.
    /// This is a shortcut for [`FilterModel::new()`].
    fn filter<F>(self, filter_function: F) -> FilterModel<Self, F>
    where
        Self: Sized + 'static,
        F: Fn(&Self::Data) -> bool + 'static,
    {
        FilterModel::new(self, filter_function)
    }

    /// Returns a new Model where the elements are sorted ascending.
    /// This is a shortcut for [`SortModel::new_ascending()`].
    #[must_use]
    fn sort(self) -> SortModel<Self, adapters::AscendingSortHelper>
    where
        Self: Sized + 'static,
        Self::Data: core::cmp::Ord,
    {
        SortModel::new_ascending(self)
    }

    /// Returns a new Model where the elements are sorted by the function `sort_function`.
    /// This is a shortcut for [`SortModel::new()`].
    fn sort_by<F>(self, sort_function: F) -> SortModel<Self, F>
    where
        Self: Sized + 'static,
        F: FnMut(&Self::Data, &Self::Data) -> core::cmp::Ordering + 'static,
    {
        SortModel::new(self, sort_function)
    }

    /// Returns a new Model where the elements are reversed.
    /// This is a shortcut for [`ReverseModel::new()`].
    fn reverse(self) -> ReverseModel<Self>
    where
        Self: Sized + 'static,
    {
        ReverseModel::new(self)
    }
}

impl<T: Model> ModelExt for T {}

/// An iterator over the elements of a model.
/// This struct is created by the [`Model::iter()`] trait function.
pub struct ModelIterator<'a, T> {
    model: &'a dyn Model<Data = T>,
    row: usize,
}

impl<'a, T> ModelIterator<'a, T> {
    /// Creates a new model iterator for a model reference.
    /// This is the same as calling [`model.iter()`](Model::iter)
    pub fn new(model: &'a dyn Model<Data = T>) -> Self {
        Self { model, row: 0 }
    }
}

impl<T> Iterator for ModelIterator<'_, T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.row >= self.model.row_count() {
            return None;
        }
        let row = self.row;
        self.row += 1;
        self.model.row_data(row)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.model.row_count();
        (len, Some(len))
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        self.row = self.row.checked_add(n)?;
        self.next()
    }
}

impl<T> ExactSizeIterator for ModelIterator<'_, T> {}

impl<M: Model> Model for Rc<M> {
    type Data = M::Data;

    fn row_count(&self) -> usize {
        (**self).row_count()
    }

    fn row_data(&self, row: usize) -> Option<Self::Data> {
        (**self).row_data(row)
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        (**self).model_tracker()
    }

    fn as_any(&self) -> &dyn core::any::Any {
        (**self).as_any()
    }
    fn set_row_data(&self, row: usize, data: Self::Data) {
        (**self).set_row_data(row, data)
    }
}

/// A [`Model`] backed by a `Vec<T>`, using interior mutability.
pub struct VecModel<T> {
    array: RefCell<Vec<T>>,
    notify: ModelNotify,
}

impl<T> Default for VecModel<T> {
    fn default() -> Self {
        Self { array: Default::default(), notify: Default::default() }
    }
}

impl<T: 'static> VecModel<T> {
    /// Allocate a new model from a slice
    pub fn from_slice(slice: &[T]) -> ModelRc<T>
    where
        T: Clone,
    {
        ModelRc::new(Self::from(slice.to_vec()))
    }

    /// Add a row at the end of the model
    pub fn push(&self, value: T) {
        self.array.borrow_mut().push(value);
        self.notify.row_added(self.array.borrow().len() - 1, 1)
    }

    /// Inserts a row at position index. All rows after that are shifted.
    /// This function panics if index is > row_count().
    pub fn insert(&self, index: usize, value: T) {
        self.array.borrow_mut().insert(index, value);
        self.notify.row_added(index, 1)
    }

    /// Remove the row at the given index from the model
    ///
    /// Returns the removed row
    pub fn remove(&self, index: usize) -> T {
        let r = self.array.borrow_mut().remove(index);
        self.notify.row_removed(index, 1);
        r
    }

    /// Replace inner Vec with new data
    pub fn set_vec(&self, new: impl Into<Vec<T>>) {
        *self.array.borrow_mut() = new.into();
        self.notify.reset();
    }

    /// Extend the model with the content of the iterator
    ///
    /// Similar to [`Vec::extend`]
    pub fn extend<I: IntoIterator<Item = T>>(&self, iter: I) {
        let mut array = self.array.borrow_mut();
        let old_idx = array.len();
        array.extend(iter);
        let count = array.len() - old_idx;
        drop(array);
        self.notify.row_added(old_idx, count);
    }

    /// Clears the model, removing all values
    ///
    /// Similar to [`Vec::clear`]
    pub fn clear(&self) {
        self.array.borrow_mut().clear();
        self.notify.reset();
    }

    /// Swaps two elements in the model.
    pub fn swap(&self, a: usize, b: usize) {
        if a == b {
            return;
        }

        self.array.borrow_mut().swap(a, b);
        self.notify.row_changed(a);
        self.notify.row_changed(b);
    }
}

impl<T: Clone + 'static> VecModel<T> {
    /// Appends all the elements in the slice to the model
    ///
    /// Similar to [`Vec::extend_from_slice`]
    pub fn extend_from_slice(&self, src: &[T]) {
        let mut array = self.array.borrow_mut();
        let old_idx = array.len();

        array.extend_from_slice(src);
        drop(array);
        self.notify.row_added(old_idx, src.len());
    }
}

impl<T> From<Vec<T>> for VecModel<T> {
    fn from(array: Vec<T>) -> Self {
        VecModel { array: RefCell::new(array), notify: Default::default() }
    }
}

impl<T> FromIterator<T> for VecModel<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        VecModel::from(Vec::from_iter(iter))
    }
}

impl<T: Clone + 'static> Model for VecModel<T> {
    type Data = T;

    fn row_count(&self) -> usize {
        self.array.borrow().len()
    }

    fn row_data(&self, row: usize) -> Option<Self::Data> {
        self.array.borrow().get(row).cloned()
    }

    fn set_row_data(&self, row: usize, data: Self::Data) {
        if row < self.row_count() {
            self.array.borrow_mut()[row] = data;
            self.notify.row_changed(row);
        }
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        &self.notify
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}

/// A model backed by a `SharedVector<T>`
#[derive(Default)]
pub struct SharedVectorModel<T> {
    array: RefCell<SharedVector<T>>,
    notify: ModelNotify,
}

impl<T: Clone + 'static> SharedVectorModel<T> {
    /// Add a row at the end of the model
    pub fn push(&self, value: T) {
        self.array.borrow_mut().push(value);
        self.notify.row_added(self.array.borrow().len() - 1, 1)
    }
}

impl<T> SharedVectorModel<T> {
    /// Returns a clone of the model's backing shared vector.
    pub fn shared_vector(&self) -> SharedVector<T> {
        self.array.borrow_mut().clone()
    }
}

impl<T> From<SharedVector<T>> for SharedVectorModel<T> {
    fn from(array: SharedVector<T>) -> Self {
        SharedVectorModel { array: RefCell::new(array), notify: Default::default() }
    }
}

impl<T: Clone + 'static> Model for SharedVectorModel<T> {
    type Data = T;

    fn row_count(&self) -> usize {
        self.array.borrow().len()
    }

    fn row_data(&self, row: usize) -> Option<Self::Data> {
        self.array.borrow().get(row).cloned()
    }

    fn set_row_data(&self, row: usize, data: Self::Data) {
        self.array.borrow_mut().make_mut_slice()[row] = data;
        self.notify.row_changed(row);
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        &self.notify
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}

impl Model for usize {
    type Data = i32;

    fn row_count(&self) -> usize {
        *self
    }

    fn row_data(&self, row: usize) -> Option<Self::Data> {
        (row < self.row_count()).then_some(row as i32)
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        &()
    }
}

impl Model for bool {
    type Data = ();

    fn row_count(&self) -> usize {
        if *self { 1 } else { 0 }
    }

    fn row_data(&self, row: usize) -> Option<Self::Data> {
        (row < self.row_count()).then_some(())
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        &()
    }
}

/// ModelRc is a type wrapper for a reference counted implementation of the [`Model`] trait.
///
/// Models are used to represent sequences of the same data type. In `.slint` code those
/// are represented using the `[T]` array syntax and typically used in `for` expressions,
/// array properties, and array struct fields.
///
/// For example, a `property <[string]> foo` will be of type `ModelRc<SharedString>`
/// and, behind the scenes, wraps a `Rc<dyn Model<Data = SharedString>>.`
///
/// An array struct field will also be of type `ModelRc`:
///
/// ```slint,no-preview
/// export struct AddressBook {
///     names: [string]
/// }
/// ```
///
/// When accessing `AddressBook` from Rust, the `names` field will be of type `ModelRc<SharedString>`.
///
/// There are several ways of constructing a ModelRc in Rust:
///
/// * An empty ModelRc can be constructed with [`ModelRc::default()`].
/// * A `ModelRc` can be constructed from a slice or an array using the [`From`] trait.
///   This allocates a [`VecModel`].
/// * Use [`ModelRc::new()`] to construct a `ModelRc` from a type that implements the
///   [`Model`] trait, such as [`VecModel`] or your own implementation.
/// * If you have your model already in an `Rc`, then you can use the [`From`] trait
///   to convert from `Rc<dyn Model<Data = T>>` to `ModelRc`.
///
/// ## Example
///
/// ```rust
/// # i_slint_backend_testing::init_no_event_loop();
/// use slint::{slint, SharedString, ModelRc, Model, VecModel};
/// use std::rc::Rc;
/// slint!{
///     import { Button } from "std-widgets.slint";
///     export component Example {
///         callback add_item <=> btn.clicked;
///         in property <[string]> the_model;
///         HorizontalLayout {
///             for it in the_model : Text { text: it; }
///             btn := Button { text: "Add"; }
///         }
///     }
/// }
/// let ui = Example::new().unwrap();
/// // Create a VecModel and put it in an Rc.
/// let the_model : Rc<VecModel<SharedString>> =
///         Rc::new(VecModel::from(vec!["Hello".into(), "World".into()]));
/// // Convert it to a ModelRc.
/// let the_model_rc = ModelRc::from(the_model.clone());
/// // Pass the model to the ui: The generated set_the_model setter from the
/// // the_model property takes a ModelRc.
/// ui.set_the_model(the_model_rc);
///
/// // We have kept a strong reference to the_model, to modify it in a callback.
/// ui.on_add_item(move || {
///     // Use VecModel API: VecModel uses the Model notification mechanism to let Slint
///     // know it needs to refresh the UI.
///     the_model.push("SomeValue".into());
/// });
///
/// // Alternative: we can re-use a getter.
/// let ui_weak = ui.as_weak();
/// ui.on_add_item(move || {
///     let ui = ui_weak.unwrap();
///     let the_model_rc = ui.get_the_model();
///     let the_model = the_model_rc.as_any().downcast_ref::<VecModel<SharedString>>()
///         .expect("We know we set a VecModel earlier");
///     the_model.push("An Item".into());
/// });
/// ```
///
/// ### Updating the Model from a Thread
///
/// `ModelRc` is not `Send` and can only be used in the main thread.
/// If you want to update the model based on data coming from another thread, you need to send back the data to the main thread
/// using [`invoke_from_event_loop`](crate::api::invoke_from_event_loop) or
/// [`Weak::upgrade_in_event_loop`](crate::api::Weak::upgrade_in_event_loop).
///
/// ```rust
/// # i_slint_backend_testing::init_integration_test_with_mock_time();
/// use slint::Model;
/// slint::slint!{
///     export component TestCase inherits Window {
///         in property <[string]> the_model;
///         //...
///     }
/// }
/// let ui = TestCase::new().unwrap();
/// // set a model (a VecModel)
/// let model = std::rc::Rc::new(slint::VecModel::<slint::SharedString>::default());
/// ui.set_the_model(model.clone().into());
///
/// // do some work in a thread
/// let ui_weak = ui.as_weak();
/// let thread = std::thread::spawn(move || {
///     // do some work
///     let new_strings = vec!["foo".into(), "bar".into()];
///     // send the data back to the main thread
///     ui_weak.upgrade_in_event_loop(move |ui| {
///         let model = ui.get_the_model();
///         let model = model.as_any().downcast_ref::<slint::VecModel<slint::SharedString>>()
///             .expect("We know we set a VecModel earlier");
///         model.set_vec(new_strings);
/// #       slint::quit_event_loop().unwrap();
///     });
/// });
/// ui.run().unwrap();
/// ```
pub struct ModelRc<T>(Option<Rc<dyn Model<Data = T>>>);

impl<T> core::fmt::Debug for ModelRc<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "ModelRc(dyn Model)")
    }
}

impl<T> Clone for ModelRc<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> Default for ModelRc<T> {
    /// Construct an empty model
    fn default() -> Self {
        Self(None)
    }
}

impl<T> core::cmp::PartialEq for ModelRc<T> {
    fn eq(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (None, None) => true,
            (Some(a), Some(b)) => core::ptr::eq(
                (&**a) as *const dyn Model<Data = T> as *const u8,
                (&**b) as *const dyn Model<Data = T> as *const u8,
            ),
            _ => false,
        }
    }
}

#[cfg(feature = "serde")]
use serde::ser::SerializeSeq;
#[cfg(feature = "serde")]
impl<T> serde::Serialize for ModelRc<T>
where
    T: serde::Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(self.row_count()))?;
        for item in self.iter() {
            seq.serialize_element(&item)?;
        }
        seq.end()
    }
}

#[cfg(feature = "serde")]
impl<'de, T> serde::Deserialize<'de> for ModelRc<T>
where
    T: serde::Deserialize<'de> + Clone + 'static,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let vec = Vec::<T>::deserialize(deserializer)?;
        if vec.is_empty() {
            return Ok(ModelRc::default());
        }
        Ok(ModelRc::new(VecModel::from(vec)))
    }
}

impl<T> ModelRc<T> {
    pub fn new(model: impl Model<Data = T> + 'static) -> Self {
        Self(Some(Rc::new(model)))
    }
}

impl<T, M: Model<Data = T> + 'static> From<Rc<M>> for ModelRc<T> {
    fn from(model: Rc<M>) -> Self {
        Self(Some(model))
    }
}

impl<T> From<Rc<dyn Model<Data = T> + 'static>> for ModelRc<T> {
    fn from(model: Rc<dyn Model<Data = T> + 'static>) -> Self {
        Self(Some(model))
    }
}

impl<T: Clone + 'static> From<&[T]> for ModelRc<T> {
    fn from(slice: &[T]) -> Self {
        VecModel::from_slice(slice)
    }
}

impl<T: Clone + 'static, const N: usize> From<[T; N]> for ModelRc<T> {
    fn from(array: [T; N]) -> Self {
        VecModel::from_slice(&array)
    }
}

impl<T> TryInto<Rc<dyn Model<Data = T>>> for ModelRc<T> {
    type Error = ();

    fn try_into(self) -> Result<Rc<dyn Model<Data = T>>, Self::Error> {
        self.0.ok_or(())
    }
}

impl<T> Model for ModelRc<T> {
    type Data = T;

    fn row_count(&self) -> usize {
        self.0.as_ref().map_or(0, |model| model.row_count())
    }

    fn row_data(&self, row: usize) -> Option<Self::Data> {
        self.0.as_ref().and_then(|model| model.row_data(row))
    }

    fn set_row_data(&self, row: usize, data: Self::Data) {
        if let Some(model) = self.0.as_ref() {
            model.set_row_data(row, data);
        }
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        self.0.as_ref().map_or(&(), |model| model.model_tracker())
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self.0.as_ref().map_or(&(), |model| model.as_any())
    }
}

impl From<SharedString> for StandardListViewItem {
    fn from(value: SharedString) -> Self {
        StandardListViewItem { text: value }
    }
}

impl From<&str> for StandardListViewItem {
    fn from(value: &str) -> Self {
        StandardListViewItem { text: value.into() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec;

    #[cfg(feature = "serde")]
    #[test]
    fn test_serialize_deserialize_modelrc() {
        let model_rc = ModelRc::new(VecModel::from(vec![1, 2, 3]));
        let serialized = serde_json::to_string(&model_rc).unwrap();
        let deserialized: ModelRc<i32> = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.row_count(), 3);
        assert_eq!(deserialized.row_data(0), Some(1));
        assert_eq!(deserialized.row_data(1), Some(2));
        assert_eq!(deserialized.row_data(2), Some(3));
    }

    #[test]
    fn test_tracking_model_handle() {
        let model: Rc<VecModel<u8>> = Rc::new(Default::default());
        let handle = ModelRc::from(model.clone() as Rc<dyn Model<Data = u8>>);
        let tracker = Box::pin(<crate::properties::PropertyTracker>::default());
        assert_eq!(
            tracker.as_ref().evaluate(|| {
                handle.model_tracker().track_row_count_changes();
                handle.row_count()
            }),
            0
        );
        assert!(!tracker.is_dirty());
        model.push(42);
        model.push(100);
        assert!(tracker.is_dirty());
        assert_eq!(
            tracker.as_ref().evaluate(|| {
                handle.model_tracker().track_row_count_changes();
                handle.row_count()
            }),
            2
        );
        assert!(!tracker.is_dirty());
        model.set_row_data(0, 41);
        assert!(!tracker.is_dirty());
        model.remove(0);
        assert!(tracker.is_dirty());
        assert_eq!(
            tracker.as_ref().evaluate(|| {
                handle.model_tracker().track_row_count_changes();
                handle.row_count()
            }),
            1
        );
        assert!(!tracker.is_dirty());
        model.set_vec(vec![1, 2, 3]);
        assert!(tracker.is_dirty());
    }

    #[test]
    fn test_data_tracking() {
        let model: Rc<VecModel<u8>> = Rc::new(VecModel::from(vec![0, 1, 2, 3, 4]));
        let handle = ModelRc::from(model.clone());
        let tracker = Box::pin(<crate::properties::PropertyTracker>::default());
        assert_eq!(
            tracker.as_ref().evaluate(|| {
                handle.model_tracker().track_row_data_changes(1);
                handle.row_data(1).unwrap()
            }),
            1
        );
        assert!(!tracker.is_dirty());

        model.set_row_data(2, 42);
        assert!(!tracker.is_dirty());
        model.set_row_data(1, 100);
        assert!(tracker.is_dirty());

        assert_eq!(
            tracker.as_ref().evaluate(|| {
                handle.model_tracker().track_row_data_changes(1);
                handle.row_data(1).unwrap()
            }),
            100
        );
        assert!(!tracker.is_dirty());

        // Any changes to rows (even if after tracked rows) for now also marks watched rows as dirty, to
        // keep the logic simple.
        model.push(200);
        assert!(tracker.is_dirty());

        assert_eq!(tracker.as_ref().evaluate(|| { handle.row_data_tracked(1).unwrap() }), 100);
        assert!(!tracker.is_dirty());

        model.insert(0, 255);
        assert!(tracker.is_dirty());

        model.set_vec(Vec::new());
        assert!(tracker.is_dirty());
    }

    #[derive(Default)]
    struct TestView {
        // Track the parameters reported by the model (row counts, indices, etc.).
        // The last field in the tuple is the row size the model reports at the time
        // of callback
        changed_rows: RefCell<Vec<(usize, usize)>>,
        added_rows: RefCell<Vec<(usize, usize, usize)>>,
        removed_rows: RefCell<Vec<(usize, usize, usize)>>,
        reset: RefCell<usize>,
        model: RefCell<Option<std::rc::Weak<dyn Model<Data = i32>>>>,
    }
    impl TestView {
        fn clear(&self) {
            self.changed_rows.borrow_mut().clear();
            self.added_rows.borrow_mut().clear();
            self.removed_rows.borrow_mut().clear();
            *self.reset.borrow_mut() = 0;
        }
        fn row_count(&self) -> usize {
            self.model
                .borrow()
                .as_ref()
                .and_then(|model| model.upgrade())
                .map_or(0, |model| model.row_count())
        }
    }
    impl ModelChangeListener for TestView {
        fn row_changed(self: Pin<&Self>, row: usize) {
            self.changed_rows.borrow_mut().push((row, self.row_count()));
        }

        fn row_added(self: Pin<&Self>, index: usize, count: usize) {
            self.added_rows.borrow_mut().push((index, count, self.row_count()));
        }

        fn row_removed(self: Pin<&Self>, index: usize, count: usize) {
            self.removed_rows.borrow_mut().push((index, count, self.row_count()));
        }
        fn reset(self: Pin<&Self>) {
            *self.reset.borrow_mut() += 1;
        }
    }

    #[test]
    fn test_vecmodel_set_vec() {
        let view = Box::pin(ModelChangeListenerContainer::<TestView>::default());

        let model = Rc::new(VecModel::from(vec![1i32, 2, 3, 4]));
        model.model_tracker().attach_peer(Pin::as_ref(&view).model_peer());
        *view.model.borrow_mut() =
            Some(std::rc::Rc::downgrade(&(model.clone() as Rc<dyn Model<Data = i32>>)));

        model.push(5);
        assert!(view.changed_rows.borrow().is_empty());
        assert_eq!(&*view.added_rows.borrow(), &[(4, 1, 5)]);
        assert!(view.removed_rows.borrow().is_empty());
        assert_eq!(*view.reset.borrow(), 0);
        view.clear();

        model.set_vec(vec![6, 7, 8]);
        assert!(view.changed_rows.borrow().is_empty());
        assert!(view.added_rows.borrow().is_empty());
        assert!(view.removed_rows.borrow().is_empty());
        assert_eq!(*view.reset.borrow(), 1);
        view.clear();

        model.extend_from_slice(&[9, 10, 11]);
        assert!(view.changed_rows.borrow().is_empty());
        assert_eq!(&*view.added_rows.borrow(), &[(3, 3, 6)]);
        assert!(view.removed_rows.borrow().is_empty());
        assert_eq!(*view.reset.borrow(), 0);
        view.clear();

        model.extend([12, 13]);
        assert!(view.changed_rows.borrow().is_empty());
        assert_eq!(&*view.added_rows.borrow(), &[(6, 2, 8)]);
        assert!(view.removed_rows.borrow().is_empty());
        assert_eq!(*view.reset.borrow(), 0);
        view.clear();

        assert_eq!(model.iter().collect::<Vec<_>>(), vec![6, 7, 8, 9, 10, 11, 12, 13]);

        model.swap(1, 1);
        assert!(view.changed_rows.borrow().is_empty());
        assert!(view.added_rows.borrow().is_empty());
        assert!(view.removed_rows.borrow().is_empty());
        assert_eq!(*view.reset.borrow(), 0);
        view.clear();

        model.swap(1, 2);
        assert_eq!(&*view.changed_rows.borrow(), &[(1, 8), (2, 8)]);
        assert!(view.added_rows.borrow().is_empty());
        assert!(view.removed_rows.borrow().is_empty());
        assert_eq!(*view.reset.borrow(), 0);
        view.clear();

        assert_eq!(model.iter().collect::<Vec<_>>(), vec![6, 8, 7, 9, 10, 11, 12, 13]);
    }

    #[test]
    fn test_vecmodel_clear() {
        let view = Box::pin(ModelChangeListenerContainer::<TestView>::default());

        let model = Rc::new(VecModel::from(vec![1, 2, 3, 4]));
        model.model_tracker().attach_peer(Pin::as_ref(&view).model_peer());
        *view.model.borrow_mut() =
            Some(std::rc::Rc::downgrade(&(model.clone() as Rc<dyn Model<Data = i32>>)));

        model.clear();
        assert_eq!(*view.reset.borrow(), 1);
        assert_eq!(model.row_count(), 0);
    }

    #[test]
    fn test_vecmodel_swap() {
        let view = Box::pin(ModelChangeListenerContainer::<TestView>::default());

        let model = Rc::new(VecModel::from(vec![1, 2, 3, 4]));
        model.model_tracker().attach_peer(Pin::as_ref(&view).model_peer());
        *view.model.borrow_mut() =
            Some(std::rc::Rc::downgrade(&(model.clone() as Rc<dyn Model<Data = i32>>)));

        model.swap(1, 1);
        assert!(view.changed_rows.borrow().is_empty());
        assert!(view.added_rows.borrow().is_empty());
        assert!(view.removed_rows.borrow().is_empty());
        assert_eq!(*view.reset.borrow(), 0);
        view.clear();

        model.swap(1, 2);
        assert_eq!(&*view.changed_rows.borrow(), &[(1, 4), (2, 4)]);
        assert!(view.added_rows.borrow().is_empty());
        assert!(view.removed_rows.borrow().is_empty());
        assert_eq!(*view.reset.borrow(), 0);
        view.clear();
    }

    #[test]
    fn modeliter_in_bounds() {
        struct TestModel {
            length: usize,
            max_requested_row: Cell<usize>,
            notify: ModelNotify,
        }

        impl Model for TestModel {
            type Data = usize;

            fn row_count(&self) -> usize {
                self.length
            }

            fn row_data(&self, row: usize) -> Option<usize> {
                self.max_requested_row.set(self.max_requested_row.get().max(row));
                (row < self.length).then_some(row)
            }

            fn model_tracker(&self) -> &dyn ModelTracker {
                &self.notify
            }
        }

        let model = Rc::new(TestModel {
            length: 10,
            max_requested_row: Cell::new(0),
            notify: Default::default(),
        });

        assert_eq!(model.iter().max().unwrap(), 9);
        assert_eq!(model.max_requested_row.get(), 9);
    }

    #[test]
    fn vecmodel_doesnt_require_default() {
        #[derive(Clone)]
        struct MyNoDefaultType {
            _foo: bool,
        }
        let model = VecModel::<MyNoDefaultType>::default();
        assert_eq!(model.row_count(), 0);
        model.push(MyNoDefaultType { _foo: true });
    }
}
