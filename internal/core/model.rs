// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

// cSpell: ignore vecmodel

//! Model and Repeater

use crate::component::ComponentVTable;
use crate::item_tree::TraversalOrder;
use crate::items::ItemRef;
use crate::layout::Orientation;
use crate::lengths::{LogicalLength, RectLengths};
use crate::{Coord, Property, SharedString, SharedVector};
pub use adapters::{FilterModel, MapModel, SortModel};
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::cell::{Cell, RefCell};
use core::pin::Pin;
use euclid::num::Zero;
#[allow(unused)]
use euclid::num::{Ceil, Floor};
pub use model_peer::*;
use once_cell::unsync::OnceCell;
use pin_project::pin_project;
use pin_weak::rc::{PinWeak, Rc};

mod adapters;
mod model_peer;

type ComponentRc<C> = vtable::VRc<crate::component::ComponentVTable, C>;

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

/// A Model is providing Data for the Repeater or ListView elements of the `.slint` language
///
/// If the model can be changed, the type implementing the Model trait should holds
/// a [`ModelNotify`], and is responsible to call functions on it to let the UI know that
/// something has changed.
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
    /// The model data: A model is a set of row and each row has this data
    type Data;
    /// The amount of row in the model
    fn row_count(&self) -> usize;
    /// Returns the data for a particular row. This function should be called with `row < row_count()`.
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
        eprintln!(
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
    fn iter(&self) -> ModelIterator<Self::Data>
    where
        Self: Sized,
    {
        ModelIterator::new(self)
    }

    /// Return something that can be downcast'ed (typically self)
    ///
    /// This is useful to get back to the actual model from a [`ModelRc`] stored
    /// in a component.
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
    /// Note: the default implementation returns nothing interesting. this method should be
    /// implemented by model implementation to return something useful. For example:
    /// ```ignore
    /// fn as_any(&self) -> &dyn core::any::Any { self }
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

impl<'a, T> Iterator for ModelIterator<'a, T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        let row = self.row;
        if self.row < self.model.row_count() {
            self.row += 1;
        }
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

impl<'a, T> ExactSizeIterator for ModelIterator<'a, T> {}

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

/// A model backed by a `Vec<T>`
#[derive(Default)]
pub struct VecModel<T> {
    array: RefCell<Vec<T>>,
    notify: ModelNotify,
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
        (row < self.row_count()).then(|| row as i32)
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
        (row < self.row_count()).then(|| ())
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        &()
    }
}

/// A Reference counted [`Model`].
///
/// The `ModelRc` struct holds something that implements the [`Model`] trait.
/// This is used in `for` expressions in the .slint language.
/// Array properties in the .slint language are holding a ModelRc.
///
/// An empty model can be constructed with [`ModelRc::default()`].
/// Use [`ModelRc::new()`] To construct a ModelRc from something that implements the
/// [`Model`] trait.
/// It is also possible to use the [`From`] trait to convert from `Rc<dyn Model>`.

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

