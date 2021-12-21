// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

//! Model and Repeater

// Safety: we use pointer to Repeater in the DependencyList, bue the Drop of the Repeater
// will remove them from the list so it will not be accessed after it is dropped
#![allow(unsafe_code)]

use crate::item_tree::TraversalOrder;
use crate::items::ItemRef;
use crate::layout::Orientation;
use crate::properties::dependency_tracker::DependencyNode;
use crate::Property;
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::cell::{Cell, RefCell};
use core::pin::Pin;
use once_cell::unsync::OnceCell;
use pin_project::pin_project;
use pin_weak::rc::{PinWeak, Rc};

#[cfg(not(feature = "std"))]
use num_traits::float::Float;

type DependencyListHead =
    crate::properties::dependency_tracker::DependencyListHead<*const dyn ErasedRepeater>;
type ComponentRc<C> = vtable::VRc<crate::component::ComponentVTable, C>;

/// Represent a handle to a view that listens to changes to a model.
///
/// See [`Model::attach_peer`] and [`ModelNotify`]
#[derive(Clone)]
pub struct ModelPeer {
    // FIXME: add a lifetime to ModelPeer so we can put the DependencyNode directly in the Repeater
    inner: PinWeak<DependencyNode<*const dyn ErasedRepeater>>,
}

/// This trait defines the interface that users of a model can use to track changes
/// to a model. It is supplied via [`Model::model_tracker`] and implementation usually
/// return a reference to its field of [`ModelNotify`].
pub trait ModelTracker {
    /// Attach one peer. The peer will be notified when the model changes
    fn attach_peer(&self, peer: ModelPeer);
    /// Register the model as a dependency to the current binding being evaluated, so
    /// that it will be notified when the model changes its size.
    fn track_row_count_changes(&self);
}

impl ModelTracker for () {
    fn attach_peer(&self, _peer: ModelPeer) {}

    fn track_row_count_changes(&self) {}
}

#[pin_project]
#[derive(Default)]
struct ModelNotifyInner {
    #[pin]
    model_dirty_property: Property<()>,
    #[pin]
    peers: DependencyListHead,
}

/// Dispatch notifications from a [`Model`] to one or several [`ModelPeer`].
/// Typically, you would want to put this in the implementation of the Model
#[derive(Default)]
pub struct ModelNotify {
    inner: OnceCell<Pin<Box<ModelNotifyInner>>>,
}

impl ModelNotify {
    fn inner(&self) -> Pin<&ModelNotifyInner> {
        self.inner.get_or_init(|| Box::pin(ModelNotifyInner::default())).as_ref()
    }

    /// Notify the peers that a specific row was changed
    pub fn row_changed(&self, row: usize) {
        if let Some(inner) = self.inner.get() {
            inner.as_ref().project_ref().peers.for_each(|p| unsafe { &**p }.row_changed(row))
        }
    }
    /// Notify the peers that rows were added
    pub fn row_added(&self, index: usize, count: usize) {
        if let Some(inner) = self.inner.get() {
            inner.model_dirty_property.mark_dirty();
            inner.as_ref().project_ref().peers.for_each(|p| unsafe { &**p }.row_added(index, count))
        }
    }
    /// Notify the peers that rows were removed
    pub fn row_removed(&self, index: usize, count: usize) {
        if let Some(inner) = self.inner.get() {
            inner.model_dirty_property.mark_dirty();
            inner
                .as_ref()
                .project_ref()
                .peers
                .for_each(|p| unsafe { &**p }.row_removed(index, count))
        }
    }
    /// Attach one peer. The peer will be notified when the model changes
    #[deprecated(
        note = "In your model, re-implement `model_tracker` of the `Model` trait instead of calling this function from our `attach_peer` implementation"
    )]
    pub fn attach(&self, peer: ModelPeer) {
        self.attach_peer(peer)
    }
}

impl ModelTracker for ModelNotify {
    /// Attach one peer. The peer will be notified when the model changes
    fn attach_peer(&self, peer: ModelPeer) {
        if let Some(peer) = peer.inner.upgrade() {
            self.inner().project_ref().peers.append(peer.as_ref())
        }
    }

    fn track_row_count_changes(&self) {
        self.inner().project_ref().model_dirty_property.get();
    }
}

