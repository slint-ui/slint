// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore vecmodel

//! Model and Repeater

use crate::item_tree::ItemTreeVTable;
use crate::item_tree::TraversalOrder;
pub use crate::items::{StandardListViewItem, TableColumn};
use crate::layout::Orientation;
use crate::lengths::{LogicalLength, RectLengths};
use crate::{Coord, Property, SharedString, SharedVector};
pub use adapters::{FilterModel, MapModel, ReverseModel, SortModel};
use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::vec::Vec;
use core::cell::{Cell, RefCell};
use core::pin::Pin;
use euclid::num::Zero;
#[allow(unused)]
use euclid::num::{Ceil, Floor};
pub use model_peer::*;
use once_cell::unsync::OnceCell;
use pin_project::pin_project;

mod adapters;
mod model_peer;

type ItemTreeRc<C> = vtable::VRc<crate::item_tree::ItemTreeVTable, C>;

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
        if *self {
            1
        } else {
            0
        }
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

/// ItemTree that can be instantiated by a repeater.
pub trait RepeatedItemTree:
    crate::item_tree::ItemTree + vtable::HasStaticVTable<ItemTreeVTable> + 'static
{
    /// The data corresponding to the model
    type Data: 'static;

    /// Update this ItemTree at the given index and the given data
    fn update(&self, index: usize, data: Self::Data);

    /// Called once after the ItemTree has been instantiated and update()
    /// was called once.
    fn init(&self) {}

    /// Layout this item in the listview
    ///
    /// offset_y is the `y` position where this item should be placed.
    /// it should be updated to be to the y position of the next item.
    ///
    /// Returns the minimum item width which will be used to compute the listview's viewport width
    fn listview_layout(self: Pin<&Self>, _offset_y: &mut LogicalLength) -> LogicalLength {
        LogicalLength::default()
    }

    /// Returns what's needed to perform the layout if this ItemTrees is in a box layout
    fn box_layout_data(
        self: Pin<&Self>,
        _orientation: Orientation,
    ) -> crate::layout::BoxLayoutCellData {
        crate::layout::BoxLayoutCellData::default()
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum RepeatedInstanceState {
    /// The item is in a clean state
    Clean,
    /// The model data is stale and needs to be refreshed
    Dirty,
}
struct RepeaterInner<C: RepeatedItemTree> {
    instances: Vec<(RepeatedInstanceState, Option<ItemTreeRc<C>>)>,

    // The remaining properties only make sense for ListView
    /// The model row (index) of the first ItemTree in the `instances` vector.
    offset: usize,
    /// The average visible item height.
    cached_item_height: LogicalLength,
    /// The viewport_y last time the layout of the ListView was done
    previous_viewport_y: LogicalLength,
    /// the position of the item in the row `offset` (which corresponds to `instances[0]`).
    /// We will try to keep this constant when re-layouting items
    anchor_y: LogicalLength,
}

impl<C: RepeatedItemTree> Default for RepeaterInner<C> {
    fn default() -> Self {
        RepeaterInner {
            instances: Default::default(),
            offset: 0,
            cached_item_height: Default::default(),
            previous_viewport_y: Default::default(),
            anchor_y: Default::default(),
        }
    }
}

/// This struct is put in a component when using the `for` syntax
/// It helps instantiating the ItemTree `T`
#[pin_project]
pub struct RepeaterTracker<T: RepeatedItemTree> {
    inner: RefCell<RepeaterInner<T>>,
    #[pin]
    model: Property<ModelRc<T::Data>>,
    #[pin]
    is_dirty: Property<bool>,
    /// Only used for the list view to track if the scrollbar has changed and item needs to be laid out again.
    #[pin]
    listview_geometry_tracker: crate::properties::PropertyTracker,
}

impl<T: RepeatedItemTree> ModelChangeListener for RepeaterTracker<T> {
    /// Notify the peers that a specific row was changed
    fn row_changed(self: Pin<&Self>, row: usize) {
        let mut inner = self.inner.borrow_mut();
        let inner = &mut *inner;
        if let Some(c) = inner.instances.get_mut(row.wrapping_sub(inner.offset)) {
            if !self.model.is_dirty() {
                if let Some(comp) = c.1.as_ref() {
                    let model = self.project_ref().model.get_untracked();
                    if let Some(data) = model.row_data(row) {
                        comp.update(row, data);
                    }
                    c.0 = RepeatedInstanceState::Clean;
                }
            } else {
                c.0 = RepeatedInstanceState::Dirty;
            }
        }
    }
    /// Notify the peers that rows were added
    fn row_added(self: Pin<&Self>, mut index: usize, mut count: usize) {
        let mut inner = self.inner.borrow_mut();
        if index < inner.offset {
            if index + count < inner.offset {
                return;
            }
            count -= inner.offset - index;
            index = 0;
        } else {
            index -= inner.offset;
        }
        if count == 0 || index > inner.instances.len() {
            return;
        }
        self.is_dirty.set(true);
        inner.instances.splice(
            index..index,
            core::iter::repeat((RepeatedInstanceState::Dirty, None)).take(count),
        );
        for c in inner.instances[index + count..].iter_mut() {
            // Because all the indexes are dirty
            c.0 = RepeatedInstanceState::Dirty;
        }
    }
    /// Notify the peers that rows were removed
    fn row_removed(self: Pin<&Self>, mut index: usize, mut count: usize) {
        let mut inner = self.inner.borrow_mut();
        if index < inner.offset {
            if index + count < inner.offset {
                return;
            }
            count -= inner.offset - index;
            index = 0;
        } else {
            index -= inner.offset;
        }
        if count == 0 || index >= inner.instances.len() {
            return;
        }
        if (index + count) > inner.instances.len() {
            count = inner.instances.len() - index;
        }
        self.is_dirty.set(true);
        inner.instances.drain(index..(index + count));
        for c in inner.instances[index..].iter_mut() {
            // Because all the indexes are dirty
            c.0 = RepeatedInstanceState::Dirty;
        }
    }

    fn reset(self: Pin<&Self>) {
        self.is_dirty.set(true);
        self.inner.borrow_mut().instances.clear();
    }
}

impl<C: RepeatedItemTree> Default for RepeaterTracker<C> {
    fn default() -> Self {
        Self {
            inner: Default::default(),
            model: Property::new_named(ModelRc::default(), "i_slint_core::Repeater::model"),
            is_dirty: Property::new_named(false, "i_slint_core::Repeater::is_dirty"),
            listview_geometry_tracker: Default::default(),
        }
    }
}

#[pin_project]
pub struct Repeater<C: RepeatedItemTree>(#[pin] ModelChangeListenerContainer<RepeaterTracker<C>>);

impl<C: RepeatedItemTree> Default for Repeater<C> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<C: RepeatedItemTree + 'static> Repeater<C> {
    fn data(self: Pin<&Self>) -> Pin<&RepeaterTracker<C>> {
        self.project_ref().0.get()
    }

    fn model(self: Pin<&Self>) -> ModelRc<C::Data> {
        let model = self.data().project_ref().model;

        if model.is_dirty() {
            let old_model = model.get_internal();
            let m = model.get();
            if old_model != m {
                *self.data().inner.borrow_mut() = RepeaterInner::default();
                self.data().is_dirty.set(true);
                let peer = self.project_ref().0.model_peer();
                m.model_tracker().attach_peer(peer);
            }
            m
        } else {
            model.get()
        }
    }

    /// Call this function to make sure that the model is updated.
    /// The init function is the function to create a ItemTree
    pub fn ensure_updated(self: Pin<&Self>, init: impl Fn() -> ItemTreeRc<C>) {
        let model = self.model();
        if self.data().project_ref().is_dirty.get() {
            self.ensure_updated_impl(init, &model, model.row_count());
        }
    }

    // returns true if new items were created
    fn ensure_updated_impl(
        self: Pin<&Self>,
        init: impl Fn() -> ItemTreeRc<C>,
        model: &ModelRc<C::Data>,
        count: usize,
    ) -> bool {
        let mut indices_to_init = Vec::new();
        let mut inner = self.0.inner.borrow_mut();
        inner.instances.resize_with(count, || (RepeatedInstanceState::Dirty, None));
        let offset = inner.offset;
        let mut any_items_created = false;
        for (i, c) in inner.instances.iter_mut().enumerate() {
            if c.0 == RepeatedInstanceState::Dirty {
                if c.1.is_none() {
                    any_items_created = true;
                    c.1 = Some(init());
                    indices_to_init.push(i);
                };
                if let Some(data) = model.row_data(i + offset) {
                    c.1.as_ref().unwrap().update(i + offset, data);
                }
                c.0 = RepeatedInstanceState::Clean;
            }
        }
        self.data().is_dirty.set(false);

        drop(inner);
        let inner = self.0.inner.borrow();
        for item in indices_to_init.into_iter().filter_map(|index| inner.instances.get(index)) {
            item.1.as_ref().unwrap().init();
        }

        any_items_created
    }

    /// Same as `Self::ensure_updated` but for a ListView
    pub fn ensure_updated_listview(
        self: Pin<&Self>,
        init: impl Fn() -> ItemTreeRc<C>,
        viewport_width: Pin<&Property<LogicalLength>>,
        viewport_height: Pin<&Property<LogicalLength>>,
        viewport_y: Pin<&Property<LogicalLength>>,
        listview_width: LogicalLength,
        listview_height: Pin<&Property<LogicalLength>>,
    ) {
        // Query is_dirty to track model changes
        self.data().project_ref().is_dirty.get();
        self.data().project_ref().is_dirty.set(false);

        let mut vp_width = listview_width;
        let model = self.model();
        let row_count = model.row_count();
        let zero = LogicalLength::zero();
        if row_count == 0 {
            self.0.inner.borrow_mut().instances.clear();
            viewport_height.set(zero);
            viewport_y.set(zero);
            viewport_width.set(vp_width);
            return;
        }

        let listview_height = listview_height.get();
        let mut vp_y = viewport_y.get().min(zero);

        // We need some sort of estimation of the element height
        let cached_item_height = self.data().inner.borrow_mut().cached_item_height;
        let element_height = if cached_item_height > zero {
            cached_item_height
        } else {
            let total_height = Cell::new(zero);
            let count = Cell::new(0);
            let get_height_visitor = |x: &ItemTreeRc<C>| {
                let height = x.as_pin_ref().item_geometry(0).height_length();
                count.set(count.get() + 1);
                total_height.set(total_height.get() + height);
            };
            for c in self.data().inner.borrow().instances.iter() {
                if let Some(x) = c.1.as_ref() {
                    get_height_visitor(x);
                }
            }

            if count.get() > 0 {
                total_height.get() / (count.get() as Coord)
            } else {
                // There seems to be currently no items. Just instantiate one item.
                {
                    let mut inner = self.0.inner.borrow_mut();
                    inner.offset = inner.offset.min(row_count - 1);
                }

                self.ensure_updated_impl(&init, &model, 1);
                if let Some(c) = self.data().inner.borrow().instances.first() {
                    if let Some(x) = c.1.as_ref() {
                        get_height_visitor(x);
                    }
                } else {
                    panic!("Could not determine size of items");
                }
                total_height.get()
            }
        };

        let data = self.data();
        let mut inner = data.inner.borrow_mut();
        if inner.offset >= row_count {
            inner.offset = row_count - 1;
        }

        let one_and_a_half_screen = listview_height * 3 as Coord / 2 as Coord;
        let first_item_y = inner.anchor_y;
        let last_item_bottom = first_item_y + element_height * inner.instances.len() as Coord;

        let mut indices_to_init = Vec::new();

        let (mut new_offset, mut new_offset_y) = if first_item_y > -vp_y + one_and_a_half_screen
            || last_item_bottom + element_height < -vp_y
        {
            // We are jumping more than 1.5 screens, consider this as a random seek.
            inner.instances.clear();
            inner.offset = ((-vp_y / element_height).get().floor() as usize).min(row_count - 1);
            (inner.offset, zero)
        } else if vp_y < inner.previous_viewport_y {
            // we scrolled down, try to find out the new offset.
            let mut it_y = first_item_y + vp_y;
            let mut new_offset = inner.offset;
            debug_assert!(it_y <= zero); // we scrolled down, the anchor should be hidden
            for (i, c) in inner.instances.iter_mut().enumerate() {
                if c.0 == RepeatedInstanceState::Dirty {
                    if c.1.is_none() {
                        c.1 = Some(init());
                        indices_to_init.push(i);
                    }
                    if let Some(data) = model.row_data(new_offset) {
                        c.1.as_ref().unwrap().update(new_offset, data);
                    }
                    c.0 = RepeatedInstanceState::Clean;
                }
                let h = c.1.as_ref().unwrap().as_pin_ref().item_geometry(0).height_length();
                if it_y + h > zero || new_offset + 1 >= row_count {
                    break;
                }
                it_y += h;
                new_offset += 1;
            }
            (new_offset, it_y)
        } else {
            // We scrolled up, we'll instantiate items before offset in the loop
            (inner.offset, first_item_y + vp_y)
        };

        let mut loop_count = 0;
        loop {
            // If there is a gap before the new_offset and the beginning of the visible viewport,
            // try to fill it with items. First look at items that are before new_offset in the
            // inner.instances, if any.
            while new_offset > inner.offset && new_offset_y > zero {
                new_offset -= 1;
                new_offset_y -= inner.instances[new_offset - inner.offset]
                    .1
                    .as_ref()
                    .unwrap()
                    .as_pin_ref()
                    .item_geometry(0)
                    .height_length();
            }
            // If there is still a gap, fill it with new instances before
            let mut new_instances = Vec::new();
            while new_offset > 0 && new_offset_y > zero {
                new_offset -= 1;
                let new_instance = init();
                if let Some(data) = model.row_data(new_offset) {
                    new_instance.update(new_offset, data);
                }
                new_offset_y -= new_instance.as_pin_ref().item_geometry(0).height_length();
                new_instances.push(new_instance);
            }
            if !new_instances.is_empty() {
                for x in &mut indices_to_init {
                    *x += new_instances.len();
                }
                indices_to_init.extend(0..new_instances.len());
                inner.instances.splice(
                    0..0,
                    new_instances
                        .into_iter()
                        .rev()
                        .map(|c| (RepeatedInstanceState::Clean, Some(c))),
                );
                inner.offset = new_offset;
            }
            assert!(
                new_offset >= inner.offset && new_offset <= inner.offset + inner.instances.len()
            );

            // Now we will layout items until we fit the view, starting with the ones that are already instantiated
            let mut y = new_offset_y;
            let mut idx = new_offset;
            let instances_begin = new_offset - inner.offset;
            for c in &mut inner.instances[instances_begin..] {
                if idx >= row_count {
                    break;
                }
                if c.0 == RepeatedInstanceState::Dirty {
                    if c.1.is_none() {
                        c.1 = Some(init());
                        indices_to_init.push(instances_begin + idx - new_offset)
                    }
                    if let Some(data) = model.row_data(idx) {
                        c.1.as_ref().unwrap().update(idx, data);
                    }
                    c.0 = RepeatedInstanceState::Clean;
                }
                if let Some(x) = c.1.as_ref() {
                    vp_width = vp_width.max(x.as_pin_ref().listview_layout(&mut y));
                }
                idx += 1;
                if y >= listview_height {
                    break;
                }
            }

            // create more items until there is no more room.
            while y < listview_height && idx < row_count {
                let new_instance = init();
                if let Some(data) = model.row_data(idx) {
                    new_instance.update(idx, data);
                }
                vp_width = vp_width.max(new_instance.as_pin_ref().listview_layout(&mut y));
                indices_to_init.push(inner.instances.len());
                inner.instances.push((RepeatedInstanceState::Clean, Some(new_instance)));
                idx += 1;
            }
            if y < listview_height && vp_y < zero && loop_count < 3 {
                assert!(idx >= row_count);
                // we reached the end of the model, and we still have room. scroll a bit up.
                vp_y += listview_height - y;
                loop_count += 1;
                continue;
            }

            // Let's cleanup the instances that are not shown.
            if new_offset != inner.offset {
                let instances_begin = new_offset - inner.offset;
                inner.instances.splice(0..instances_begin, core::iter::empty());
                indices_to_init.retain_mut(|idx| {
                    if *idx < instances_begin {
                        false
                    } else {
                        *idx -= instances_begin;
                        true
                    }
                });
                inner.offset = new_offset;
            }
            if inner.instances.len() != idx - new_offset {
                inner.instances.splice(idx - new_offset.., core::iter::empty());
                indices_to_init.retain(|x| *x < idx - new_offset);
            }

            if inner.instances.is_empty() {
                break;
            }

            // Now re-compute some coordinate such a way that the scrollbar are adjusted.
            inner.cached_item_height = (y - new_offset_y) / inner.instances.len() as Coord;
            inner.anchor_y = inner.cached_item_height * inner.offset as Coord;
            viewport_height.set(inner.cached_item_height * row_count as Coord);
            viewport_width.set(vp_width);
            let new_viewport_y = -inner.anchor_y + new_offset_y;
            viewport_y.set(new_viewport_y);
            inner.previous_viewport_y = new_viewport_y;
            break;
        }
        drop(inner);
        let inner = self.0.inner.borrow();
        for item in indices_to_init.into_iter().filter_map(|index| inner.instances.get(index)) {
            item.1.as_ref().unwrap().init();
        }
    }

    /// Sets the data directly in the model
    pub fn model_set_row_data(self: Pin<&Self>, row: usize, data: C::Data) {
        let model = self.model();
        model.set_row_data(row, data);
    }

    /// Set the model binding
    pub fn set_model_binding(&self, binding: impl Fn() -> ModelRc<C::Data> + 'static) {
        self.0.model.set_binding(binding);
    }

    /// Call the visitor for the root of each instance
    pub fn visit(
        &self,
        order: TraversalOrder,
        mut visitor: crate::item_tree::ItemVisitorRefMut,
    ) -> crate::item_tree::VisitChildrenResult {
        // We can't keep self.inner borrowed because the event might modify the model
        let count = self.0.inner.borrow().instances.len() as u32;
        for i in 0..count {
            let i = if order == TraversalOrder::BackToFront { i } else { count - i - 1 };
            let c = self.0.inner.borrow().instances.get(i as usize).and_then(|c| c.1.clone());
            if let Some(c) = c {
                if c.as_pin_ref().visit_children_item(-1, order, visitor.borrow_mut()).has_aborted()
                {
                    return crate::item_tree::VisitChildrenResult::abort(i, 0);
                }
            }
        }
        crate::item_tree::VisitChildrenResult::CONTINUE
    }

    /// Return the amount of instances currently in the repeater
    pub fn len(&self) -> usize {
        self.0.inner.borrow().instances.len()
    }

    /// Return the range of indices used by this Repeater.
    ///
    /// Two values are necessary here since the Repeater can start to insert the data from its
    /// model at an offset.
    pub fn range(&self) -> core::ops::Range<usize> {
        let inner = self.0.inner.borrow();
        core::ops::Range { start: inner.offset, end: inner.offset + inner.instances.len() }
    }

    /// Return the instance for the given model index.
    /// The index should be within [`Self::range()`]
    pub fn instance_at(&self, index: usize) -> Option<ItemTreeRc<C>> {
        let inner = self.0.inner.borrow();
        inner
            .instances
            .get(index.checked_sub(inner.offset)?)
            .map(|c| c.1.clone().expect("That was updated before!"))
    }

    /// Return true if the Repeater as empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns a vector containing all instances
    pub fn instances_vec(&self) -> Vec<ItemTreeRc<C>> {
        self.0.inner.borrow().instances.iter().flat_map(|x| x.1.clone()).collect()
    }
}

#[pin_project]
pub struct Conditional<C: RepeatedItemTree> {
    #[pin]
    model: Property<bool>,
    instance: RefCell<Option<ItemTreeRc<C>>>,
}

impl<C: RepeatedItemTree> Default for Conditional<C> {
    fn default() -> Self {
        Self {
            model: Property::new_named(false, "i_slint_core::Conditional::model"),
            instance: RefCell::new(None),
        }
    }
}

impl<C: RepeatedItemTree + 'static> Conditional<C> {
    /// Call this function to make sure that the model is updated.
    /// The init function is the function to create a ItemTree
    pub fn ensure_updated(self: Pin<&Self>, init: impl Fn() -> ItemTreeRc<C>) {
        let model = self.project_ref().model.get();

        if !model {
            drop(self.instance.replace(None));
        } else if self.instance.borrow().is_none() {
            let i = init();
            self.instance.replace(Some(i.clone()));
            i.init();
        }
    }

    /// Set the model binding
    pub fn set_model_binding(&self, binding: impl Fn() -> bool + 'static) {
        self.model.set_binding(binding);
    }

    /// Call the visitor for the root of each instance
    pub fn visit(
        &self,
        order: TraversalOrder,
        mut visitor: crate::item_tree::ItemVisitorRefMut,
    ) -> crate::item_tree::VisitChildrenResult {
        // We can't keep self.inner borrowed because the event might modify the model
        let instance = self.instance.borrow().clone();
        if let Some(c) = instance {
            if c.as_pin_ref().visit_children_item(-1, order, visitor.borrow_mut()).has_aborted() {
                return crate::item_tree::VisitChildrenResult::abort(0, 0);
            }
        }

        crate::item_tree::VisitChildrenResult::CONTINUE
    }

    /// Return the amount of instances (1 if the conditional is active, 0 otherwise)
    pub fn len(&self) -> usize {
        self.instance.borrow().is_some() as usize
    }

    /// Return the range of indices used by this Conditional.
    ///
    /// Similar to Repeater::range, but the range is always [0, 1] if the Conditional is active.
    pub fn range(&self) -> core::ops::Range<usize> {
        0..self.len()
    }

    /// Return the instance for the given model index.
    /// The index should be within [`Self::range()`]
    pub fn instance_at(&self, index: usize) -> Option<ItemTreeRc<C>> {
        if index != 0 {
            return None;
        }
        self.instance.borrow().clone()
    }

    /// Return true if the Repeater as empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns a vector containing all instances
    pub fn instances_vec(&self) -> Vec<ItemTreeRc<C>> {
        self.instance.borrow().clone().into_iter().collect()
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

    #[test]
    fn test_tracking_model_handle() {
        let model: Rc<VecModel<u8>> = Rc::new(Default::default());
        let handle = ModelRc::from(model.clone() as Rc<dyn Model<Data = u8>>);
        let tracker = Box::pin(crate::properties::PropertyTracker::default());
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
        let tracker = Box::pin(crate::properties::PropertyTracker::default());
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

        model.set_vec(vec![]);
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
