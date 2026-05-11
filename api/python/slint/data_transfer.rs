// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use pyo3::prelude::*;
use pyo3_stub_gen::{derive::gen_stub_pyclass, derive::gen_stub_pymethods};

/// Python representation of some form of type-indexed possibly-lazy data transfer.
/// Used for accessing the platform clipboard and drag-and-drop APIs.
#[gen_stub_pyclass]
#[pyclass(unsendable, name = "DataTransfer", skip_from_py_object)]
#[derive(Clone)]
pub struct PyDataTransfer {
    pub data_transfer: i_slint_core::data_transfer::DataTransfer,
}

#[gen_stub_pymethods]
#[pymethods]
impl PyDataTransfer {
    fn __repr__(&self) -> String {
        format!("DataTransfer({:?})", self.data_transfer)
    }

    fn __eq__(&self, other: &Self) -> bool {
        self.data_transfer == other.data_transfer
    }
}

impl From<i_slint_core::data_transfer::DataTransfer> for PyDataTransfer {
    fn from(data_transfer: i_slint_core::data_transfer::DataTransfer) -> Self {
        Self { data_transfer }
    }
}
