// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

use pyo3::prelude::*;

#[pymodule]
fn slint(_py: Python<'_>, m: &PyModule) -> PyResult<()> {
    Ok(())
}
