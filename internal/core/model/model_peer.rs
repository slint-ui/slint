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

#[pin_project]
#[derive(Default)]
struct ModelNotifyInner {
    #[pin]
    model_row_count_dirty_property: Property<()>,
    #[pin]
    model_row_data_dirty_property: Property<()>,
    #[pin]
    peers: DependencyListHead,
    // Sorted list of rows that track_row_data_changes() was called for
    tracked_rows: RefCell<Vec<usize>>,
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
            if inner.tracked_rows.borrow().binary_search(&row).is_ok() {
                inner.model_row_data_dirty_property.mark_dirty();
            }
            inner.as_ref().project_ref().peers.for_each(|p| {
                // Safety: The peers contain a list of pinned ModelChangedListener
                unsafe { Pin::new_unchecked(&**p) }.row_changed(row)
            })
        }
    }
    /// Notify the peers that rows were added
    pub fn row_added(&self, index: usize, count: usize) {
        if let Some(inner) = self.inner.get() {
            inner.model_row_count_dirty_property.mark_dirty();
            inner.tracked_rows.borrow_mut().clear();
            inner.model_row_data_dirty_property.mark_dirty();
            inner.as_ref().project_ref().peers.for_each(|p| {
                // Safety: The peers contain a list of pinned ModelChangedListener
                unsafe { Pin::new_unchecked(&**p) }.row_added(index, count)
            })
        }
    }
    /// Notify the peers that rows were removed
    pub fn row_removed(&self, index: usize, count: usize) {
        if let Some(inner) = self.inner.get() {
            inner.model_row_count_dirty_property.mark_dirty();
            inner.tracked_rows.borrow_mut().clear();
            inner.model_row_data_dirty_property.mark_dirty();
            inner.as_ref().project_ref().peers.for_each(|p| {
                // Safety: The peers contain a list of pinned ModelChangedListener
                unsafe { Pin::new_unchecked(&**p) }.row_removed(index, count)
            })
        }
    }

    /// Notify the peer that the model has been changed in some way and
    /// everything needs to be reloaded
    pub fn reset(&self) {
        if let Some(inner) = self.inner.get() {
            inner.model_row_count_dirty_property.mark_dirty();
            inner.tracked_rows.borrow_mut().clear();
            inner.model_row_data_dirty_property.mark_dirty();
            inner.as_ref().project_ref().peers.for_each(|p| {
                // Safety: The peers contain a list of pinned ModelChangedListener
                unsafe { Pin::new_unchecked(&**p) }.reset()
            })
        }
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

            let mut tracked_rows = inner.tracked_rows.borrow_mut();
            if let Err(insertion_point) = tracked_rows.binary_search(&row) {
                tracked_rows.insert(insertion_point, row);
            }

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
