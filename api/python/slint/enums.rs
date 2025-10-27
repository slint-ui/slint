// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::collections::HashMap;

use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
#[cfg(feature = "stubgen")]
use pyo3_stub_gen::derive::gen_stub_pyclass_enum;

static BUILTIN_ENUM_CLASSES: PyOnceLock<HashMap<String, Py<PyAny>>> = PyOnceLock::new();

macro_rules! generate_enum_support {
    ($(
        $(#[$enum_attr:meta])*
        enum $Name:ident {
            $(
                $(#[$value_attr:meta])*
                $Value:ident,
            )*
        }
    )*) => {
        #[cfg(feature = "stubgen")]
        pub(super) mod stub_enums {
            use super::*;

            $(
                #[gen_stub_pyclass_enum]
                #[pyclass(module = "slint.core", rename_all = "lowercase")]
                #[allow(non_camel_case_types)]
                $(#[$enum_attr])*
                pub enum $Name {
                    $(
                        $(#[$value_attr])*
                        $Value,
                    )*
                }
            )*
        }

        fn register_built_in_enums(
            py: Python<'_>,
            module: &Bound<'_, PyModule>,
            enum_base: &Bound<'_, PyAny>,
            enum_classes: &mut HashMap<String, Py<PyAny>>,
        ) -> PyResult<()> {
            $(
                {
                    let name = stringify!($Name);
                    let variants = vec![
                        $(
                            {
                                let value = i_slint_core::items::$Name::$Value.to_string();
                                (stringify!($Value).to_ascii_lowercase(), value)
                            },
                        )*
                    ];

                    let cls = enum_base.call((name, variants), None)?;
                    let cls_owned = cls.unbind();
                    module.add(name, cls_owned.bind(py))?;
                    enum_classes.insert(name.to_string(), cls_owned);
                }
            )*
            Ok(())
        }
    };
}

i_slint_common::for_each_enums!(generate_enum_support);

pub fn register_enums(py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    let enum_base = crate::value::enum_class(py).into_bound(py);
    let mut enum_classes: HashMap<String, Py<PyAny>> = HashMap::new();

    register_built_in_enums(py, module, &enum_base, &mut enum_classes)?;

    let _ = BUILTIN_ENUM_CLASSES.set(py, enum_classes);

    Ok(())
}

pub fn built_in_enum_classes(py: Python<'_>) -> HashMap<String, Py<PyAny>> {
    BUILTIN_ENUM_CLASSES
        .get(py)
        .map(|map| map.iter().map(|(name, class)| (name.clone(), class.clone_ref(py))).collect())
        .unwrap_or_default()
}
