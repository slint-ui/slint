// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::api::Value;
use i_slint_core::model::{Model, ModelRc, ModelTracker};

/// A number used as a model (`for i in 42`): `n` rows whose data is the row
/// index. The type-erased equivalent of core's `impl Model for usize`; the
/// count is baked in — a change to the number re-evaluates the model binding
/// and produces a new `IntModel`, so the tracker has nothing to track.
pub struct IntModel(pub usize);

impl Model for IntModel {
    type Data = Value;

    fn row_count(&self) -> usize {
        self.0
    }

    fn row_data(&self, row: usize) -> Option<Self::Data> {
        (row < self.0).then(|| Value::Number(row as f64))
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        &()
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}

// A map model that wraps a Model
pub struct ValueMapModel<T>(pub ModelRc<T>);

impl<T: TryFrom<Value> + Into<Value> + 'static> Model for ValueMapModel<T> {
    type Data = Value;

    fn row_count(&self) -> usize {
        self.0.row_count()
    }

    fn row_data(&self, row: usize) -> Option<Self::Data> {
        self.0.row_data(row).map(|x| x.into())
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        self.0.model_tracker()
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }

    fn set_row_data(&self, row: usize, data: Self::Data) {
        if let Ok(data) = data.try_into() {
            self.0.set_row_data(row, data)
        }
    }

    fn push_row(&self, data: Self::Data) {
        if let Ok(data) = data.try_into() {
            self.0.push_row(data)
        }
    }

    fn remove_row(&self, row: isize) {
        if row >= 0 && row < self.0.row_count() as isize {
            self.0.remove_row(row);
        }
    }

    fn insert_row(&self, row: isize, data: Self::Data) {
        if row < 0 || row > self.0.row_count() as isize {
            return;
        }
        if let Ok(data) = data.try_into() {
            self.0.insert_row(row, data);
        }
    }
}
