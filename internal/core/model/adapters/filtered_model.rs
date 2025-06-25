// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Provides the `FilteredModel` adapter that wraps another model and filters items based on a predicate.
//!
//! This module implements a reactive filtering mechanism that automatically updates when the source model changes.
//! The filter predicate is applied lazily and results are cached for performance.

use i_slint_core::model::{Model, ModelNotify, ModelRc, ModelTracker, ModelEvent, ModelPeer};
use core::cell::RefCell;
use std::rc::{Rc, Weak};

/// A model that wraps another model and filters its items based on a predicate
pub struct FilteredModel<SourceModel, F>
where
    SourceModel: Model,
    F: Fn(&SourceModel::Data) -> bool + 'static,
{
    source: ModelRc<SourceModel>,
    filter: Rc<F>,
    notify: ModelNotify,
    /// Cached indices of the source model that pass the filter
    filtered_indices: RefCell<Vec<usize>>,
}

unsafe impl<SourceModel, F> !Send for FilteredModel<SourceModel, F> {}
unsafe impl<SourceModel, F> !Sync for FilteredModel<SourceModel, F> {}

impl<SourceModel, F> FilteredModel<SourceModel, F>
where
    SourceModel: Model,
    F: Fn(&SourceModel::Data) -> bool + 'static,
{
    /// Rebuilds the entire list of filtered indices by iterating through all source model rows
    fn regenerate_filtered_indices(&self) {
        let mut filtered_indices = Vec::new();
        for i in 0..self.source.row_count() {
            if let Some(data) = self.source.row_data(i) {
                if (self.filter)(&data) {
                    filtered_indices.push(i);
                }
            }
        }
        *self.filtered_indices.borrow_mut() = filtered_indices;
    }

    /// Updates filtered indices when source model row count changes (currently unused)
    #[allow(dead_code)]
    fn handle_row_count_change(&self) {
        let new_count = self.source.row_count();
        let mut indices = self.filtered_indices.borrow_mut();

        // Remove indices that are now out of bounds due to reduced row count
        indices.retain(|&i| i < new_count);

        // Check only newly added rows to avoid full regeneration
        for i in indices.len()..new_count {
            if let Some(data) = self.source.row_data(i) {
                if (self.filter)(&data) {
                    indices.push(i);
                }
            }
        }
    }

    /// Creates a new FilteredModel that wraps the given source model and applies the filter predicate
    pub fn new(source: ModelRc<SourceModel>, filter: F) -> Rc<Self> {
        let mut filtered_indices = Vec::new();
        for i in 0..source.row_count() {
            if let Some(data) = source.row_data(i) {
                if (filter)(&data) {
                    filtered_indices.push(i);
                }
            }
        }

        let result = Rc::new(Self {
            source: source.clone(),
            filter: Rc::new(filter),
            notify: ModelNotify::default(),
            filtered_indices: RefCell::new(filtered_indices),
        });

        // Set up source model change tracking
        let notify = result.notify.clone();
        let weak_self = Rc::downgrade(&result);
        let filter = result.filter.clone();
        source.model_tracker().attach_peer(ModelPeer::new(move |event| {
            if let Some(this) = weak_self.upgrade() {
            match event {
                ModelEvent::RowCountChanged => {
                    this.regenerate_filtered_indices();
                    notify.row_count_changed();
                }
                ModelEvent::RowDataChanged(row) => {
                    // Define action type enumeration
                    /// Action types for handling row data changes
                    enum Action { Update(usize), Remove(usize), Add(usize), None }
                    // Phase 1: Read-only operations to determine required action
                    let action = {
                        let filtered_indices = this.filtered_indices.borrow();
                        match filtered_indices.binary_search(&row) {
                            Ok(pos) => {
                                // Check if row still passes filter
                                let passes_filter = match source.row_data(row) {
                                    Some(data) => match std::panic::catch_unwind(|| (filter)(&data)) {
                                        Ok(result) => result,
                                        Err(_) => false, // Catch panics, treat as filter failure
                                    },
                                    None => false
                                };
                                if passes_filter {
                                    Action::Update(pos)
                                } else {
                                    Action::Remove(pos)
                                }
                            }
                            Err(_) => {
                                // Check if row now passes filter
                                match source.row_data(row) {
                                    Some(data) => match std::panic::catch_unwind(|| (filter)(&data)) {
                                        Ok(true) => Action::Add(row),
                                        _ => Action::None // Treat panic or filter failure as not passing
                                    },
                                    None => Action::None
                                }
                            }
                        }
                    };

                    // Phase 2: Perform actions (read lock already released)
                    match action {
                        Action::Update(pos) => notify.row_data_changed(pos),
                        Action::Remove(pos) => {
                            let current_count = this.row_count();
                            this.filtered_indices.borrow_mut().remove(pos);
                            notify.row_removed(pos);
                            #[cfg(debug_assertions)]
                            if this.row_count() != current_count - 1 {
                                use std::sync::Once;
                                static mut LAST_LOG: Option<std::time::Instant> = None;
                                static mut ONCE: Once = Once::new();

                                ONCE.call_once(|| {
                                    LAST_LOG = Some(std::time::Instant::now() - std::time::Duration::from_secs(61));
                                });

                                let now = std::time::Instant::now();
                                let can_log = unsafe {
                                    match LAST_LOG {
                                        None => true,
                                        Some(last) => now.duration_since(last) > std::time::Duration::from_secs(60),
                                    }
                                };

                                if can_log {
                                    log::error!("Index inconsistency detected after removal: expected {} rows but got {}", current_count - 1, this.row_count());
                                    unsafe { LAST_LOG = Some(now); }
                                }
                            },
                        Action::Add(row) => {
                            let mut indices = this.filtered_indices.borrow_mut();
                            let pos = indices.partition_point(|&i| i < row);
                            indices.insert(pos, row);
                            notify.row_added(pos);
                        },
                        Action::None => {}
                    }
                }
                ModelEvent::Reset => {
                    this.regenerate_filtered_indices();
                    notify.reset();
                }
                _ => {}
            }
        }));

        result
    }
}

impl<SourceModel, F> Model for FilteredModel<SourceModel, F>
where
    SourceModel: Model,
    F: Fn(&SourceModel::Data) -> bool + 'static,
{
    type Data = SourceModel::Data;

    fn row_count(&self) -> usize {
        self.filtered_indices.borrow().len()
    }

    fn row_data(&self, row: usize) -> Option<Self::Data> {
        let source_row = self.filtered_indices.borrow().get(row).copied()?;
        self.source.row_data(source_row)
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        &self.notify
    }

    fn set_row_data(&self, row: usize, data: Self::Data) {
        if let Some(source_row) = self.filtered_indices.borrow().get(row).copied() {
            self.source.set_row_data(source_row, data);
        }
    }
}

impl<SourceModel, F> Drop for FilteredModel<SourceModel, F>
where
    SourceModel: Model,
    F: Fn(&SourceModel::Data) -> bool + 'static,
{
    fn drop(&mut self) {
        /// Custom destructor with debug logging
        #[cfg(debug_assertions)]
        log::debug!("FilteredModel dropped");
    }
}


impl<SourceModel, F> ModelTracker for FilteredModel<SourceModel, F>
where
    SourceModel: Model,
    F: Fn(&SourceModel::Data) -> bool + 'static,
{
    fn attach_peer(&self, peer: ModelPeer) {
        self.notify.attach_peer(peer);
    }

    fn track_row_count_changes(&self) {
        self.notify.track_row_count_changes();
    }

    fn track_row_data_changes(&self, row: usize) {
        self.notify.track_row_data_changes(row);
    }
}