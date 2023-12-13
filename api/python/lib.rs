// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

mod interpreter;
use interpreter::{ComponentCompiler, PyDiagnostic, PyDiagnosticLevel, PyValueType};
mod errors;

use pyo3::prelude::*;

#[pymodule]
fn slint(_py: Python<'_>, m: &PyModule) -> PyResult<()> {
    m.add_class::<ComponentCompiler>()?;
    m.add_class::<PyValueType>()?;
    m.add_class::<PyDiagnosticLevel>()?;
    m.add_class::<PyDiagnostic>()?;

    Ok(())
}
