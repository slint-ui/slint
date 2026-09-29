// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! This module contains the implementation of the model change tracking.

// Safety: we use pointer to ModelChangeListenerContainer in the DependencyList,
// but the Drop of the ModelChangeListenerContainer will remove them from the list
// so it will not be accessed after it is dropped
#![allow(unsafe_code)]

use super::*;
use crate::properties::dependency_tracker::DependencyNode;

type DependencyListHead =
    crate::properties::dependency_tracker::DependencyListHead<*const dyn ModelChangeListener>;

/// Represent a handle to a view that listens to changes to a model.
///
/// One should normally not use this class directly, it is just
/// used internally by via [`ModelTracker::attach_peer`] and [`ModelNotify`]
#[derive(Clone)]
pub struct ModelPeer<'a> {
    inner: Pin<&'a DependencyNode<*const dyn ModelChangeListener>>,
}

/// Which rows [`ModelTracker::track_row_data_changes`] and [`ModelTracker::track_any_change`]
/// have registered a dependency on.
enum TrackedRows {
    /// Sorted list of individually tracked rows.
    Rows(Vec<usize>),
    /// track_any_change() was called, making every row implicitly tracked.
    All,
}

impl Default for TrackedRows {
    fn default() -> Self {
        TrackedRows::Rows(Vec::new())
    }
}

impl TrackedRows {
    fn is_tracked(&self, row: usize) -> bool {
        match self {
            TrackedRows::Rows(rows) => rows.binary_search(&row).is_ok(),
            TrackedRows::All => true,
        }
    }
}

#[pin_project]
#[derive(Default)]
struct ModelNotifyInner {
    #[pin]
    model_row_count_dirty_property: Property<()>,
    #[pin]
    model_row_data_dirty_property: Property<()>,
    #[pin]
    peers: DependencyListHead,
    tracked_rows: RefCell<TrackedRows>,
}

/// Dispatch notifications from a [`Model`] to one or several [`ModelPeer`].
/// Typically, you would want to put this in the implementation of the Model
#[derive(Default)]
pub struct ModelNotify {
    inner: Pin<Box<ModelNotifyInner>>,
}

impl ModelNotify {
    fn inner(&self) -> Pin<&ModelNotifyInner> {
        self.inner.as_ref()
    }

    /// Notify the peers that a specific row was changed
    pub fn row_changed(&self, row: usize) {
        let inner = &self.inner;
        if inner.tracked_rows.borrow().is_tracked(row) {
            inner.model_row_data_dirty_property.mark_dirty();
        }
        inner.as_ref().project_ref().peers.for_each(|p| {
            // Safety: The peers contain a list of pinned ModelChangedListener
            unsafe { Pin::new_unchecked(&**p) }.row_changed(row)
        })
    }
    /// Notify the peers that rows were added
    pub fn row_added(&self, index: usize, count: usize) {
        let inner = &self.inner;
        inner.model_row_count_dirty_property.mark_dirty();
        *inner.tracked_rows.borrow_mut() = TrackedRows::default();
        inner.model_row_data_dirty_property.mark_dirty();
        inner.as_ref().project_ref().peers.for_each(|p| {
            // Safety: The peers contain a list of pinned ModelChangedListener
            unsafe { Pin::new_unchecked(&**p) }.row_added(index, count)
        })
    }
    /// Notify the peers that rows were removed
    pub fn row_removed(&self, index: usize, count: usize) {
        let inner = &self.inner;
        inner.model_row_count_dirty_property.mark_dirty();
        *inner.tracked_rows.borrow_mut() = TrackedRows::default();
        inner.model_row_data_dirty_property.mark_dirty();
        inner.as_ref().project_ref().peers.for_each(|p| {
            // Safety: The peers contain a list of pinned ModelChangedListener
            unsafe { Pin::new_unchecked(&**p) }.row_removed(index, count)
        })
    }

    /// Notify the peer that the model has been changed in some way and
    /// everything needs to be reloaded
    pub fn reset(&self) {
        let inner = &self.inner;
        inner.model_row_count_dirty_property.mark_dirty();
        *inner.tracked_rows.borrow_mut() = TrackedRows::default();
        inner.model_row_data_dirty_property.mark_dirty();
        inner.as_ref().project_ref().peers.for_each(|p| {
            // Safety: The peers contain a list of pinned ModelChangedListener
            unsafe { Pin::new_unchecked(&**p) }.reset()
        })
    }
}

impl ModelTracker for ModelNotify {
    /// Attach one peer. The peer will be notified when the model changes
    fn attach_peer(&self, peer: ModelPeer) {
        self.inner().project_ref().peers.append(peer.inner)
    }