/// Component that can be instantiated by a repeater.
pub trait RepeatedComponent:
    crate::component::Component + vtable::HasStaticVTable<ComponentVTable> + 'static
{
    /// The data corresponding to the model
    type Data: 'static;

    /// Update this component at the given index and the given data
    fn update(&self, index: usize, data: Self::Data);

    /// Called once after the component has been instantiated and update()
    /// was called once.
    fn init(&self) {}

    /// Layout this item in the listview
    ///
    /// offset_y is the `y` position where this item should be placed.
    /// it should be updated to be to the y position of the next item.
    fn listview_layout(
        self: Pin<&Self>,
        _offset_y: &mut LogicalLength,
        _viewport_width: Pin<&Property<LogicalLength>>,
    ) {
    }

    /// Returns what's needed to perform the layout if this component is in a box layout
    fn box_layout_data(
        self: Pin<&Self>,
        _orientation: Orientation,
    ) -> crate::layout::BoxLayoutCellData {
        crate::layout::BoxLayoutCellData::default()
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum RepeatedComponentState {
    /// The item is in a clean state
    Clean,
    /// The model data is stale and needs to be refreshed
    Dirty,
}
struct RepeaterInner<C: RepeatedComponent> {
    components: Vec<(RepeatedComponentState, Option<ComponentRc<C>>)>,

    // The remaining properties only make sense for ListView
    /// The model row (index) of the first component in the `components` vector.
    offset: usize,
    /// The average visible item height.
    cached_item_height: LogicalLength,
    /// The viewport_y last time the layout of the ListView was done
    previous_viewport_y: LogicalLength,
    /// the position of the item in the row `offset` (which corresponds to `components[0]`).
    /// We will try to keep this constant when re-layouting items
    anchor_y: LogicalLength,
}

impl<C: RepeatedComponent> Default for RepeaterInner<C> {
    fn default() -> Self {
        RepeaterInner {
            components: Default::default(),
            offset: 0,
            cached_item_height: Default::default(),
            previous_viewport_y: Default::default(),
            anchor_y: Default::default(),
        }
    }
}

/// This field is put in a component when using the `for` syntax
/// It helps instantiating the components `C`
#[pin_project]
pub struct RepeaterTracker<C: RepeatedComponent> {
    inner: RefCell<RepeaterInner<C>>,
    #[pin]
    model: Property<ModelRc<C::Data>>,
    #[pin]
    is_dirty: Property<bool>,
    /// Only used for the list view to track if the scrollbar has changed and item needs to be layed out again.
    #[pin]
    listview_geometry_tracker: crate::properties::PropertyTracker,
}

impl<C: RepeatedComponent> ModelChangeListener for RepeaterTracker<C> {
    /// Notify the peers that a specific row was changed
    fn row_changed(&self, row: usize) {
        self.is_dirty.set(true);
        let mut inner = self.inner.borrow_mut();
        let inner = &mut *inner;
        if let Some(c) = inner.components.get_mut(row.wrapping_sub(inner.offset)) {
            c.0 = RepeatedComponentState::Dirty;
        }
    }
    /// Notify the peers that rows were added
    fn row_added(&self, mut index: usize, mut count: usize) {
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
        if count == 0 || index > inner.components.len() {
            return;
        }
        self.is_dirty.set(true);
        inner.components.splice(
            index..index,
            core::iter::repeat((RepeatedComponentState::Dirty, None)).take(count),
        );
        for c in inner.components[index + count..].iter_mut() {
            // Because all the indexes are dirty
            c.0 = RepeatedComponentState::Dirty;
        }
    }
    /// Notify the peers that rows were removed
    fn row_removed(&self, mut index: usize, mut count: usize) {
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
        if count == 0 || index >= inner.components.len() {
            return;
        }
        if (index + count) > inner.components.len() {
            count = inner.components.len() - index;
        }
        self.is_dirty.set(true);
        inner.components.drain(index..(index + count));
        for c in inner.components[index..].iter_mut() {
            // Because all the indexes are dirty
            c.0 = RepeatedComponentState::Dirty;
        }
    }

    fn reset(&self) {
        self.is_dirty.set(true);
        self.inner.borrow_mut().components.clear();
    }
}

impl<C: RepeatedComponent> Default for RepeaterTracker<C> {
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
pub struct Repeater<C: RepeatedComponent>(#[pin] ModelChangeListenerContainer<RepeaterTracker<C>>);

impl<C: RepeatedComponent> Default for Repeater<C> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<C: RepeatedComponent + 'static> Repeater<C> {
    fn data(self: Pin<&Self>) -> Pin<&RepeaterTracker<C>> {
        self.project_ref().0.get()
    }

    fn model(self: Pin<&Self>) -> ModelRc<C::Data> {
        // Safety: Repeater does not implement drop and never allows access to model as mutable
        let model = self.data().project_ref().model;

        if model.is_dirty() {
            self.data()
                .inner
                .borrow_mut()
                .components
                .iter_mut()
                .for_each(|c| c.0 = RepeatedComponentState::Dirty);

            self.data().is_dirty.set(true);
            let m = model.get();
            let peer = self.project_ref().0.model_peer();
            m.model_tracker().attach_peer(peer);
            m
        } else {
            model.get()
        }
    }

    /// Call this function to make sure that the model is updated.
    /// The init function is the function to create a component
    pub fn ensure_updated(self: Pin<&Self>, init: impl Fn() -> ComponentRc<C>) {
        let model = self.model();
        if self.data().project_ref().is_dirty.get() {
            self.ensure_updated_impl(init, &model, model.row_count());
        }
    }

    // returns true if new items were created
    fn ensure_updated_impl(
        self: Pin<&Self>,
        init: impl Fn() -> ComponentRc<C>,
        model: &ModelRc<C::Data>,
        count: usize,
    ) -> bool {
        let mut inner = self.0.inner.borrow_mut();
        inner.components.resize_with(count, || (RepeatedComponentState::Dirty, None));
        let offset = inner.offset;
        let mut any_items_created = false;
        for (i, c) in inner.components.iter_mut().enumerate() {
            if c.0 == RepeatedComponentState::Dirty {
                let created = if c.1.is_none() {
                    any_items_created = true;
                    c.1 = Some(init());
                    true
                } else {
                    false
                };
                c.1.as_ref().unwrap().update(i + offset, model.row_data(i + offset).unwrap());
                if created {
                    c.1.as_ref().unwrap().init();
                }
                c.0 = RepeatedComponentState::Clean;
            }
        }
        self.data().is_dirty.set(false);
        any_items_created
    }

    /// Same as `Self::ensuer_updated` but for a ListView
    pub fn ensure_updated_listview(
        self: Pin<&Self>,
        init: impl Fn() -> ComponentRc<C>,
        viewport_width: Pin<&Property<LogicalLength>>,
        viewport_height: Pin<&Property<LogicalLength>>,
        viewport_y: Pin<&Property<LogicalLength>>,
        listview_width: LogicalLength,
        listview_height: Pin<&Property<LogicalLength>>,
    ) {
        viewport_width.set(listview_width);
        let model = self.model();
        let row_count = model.row_count();
        if row_count == 0 {
            self.0.inner.borrow_mut().components.clear();
            viewport_height.set(LogicalLength::zero());
            viewport_y.set(LogicalLength::zero());

            return;
        }

        let listview_height = listview_height.get();
        let mut vp_y = viewport_y.get().min(LogicalLength::zero());

        // We need some sort of estimation of the element height
        let cached_item_height = self.data().inner.borrow_mut().cached_item_height;
        let element_height = if cached_item_height > LogicalLength::zero() {
            cached_item_height
        } else {
            let total_height = Cell::new(LogicalLength::zero());
            let count = Cell::new(0);
            let get_height_visitor = |item: Pin<ItemRef>| {
                count.set(count.get() + 1);
                let height = item.as_ref().geometry().height_length();
                total_height.set(total_height.get() + height);
            };
            for c in self.data().inner.borrow().components.iter() {
                if let Some(x) = c.1.as_ref() {
                    get_height_visitor(x.as_pin_ref().get_item_ref(0));
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
                if let Some(c) = self.data().inner.borrow().components.get(0) {
                    if let Some(x) = c.1.as_ref() {
                        get_height_visitor(x.as_pin_ref().get_item_ref(0));
                    }
                } else {
                    panic!("Could not determine size of items");
                }
                total_height.get()
            }
        };

        let data = self.data();
        let mut inner = data.inner.borrow_mut();
        let one_and_a_half_screen = listview_height * 3 as Coord / 2 as Coord;
        let first_item_y = inner.anchor_y;
        let last_item_bottom = first_item_y + element_height * inner.components.len() as Coord;

        let (mut new_offset, mut new_offset_y) = if first_item_y > -vp_y + one_and_a_half_screen
            || last_item_bottom + element_height < -vp_y
        {
            // We are jumping more than 1.5 screens, consider this as a random seek.
            inner.components.clear();
            inner.offset = ((-vp_y / element_height).get().floor() as usize).min(row_count - 1);
            (inner.offset, -vp_y)
        } else if vp_y < inner.previous_viewport_y {
            // we scrolled down, try to find out the new offset.
            let mut it_y = first_item_y;
            let mut new_offset = inner.offset;
            debug_assert!(it_y <= -vp_y); // we scrolled down, the anchor should be hidden
            for c in inner.components.iter_mut() {
                if c.0 == RepeatedComponentState::Dirty {
                    if c.1.is_none() {
                        c.1 = Some(init());
                    }
                    c.1.as_ref().unwrap().update(new_offset, model.row_data(new_offset).unwrap());
                    c.0 = RepeatedComponentState::Clean;
                }
                let h =
                    c.1.as_ref()
                        .unwrap()
                        .as_pin_ref()
                        .get_item_ref(0)
                        .as_ref()
                        .geometry()
                        .height_length();
                if it_y + h >= -vp_y || new_offset + 1 >= row_count {
                    break;
                }
                it_y += h;
                new_offset += 1;
            }
            (new_offset, it_y)
        } else {
            // We scrolled up, we'll instantiate items before offset in the loop
            (inner.offset, first_item_y)
        };

        loop {
            // If there is a gap before the new_offset and the beginning of the visible viewport,
            // try to fill it with items. First look at items that are before new_offset in the
            // inner.components, if any.
            while new_offset > inner.offset && new_offset_y > -vp_y {
                new_offset -= 1;
                new_offset_y -= inner.components[new_offset - inner.offset]
                    .1
                    .as_ref()
                    .unwrap()
                    .as_pin_ref()
                    .get_item_ref(0)
                    .as_ref()
                    .geometry()
                    .height_length();
            }
            // If there is still a gap, fill it with new component before
            let mut new_components = Vec::new();
            while new_offset > 0 && new_offset_y > -vp_y {
                new_offset -= 1;
                let new_component = init();
                new_component.update(new_offset, model.row_data(new_offset).unwrap());
                new_offset_y -=
                    new_component.as_pin_ref().get_item_ref(0).as_ref().geometry().height_length();
                new_components.push(new_component);
            }
            if !new_components.is_empty() {
                inner.components.splice(
                    0..0,
                    new_components
                        .into_iter()
                        .rev()
                        .map(|c| (RepeatedComponentState::Clean, Some(c))),
                );
                inner.offset = new_offset;
            }
            assert!(
                new_offset >= inner.offset && new_offset <= inner.offset + inner.components.len()
            );

            // Now we will layout items until we fit the view, starting with the ones that are already instantiated
            let mut y = new_offset_y;
            let mut idx = new_offset;
            let components_begin = new_offset - inner.offset;
            for c in &mut inner.components[components_begin..] {
                if c.0 == RepeatedComponentState::Dirty {
                    if c.1.is_none() {
                        c.1 = Some(init());
                    }
                    c.1.as_ref().unwrap().update(idx, model.row_data(idx).unwrap());
                    c.0 = RepeatedComponentState::Clean;
                }
                if let Some(x) = c.1.as_ref() {
                    x.as_pin_ref().listview_layout(&mut y, viewport_width);
                }
                idx += 1;
                if y >= -vp_y + listview_height {
                    break;
                }
            }

            // create more items until there is no more room.
            while y < -vp_y + listview_height && idx < row_count {
                let new_component = init();
                new_component.update(idx, model.row_data(idx).unwrap());
                new_component.as_pin_ref().listview_layout(&mut y, viewport_width);
                inner.components.push((RepeatedComponentState::Clean, Some(new_component)));
                idx += 1;
            }
            if y < -vp_y + listview_height && vp_y < LogicalLength::zero() {
                assert!(idx >= row_count);
                // we reached the end of the model, and we still have room. scroll a bit up.
                vp_y = listview_height - y;
                continue;
            }

            // Let's cleanup the components that are not shown.
            if new_offset != inner.offset {
                let components_begin = new_offset - inner.offset;
                inner.components.splice(0..components_begin, core::iter::empty());
                inner.offset = new_offset;
            }
            if inner.components.len() != idx - new_offset {
                inner.components.splice(idx - new_offset.., core::iter::empty());
            }

            // Now re-compute some coordinate such a way that the scrollbar are adjusted.
            inner.cached_item_height = (y - new_offset_y) / inner.components.len() as Coord;
            inner.anchor_y = inner.cached_item_height * inner.offset as Coord;
            viewport_height.set(inner.cached_item_height * row_count as Coord);
            let new_viewport_y = -inner.anchor_y + vp_y + new_offset_y;
            viewport_y.set(new_viewport_y);
            inner.previous_viewport_y = new_viewport_y;
            break;
        }
    }

    /// Sets the data directly in the model
    pub fn model_set_row_data(self: Pin<&Self>, row: usize, data: C::Data) {
        let model = self.model();
        model.set_row_data(row, data);
        if let Some(c) = self.data().inner.borrow_mut().components.get_mut(row) {
            if c.0 == RepeatedComponentState::Dirty {
                if let Some(comp) = c.1.as_ref() {
                    comp.update(row, model.row_data(row).unwrap());
                    c.0 = RepeatedComponentState::Clean;
                }
            }
        }
    }

    /// Set the model binding
    pub fn set_model_binding(&self, binding: impl Fn() -> ModelRc<C::Data> + 'static) {
        self.0.model.set_binding(binding);
    }

    /// Call the visitor for each component
    pub fn visit(
        &self,
        order: TraversalOrder,
        mut visitor: crate::item_tree::ItemVisitorRefMut,
    ) -> crate::item_tree::VisitChildrenResult {
        // We can't keep self.inner borrowed because the event might modify the model
        let count = self.0.inner.borrow().components.len();
        for i in 0..count {
            let i = if order == TraversalOrder::BackToFront { i } else { count - i - 1 };
            let c = self.0.inner.borrow().components.get(i).and_then(|c| c.1.clone());
            if let Some(c) = c {
                if c.as_pin_ref().visit_children_item(-1, order, visitor.borrow_mut()).has_aborted()
                {
                    return crate::item_tree::VisitChildrenResult::abort(i, 0);
                }
            }
        }
        crate::item_tree::VisitChildrenResult::CONTINUE
    }

    /// Return the amount of item currently in the component
    pub fn len(&self) -> usize {
        self.0.inner.borrow().components.len()
    }

    /// Return the range of indices used by this Repeater.
    ///
    /// Two values are necessary here since the Repeater can start to insert the data from its
    /// model at an offset.
    pub fn range(&self) -> core::ops::Range<usize> {
        let inner = self.0.inner.borrow();
        core::ops::Range { start: inner.offset, end: inner.offset + inner.components.len() }
    }

    /// Return the component instance for the given model index.
    /// The index should be within [`Self::range()`]
    pub fn component_at(&self, index: usize) -> Option<ComponentRc<C>> {
        let inner = self.0.inner.borrow();
        inner
            .components
            .get(index - inner.offset)
            .map(|c| c.1.clone().expect("That was updated before!"))
    }

    /// Return true if the Repeater as empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns a vector containing all components
    pub fn components_vec(&self) -> Vec<ComponentRc<C>> {
        self.0.inner.borrow().components.iter().flat_map(|x| x.1.clone()).collect()
    }
}

/// Represent an item in a StandardListView
#[repr(C)]
#[derive(Clone, Default, Debug, PartialEq)]
pub struct StandardListViewItem {
    /// The text content of the item
    pub text: crate::SharedString,
}

impl From<&str> for StandardListViewItem {
    fn from(other: &str) -> Self {
        return Self { text: other.into() };
    }
}

impl From<SharedString> for StandardListViewItem {
    fn from(other: SharedString) -> Self {
        return Self { text: other };
    }
}

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

#[test]
fn test_vecmodel_set_vec() {
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
        fn row_changed(&self, row: usize) {
            self.changed_rows.borrow_mut().push((row, self.row_count()));
        }

        fn row_added(&self, index: usize, count: usize) {
            self.added_rows.borrow_mut().push((index, count, self.row_count()));
        }

        fn row_removed(&self, index: usize, count: usize) {
            self.removed_rows.borrow_mut().push((index, count, self.row_count()));
        }
        fn reset(&self) {
            *self.reset.borrow_mut() += 1;
        }
    }

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
}