/// A Model is providing Data for the Repeater or ListView elements of the `.60` language
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
/// # use sixtyfps_corelib::model::{Model, ModelNotify, ModelPeer, ModelTracker};
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
///     fn row_data(&self, row: usize) -> Self::Data {
///         self.array.borrow()[row].clone()
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
    fn row_data(&self, row: usize) -> Self::Data;
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
    /// The implementation should forward to [`ModelNotify::attach`]
    #[deprecated(note = "Re-implement model_tracker instead of this function")]
    fn attach_peer(&self, peer: ModelPeer) {
        self.model_tracker().attach_peer(peer)
    }

    /// The implementation should return a reference to its [`ModelNotify`] field.
    fn model_tracker(&self) -> &dyn ModelTracker {
        &()
    }

    /// Returns an iterator visiting all elements of the model.
    fn iter(&self) -> ModelIterator<Self::Data>
    where
        Self: Sized,
    {
        ModelIterator { model: self, row: 0 }
    }

    /// Return something that can be downcast'ed (typically self)
    ///
    /// This is useful to get back to the actual model from a ModelHandle stored
    /// in a component.
    ///
    /// ```
    /// # use sixtyfps_corelib::model::*;
    /// # use std::rc::Rc;
    /// let vec_model = Rc::new(VecModel::from(vec![1i32, 2, 3]));
    /// let handle = ModelHandle::from(vec_model as Rc<dyn Model<Data = i32>>);
    /// // later:
    /// handle.as_any().downcast_ref::<VecModel<i32>>().unwrap().push(4);
    /// assert_eq!(handle.row_data(3), 4);
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

/// An iterator over the elements of a model.
/// This struct is created by the [Model::iter()] trait function.
pub struct ModelIterator<'a, T> {
    model: &'a dyn Model<Data = T>,
    row: usize,
}

impl<'a, T> Iterator for ModelIterator<'a, T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.row < self.model.row_count() {
            let row = self.row;
            self.row += 1;
            Some(self.model.row_data(row))
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.model.row_count();
        (len, Some(len))
    }
}

impl<'a, T> ExactSizeIterator for ModelIterator<'a, T> {}

/// A model backed by a `Vec<T>`
#[derive(Default)]
pub struct VecModel<T> {
    array: RefCell<Vec<T>>,
    notify: ModelNotify,
}

impl<T: 'static> VecModel<T> {
    /// Allocate a new model from a slice
    pub fn from_slice(slice: &[T]) -> ModelHandle<T>
    where
        T: Clone,
    {
        ModelHandle(Some(Rc::<Self>::new(slice.to_vec().into())))
    }

    /// Add a row at the end of the model
    pub fn push(&self, value: T) {
        self.array.borrow_mut().push(value);
        self.notify.row_added(self.array.borrow().len() - 1, 1)
    }

    /// Remove the row at the given index from the model
    pub fn remove(&self, index: usize) {
        self.array.borrow_mut().remove(index);
        self.notify.row_removed(index, 1)
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

    fn row_data(&self, row: usize) -> Self::Data {
        self.array.borrow()[row].clone()
    }

    fn set_row_data(&self, row: usize, data: Self::Data) {
        self.array.borrow_mut()[row] = data;
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

    fn row_data(&self, row: usize) -> Self::Data {
        row as i32
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
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

    fn row_data(&self, _row: usize) -> Self::Data {}

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}

/// Properties of type array in the .60 language are represented as
/// an [`Option`] of an [`Rc`] of something implemented the [`Model`] trait
#[derive(derive_more::Deref, derive_more::DerefMut, derive_more::From, derive_more::Into)]
pub struct ModelHandle<T>(pub Option<Rc<dyn Model<Data = T>>>);

impl<T> core::fmt::Debug for ModelHandle<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "ModelHandle({:?})", self.0.as_ref().map(|_| "dyn Model"))
    }
}

impl<T> Clone for ModelHandle<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<T> Default for ModelHandle<T> {
    fn default() -> Self {
        Self(None)
    }
}

impl<T> core::cmp::PartialEq for ModelHandle<T> {
    fn eq(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (None, None) => true,
            (None, Some(_)) => false,
            (Some(_), None) => false,
            (Some(a), Some(b)) => core::ptr::eq(
                (&**a) as *const dyn Model<Data = T> as *const u8,
                (&**b) as *const dyn Model<Data = T> as *const u8,
            ),
        }
    }
}

impl<T> From<Rc<dyn Model<Data = T>>> for ModelHandle<T> {
    fn from(x: Rc<dyn Model<Data = T>>) -> Self {
        Self(Some(x))
    }
}

impl<T> ModelHandle<T> {
    /// Create a new handle wrapping the given model
    pub fn new(model: Rc<dyn Model<Data = T>>) -> Self {
        Self(Some(model))
    }
}

