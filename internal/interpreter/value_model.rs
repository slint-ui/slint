// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::api::Value;
use core::cell::Cell;
use i_slint_core::model::{Model, ModelNotify, ModelTracker};

pub struct ValueModel {
    value: Value,
}

impl ValueModel {
    pub fn new(value: Value) -> Self {
        Self { value }
    }
}

impl ModelTracker for ValueModel {
    fn attach_peer(&self, peer: i_slint_core::model::ModelPeer) {
        if let Value::Model(ref model_ptr) = self.value {
            model_ptr.model_tracker().attach_peer(peer)
        }
    }

    fn track_row_count_changes(&self) {
        if let Value::Model(ref model_ptr) = self.value {
            model_ptr.model_tracker().track_row_count_changes()
        }
    }

    fn track_row_data_changes(&self, row: usize) {
        if let Value::Model(ref model_ptr) = self.value {
            model_ptr.model_tracker().track_row_data_changes(row)
        }
    }
}

impl Model for ValueModel {
    type Data = Value;

    fn row_count(&self) -> usize {
        match &self.value {
            Value::Bool(b) => {
                if *b {
                    1
                } else {
                    0
                }
            }
            Value::Number(x) => x.max(Default::default()) as usize,
            Value::Void => 0,
            Value::Model(model_ptr) => model_ptr.row_count(),
            x => panic!("Invalid model {x:?}"),
        }
    }

    fn row_data(&self, row: usize) -> Option<Self::Data> {
        if row >= self.row_count() {
            None
        } else {
            Some(match &self.value {
                Value::Bool(_) => Value::Void,
                Value::Number(_) => Value::Number(row as _),
                Value::Model(model_ptr) => model_ptr.row_data(row)?,
                x => panic!("Invalid model {x:?}"),
            })
        }
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        self
    }

    fn set_row_data(&self, row: usize, data: Self::Data) {
        match &self.value {
            Value::Model(model_ptr) => model_ptr.set_row_data(row, data),
            _ => eprintln!("Trying to change the value of a read-only integer model."),
        }
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}

/// A model for conditional elements
#[derive(Default)]
pub(crate) struct BoolModel {
    value: Cell<bool>,
    notify: ModelNotify,
}

impl Model for BoolModel {
    type Data = Value;
    fn row_count(&self) -> usize {
        if self.value.get() {
            1
        } else {
            0
        }
    }
    fn row_data(&self, row: usize) -> Option<Self::Data> {
        (row == 0 && self.value.get()).then_some(Value::Void)
    }
    fn model_tracker(&self) -> &dyn ModelTracker {
        &self.notify
    }
}

impl BoolModel {
    pub fn set_value(&self, val: bool) {
        let old = self.value.replace(val);
        if old != val {
            self.notify.reset();
        }
    }
}