    fn track_row_count_changes(&self) {
        self.inner().project_ref().model_row_count_dirty_property.get();
    }

    fn track_row_data_changes(&self, row: usize) {
        if crate::properties::is_currently_tracking() {
            let inner = self.inner().project_ref();

            // Recording the row individually is redundant once every row is tracked.
            if let TrackedRows::Rows(rows) = &mut *inner.tracked_rows.borrow_mut()
                && let Err(insertion_point) = rows.binary_search(&row)
            {
                rows.insert(insertion_point, row);
            }

            inner.model_row_data_dirty_property.get();
        }
    }

    fn track_any_change(&self, _row_count: usize, _: crate::InternalToken) {
        self.track_row_count_changes();
        if crate::properties::is_currently_tracking() {
            let inner = self.inner().project_ref();
            // Any individually tracked rows are now subsumed by the whole-model dependency.
            *inner.tracked_rows.borrow_mut() = TrackedRows::All;
            inner.model_row_data_dirty_property.get();
        }
    }
}

pub trait ModelChangeListener {
    fn row_changed(self: Pin<&Self>, row: usize);
    fn row_added(self: Pin<&Self>, index: usize, count: usize);
    fn row_removed(self: Pin<&Self>, index: usize, count: usize);
    fn reset(self: Pin<&Self>);
}

#[pin_project(PinnedDrop)]
#[derive(Default, derive_more::Deref)]
/// This is a structure that contains a T which implements [`ModelChangeListener`]
/// and can provide a [`ModelPeer`] for it when pinned.
pub struct ModelChangeListenerContainer<T: ModelChangeListener> {
    /// Will be initialized when the ModelPeer is initialized.
    /// The DependencyNode points to data.
    peer: OnceCell<DependencyNode<*const dyn ModelChangeListener>>,

    #[pin]
    #[deref]
    data: T,
}

#[pin_project::pinned_drop]
impl<T: ModelChangeListener> PinnedDrop for ModelChangeListenerContainer<T> {
    fn drop(self: Pin<&mut Self>) {
        if let Some(peer) = self.peer.get() {
            peer.remove();
        }
    }
}

impl<T: ModelChangeListener + 'static> ModelChangeListenerContainer<T> {
    pub fn new(data: T) -> Self {
        Self { peer: Default::default(), data }
    }

    pub fn model_peer(self: Pin<&Self>) -> ModelPeer<'_> {
        let peer = self.get_ref().peer.get_or_init(|| {
            //Safety: self.data and self.peer have the same lifetime, so the pointer stays valid
            DependencyNode::new(
                (&self.data) as &dyn ModelChangeListener as *const dyn ModelChangeListener,
            )
        });

        // Safety: `peer` is pinned because `self` is pinned and it is a projection, but pin_project don't go through the OnceCell
        let peer = unsafe { Pin::new_unchecked(peer) };

        ModelPeer { inner: peer }
    }

    pub fn get(self: Pin<&Self>) -> Pin<&T> {
        self.project_ref().data
    }
}

/// A pinned `ModelChangeListenerContainer` using `NonNull` instead of `Box`
/// to avoid aliasing issues when the struct is moved into `Rc::new()`.
pub struct ModelChangeListenerBox<T: ModelChangeListener + 'static> {
    ptr: core::ptr::NonNull<ModelChangeListenerContainer<T>>,
}

impl<T: ModelChangeListener + 'static> ModelChangeListenerBox<T> {
    pub fn new(data: T) -> Self {
        let container = ModelChangeListenerContainer::new(data);
        // Safety: Box::into_raw returns a non-null pointer
        let ptr = unsafe { core::ptr::NonNull::new_unchecked(Box::into_raw(Box::new(container))) };
        Self { ptr }
    }

    pub fn as_ref(&self) -> Pin<&ModelChangeListenerContainer<T>> {
        // Safety: the data is pinned because we never move it or expose &mut to it
        unsafe { Pin::new_unchecked(self.ptr.as_ref()) }
    }
}

impl<T: ModelChangeListener + 'static> core::ops::Deref for ModelChangeListenerBox<T> {
    type Target = T;
    fn deref(&self) -> &T {
        // Safety: ptr is valid for the lifetime of self
        unsafe { &self.ptr.as_ref().data }
    }
}

impl<T: ModelChangeListener + 'static> Drop for ModelChangeListenerBox<T> {
    fn drop(&mut self) {
        // Safety: we own the allocation and it was created by Box::new.
        // Box::from_raw runs PinnedDrop which calls peer.remove().
        unsafe { drop(Box::from_raw(self.ptr.as_ptr())) }
    }
}