impl<T> Model for ModelHandle<T> {
    type Data = T;

    fn row_count(&self) -> usize {
        self.0.as_ref().map_or(0, |model| model.row_count())
    }

    fn row_data(&self, row: usize) -> Self::Data {
        self.0.as_ref().unwrap().row_data(row)
    }

    fn set_row_data(&self, row: usize, data: Self::Data) {
        self.0.as_ref().unwrap().set_row_data(row, data)
    }

    fn attach_peer(&self, peer: ModelPeer) {
        // Forward, in case the model doesn't provide `model_tracker` yet.
        if let Some(model) = self.0.as_ref() {
            #[allow(deprecated)]
            model.attach_peer(peer);
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
pub trait RepeatedComponent: crate::component::Component {
    /// The data corresponding to the model
    type Data: 'static;

    /// Update this component at the given index and the given data
    fn update(&self, index: usize, data: Self::Data);

    /// Layout this item in the listview
    ///
    /// offset_y is the `y` position where this item should be placed.
    /// it should be updated to be to the y position of the next item.
    fn listview_layout(
        self: Pin<&Self>,
        _offset_y: &mut f32,
        _viewport_width: Pin<&Property<f32>>,
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
    /// The model row (index) of the first component in the `components` vector.
    /// Only used for ListView
    offset: usize,
    /// The average visible item_height. Only used for ListView
    cached_item_height: f32,
}

impl<C: RepeatedComponent> Default for RepeaterInner<C> {
    fn default() -> Self {
        RepeaterInner { components: Default::default(), offset: 0, cached_item_height: 0. }
    }
}
trait ErasedRepeater {
    fn row_changed(&self, row: usize);
    fn row_added(&self, index: usize, count: usize);
    fn row_removed(&self, index: usize, count: usize);
}

impl<C: RepeatedComponent> ErasedRepeater for Repeater<C> {
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
}

/// This field is put in a component when using the `for` syntax
/// It helps instantiating the components `C`
#[pin_project(PinnedDrop)]
pub struct Repeater<C: RepeatedComponent> {
    inner: RefCell<RepeaterInner<C>>,
    #[pin]
    model: Property<ModelHandle<C::Data>>,
    #[pin]
    is_dirty: Property<bool>,
    /// Only used for the list view to track if the scrollbar has changed and item needs to be layed out again.
    #[pin]
    listview_geometry_tracker: crate::properties::PropertyTracker,

    /// Will be initialized when the ModelPeer is initialized.
    /// The DependencyNode points to self
    peer: OnceCell<Pin<Rc<DependencyNode<*const dyn ErasedRepeater>>>>,
}

impl<C: RepeatedComponent> Default for Repeater<C> {
    fn default() -> Self {
        Repeater {
            inner: Default::default(),
            model: Default::default(),
            is_dirty: Default::default(),
            listview_geometry_tracker: Default::default(),
            peer: Default::default(),
        }
    }
}

#[pin_project::pinned_drop]
impl<C: RepeatedComponent> PinnedDrop for Repeater<C> {
    fn drop(self: Pin<&mut Self>) {
        if let Some(peer) = self.peer.get() {
            peer.remove();
            // We should be the only one still holding a ref count to it, so that
            // it cannot be re-added in any list, and the pointer to self will not
            // be accessed anymore
            debug_assert_eq!(PinWeak::downgrade(peer.clone()).strong_count(), 1);
        }
    }
}

impl<C: RepeatedComponent + 'static> Repeater<C> {
    fn model(self: Pin<&Self>) -> ModelHandle<C::Data> {
        // Safety: Repeater does not implement drop and never let access model as mutable
        let model = self.project_ref().model;

        if model.is_dirty() {
            *self.inner.borrow_mut() = RepeaterInner::default();
            self.is_dirty.set(true);
            if let ModelHandle(Some(m)) = model.get() {
                let peer = self.peer.get_or_init(|| {
                    //Safety: we will reset it when we Drop the Repeater
                    Rc::pin(DependencyNode::new(
                        self.get_ref() as &dyn ErasedRepeater as *const dyn ErasedRepeater
                    ))
                });

                #[allow(deprecated)]
                m.attach_peer(ModelPeer { inner: PinWeak::downgrade(peer.clone()) });
            }
        }
        model.get()
    }

    /// Call this function to make sure that the model is updated.
    /// The init function is the function to create a component
    pub fn ensure_updated(self: Pin<&Self>, init: impl Fn() -> ComponentRc<C>) {
        if let ModelHandle(Some(model)) = self.model() {
            if self.project_ref().is_dirty.get() {
                self.ensure_updated_impl(init, &model, model.row_count());
            }
        } else {
            self.inner.borrow_mut().components.clear();
        }
    }

    // returns true if new items were created
    fn ensure_updated_impl(
        self: Pin<&Self>,
        init: impl Fn() -> ComponentRc<C>,
        model: &Rc<dyn Model<Data = C::Data>>,
        count: usize,
    ) -> bool {
        let mut inner = self.inner.borrow_mut();
        inner.components.resize_with(count, || (RepeatedComponentState::Dirty, None));
        let offset = inner.offset;
        let mut created = false;
        for (i, c) in inner.components.iter_mut().enumerate() {
            if c.0 == RepeatedComponentState::Dirty {
                if c.1.is_none() {
                    created = true;
                    c.1 = Some(init());
                }
                c.1.as_ref().unwrap().update(i + offset, model.row_data(i + offset));
                c.0 = RepeatedComponentState::Clean;
            }
        }
        self.is_dirty.set(false);
        created
    }

    /// Same as `Self::ensuer_updated` but for a ListView
    pub fn ensure_updated_listview(
        self: Pin<&Self>,
        init: impl Fn() -> ComponentRc<C>,
        viewport_width: Pin<&Property<f32>>,
        viewport_height: Pin<&Property<f32>>,
        viewport_y: Pin<&Property<f32>>,
        listview_width: f32,
        listview_height: Pin<&Property<f32>>,
    ) {
        let empty_model = || {
            self.inner.borrow_mut().components.clear();
            viewport_height.set(0.);
            viewport_y.set(0.);
        };

        let model = if let ModelHandle(Some(model)) = self.model() {
            model
        } else {
            return empty_model();
        };
        let row_count = model.row_count();
        if row_count == 0 {
            return empty_model();
        }

        let init = &init;

        let listview_geometry_tracker = self.project_ref().listview_geometry_tracker;
        let geometry_updated = listview_geometry_tracker
            .evaluate_if_dirty(|| {
                // Fetch the model again to make sure that it is a dependency of this geometry tracker.
                // invariant: model existence was checked earlier, so unwrap() should be safe.
                let model = self.model().0.unwrap();
                // Also register a dependency to "is_dirty"
                let _ = self.project_ref().is_dirty.get();

                let listview_height = listview_height.get();
                // Compute the element height
                let total_height = Cell::new(0.);
                let min_height = Cell::new(listview_height);
                let count = Cell::new(0);

                let get_height_visitor = |item: Pin<ItemRef>| {
                    count.set(count.get() + 1);
                    let height = item.as_ref().geometry().height();
                    total_height.set(total_height.get() + height);
                    min_height.set(min_height.get().min(height))
                };
                for c in self.inner.borrow().components.iter() {
                    if let Some(x) = c.1.as_ref() {
                        get_height_visitor(x.as_pin_ref().get_item_ref(0));
                    }
                }

                let mut element_height = if count.get() > 0 {
                    total_height.get() / (count.get() as f32)
                } else {
                    // There seems to be currently no items. Just instantiate one item.

                    {
                        let mut inner = self.inner.borrow_mut();
                        inner.offset = inner.offset.min(row_count - 1);
                    }

                    self.ensure_updated_impl(&init, &model, 1);
                    if let Some(c) = self.inner.borrow().components.get(0) {
                        if let Some(x) = c.1.as_ref() {
                            get_height_visitor(x.as_pin_ref().get_item_ref(0));
                        }
                    } else {
                        panic!("Could not determine size of items");
                    }
                    total_height.get()
                };

                let min_height = min_height.get().min(element_height).max(1.);

                let mut offset_y = -viewport_y.get().min(0.);
                if offset_y > element_height * row_count as f32 - listview_height
                    && offset_y > viewport_height.get() - listview_height
                {
                    offset_y = (element_height * row_count as f32 - listview_height).max(0.);
                }
                let mut count = ((listview_height / min_height).ceil() as usize)
                    // count never decreases to avoid too much flickering if items have different size
                    .max(self.inner.borrow().components.len())
                    .min(row_count);
                let mut offset =
                    ((offset_y / element_height).floor() as usize).min(row_count - count);
                self.inner.borrow_mut().cached_item_height = element_height;
                loop {
                    self.inner.borrow_mut().cached_item_height = element_height;
                    self.set_offset(offset, count);
                    self.ensure_updated_impl(init, &model, count);
                    let end = self.compute_layout_listview(viewport_width, listview_width);
                    let adjusted_element_height =
                        (end - element_height * offset as f32) / count as f32;
                    element_height = adjusted_element_height;
                    let diff = listview_height + offset_y - end;
                    if diff > 0.5 && count < row_count {
                        // we did not create enough item, try increasing count until it matches
                        count = (count + (diff / element_height).ceil() as usize).min(row_count);
                        if offset + count > row_count {
                            // apparently, we scrolled past the end, so decrease the offset and make offset_y
                            // so we just are at the end
                            offset = row_count - count;
                            offset_y = (offset_y - diff).max(0.);
                        }
                        continue;
                    }
                    viewport_height.set((element_height * row_count as f32).max(end));
                    viewport_y.set(-offset_y);
                    break;
                }
            })
            .is_some();

        if !geometry_updated && self.project_ref().is_dirty.get() {
            let count = self
                .inner
                .borrow()
                .components
                .len()
                .min(row_count.saturating_sub(self.inner.borrow().offset));
            self.ensure_updated_impl(init, &model, count);
            self.compute_layout_listview(viewport_width, listview_width);
        }
    }

    fn set_offset(&self, offset: usize, count: usize) {
        let mut inner = self.inner.borrow_mut();
        let old_offset = inner.offset;
        // Remove the items before the offset, or add items until the old offset
        let to_remove = offset.saturating_sub(old_offset);
        if to_remove < inner.components.len() {
            inner.components.splice(
                0..to_remove,
                core::iter::repeat((RepeatedComponentState::Dirty, None))
                    .take(old_offset.saturating_sub(offset)),
            );
        } else {
            inner.components.truncate(0);
        }
        inner.components.resize_with(count, || (RepeatedComponentState::Dirty, None));
        inner.offset = offset;
        self.is_dirty.set(true);
    }

    /// Sets the data directly in the model
    pub fn model_set_row_data(self: Pin<&Self>, row: usize, data: C::Data) {
        if let ModelHandle(Some(model)) = self.model() {
            model.set_row_data(row, data);
            if let Some(c) = self.inner.borrow_mut().components.get_mut(row) {
                if c.0 == RepeatedComponentState::Dirty {
                    if let Some(comp) = c.1.as_ref() {
                        comp.update(row, model.row_data(row));
                        c.0 = RepeatedComponentState::Clean;
                    }
                }
            }
        }
    }
}

impl<C: RepeatedComponent> Repeater<C> {
    /// Set the model binding
    pub fn set_model_binding(&self, binding: impl Fn() -> ModelHandle<C::Data> + 'static) {
        self.model.set_binding(binding);
    }

    /// Call the visitor for each component
    pub fn visit(
        &self,
        order: TraversalOrder,
        mut visitor: crate::item_tree::ItemVisitorRefMut,
    ) -> crate::item_tree::VisitChildrenResult {
        // We can't keep self.inner borrowed because the event might modify the model
        let count = self.inner.borrow().components.len();
        for i in 0..count {
            let i = if order == TraversalOrder::BackToFront { i } else { count - i - 1 };
            let c = self.inner.borrow().components.get(i).and_then(|c| c.1.clone());
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
        self.inner.borrow().components.len()
    }

    /// Return true if the Repeater is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns a vector containing all components
    pub fn components_vec(&self) -> Vec<ComponentRc<C>> {
        self.inner.borrow().components.iter().flat_map(|x| x.1.clone()).collect()
    }

    /// Set the position of all the element in the listview
    ///
    /// Returns the offset of the end of the last element
    pub fn compute_layout_listview(
        &self,
        viewport_width: Pin<&Property<f32>>,
        listview_width: f32,
    ) -> f32 {
        let inner = self.inner.borrow();
        let mut y_offset = inner.offset as f32 * inner.cached_item_height;
        viewport_width.set(listview_width);
        for c in self.inner.borrow().components.iter() {
            if let Some(x) = c.1.as_ref() {
                x.as_pin_ref().listview_layout(&mut y_offset, viewport_width);
            }
        }
        y_offset
    }
}

/// Represent an item in a StandardListView
#[repr(C)]
#[derive(Clone, Default, Debug, PartialEq)]
pub struct StandardListViewItem {
    /// The text content of the item
    pub text: crate::SharedString,
}

#[test]
fn test_tracking_model_handle() {
    let model: Rc<VecModel<u8>> = Rc::new(Default::default());
    let handle = ModelHandle::new(model.clone());
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
}
