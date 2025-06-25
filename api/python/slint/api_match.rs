// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::path::PathBuf;

use pyo3::prelude::*;
use pyo3_stub_gen::{derive::gen_stub_pyclass, derive::gen_stub_pymethods};

#[gen_stub_pyclass]
#[pyclass(name = "GeneratedAPI", unsendable)]
pub struct PyGeneratedAPI {
    pub(crate) path: PathBuf,
    pub(crate) module: i_slint_compiler::generator::python::PyModule,
}

#[gen_stub_pymethods]
#[pymethods]
impl PyGeneratedAPI {
    #[new]
    fn new(path: PathBuf, json: &str) -> PyResult<Self> {
        let module = i_slint_compiler::generator::python::PyModule::load_from_json(json)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
        Ok(Self { path, module })
    }

    #[staticmethod]
    fn compare_generated_vs_actual(generated: &Self, actual: &Self) -> PyResult<()> {
        let changed_globals = generated.module.changed_globals(&actual.module);
        let changed_components = generated.module.changed_components(&actual.module);
        let changed_structs_or_enums = generated.module.changed_structs_or_enums(&actual.module);

        let diff = changed_globals.is_some()
            || changed_components.is_some()
            || changed_structs_or_enums.is_some();

        let incompatible_changes =
            changed_globals.as_ref().map_or(false, |c| c.incompatible_changes())
                || changed_components.as_ref().map_or(false, |c| c.incompatible_changes())
                || changed_structs_or_enums.as_ref().map_or(false, |c| c.incompatible_changes());

        if diff {
            let slint_file = actual.path.display();
            let python_file = generated.path.display();
            eprintln!(
                r#"Changes detected between {slint_file} and {python_file}
Re-run the slint compiler to re-generate the file, for example:

uxv slint-compiler -f python -o {slint_file} {python_file}
"#,
            )
        }

        if incompatible_changes {
            Err(pyo3::exceptions::PyRuntimeError::new_err(format!(
                "Incompatible API changes detected between {} and {}",
                generated.path.display(),
                actual.path.display()
            )))
        } else {
            Ok(())
        }
    }
}
