// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! This module generates Python `typing.NamedTuple` bindings for public Slint
//! structs using the `for_each_builtin_structs` macro, reusing documentation
//! from `builtin_structs.rs`.
//!
//! The pattern follows `cbindgen.rs`: a macro consumes `for_each_builtin_structs`,
//! matches on `BuiltinPublicStruct` variants, and generates NamedTuple classes
//! with the original doc comments. Private structs are skipped.

use pyo3::IntoPyObjectExt;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyTuple};

fn get_default_value<'py>(py: Python<'py>, ty: &str) -> PyResult<Bound<'py, PyAny>> {
    match ty {
        "bool" => false.into_bound_py_any(py),
        "SharedString" => "".into_bound_py_any(py),
        "i32" => 0_i32.into_bound_py_any(py),
        "f32" | "Coord" => 0.0_f32.into_bound_py_any(py),
        _ => Ok(py.None().into_bound(py)),
    }
}

/// Dynamically creates a Python `typing.NamedTuple` class and registers it
/// in the `slint.language` submodule.
fn register_named_tuple(
    py: Python<'_>,
    m: &Bound<'_, PyModule>,
    class_name: &str,
    class_doc: &str,
    fields: &[(&str, &str, String)], // name, rust_type, doc
) -> PyResult<()> {
    let collections = py.import("collections")?;
    let namedtuple = collections.getattr("namedtuple")?;

    let mut field_names = Vec::new();
    let mut defaults = Vec::new();
    let mut full_doc = class_doc.to_string();

    for (name, rust_ty, doc) in fields {
        field_names.push(*name);
        defaults.push(get_default_value(py, rust_ty)?);
        if !doc.is_empty() {
            use std::fmt::Write;
            let _ = write!(full_doc, "\n\n:param {name}: {doc}");
        }
    }

    let kwargs = PyDict::new(py);
    kwargs.set_item("defaults", PyTuple::new(py, defaults)?)?;
    kwargs.set_item("module", "slint.language")?;

    let class = namedtuple.call((class_name, field_names), Some(&kwargs))?;
    class.setattr("__doc__", full_doc)?;

    // Register the class in the "slint.language" submodule.
    // The submodule is created lazily on the first call and reused for subsequent structs.
    let language_mod = match m.getattr("language") {
        Ok(existing) => existing.cast_into::<PyModule>()?,
        Err(_) => {
            let sub = PyModule::new(py, "slint.language")?;
            m.add("language", &sub)?;
            // Register in sys.modules so "from slint.language import ..." works
            let sys = py.import("sys")?;
            let modules = sys.getattr("modules")?;
            modules.set_item("slint.language", &sub)?;
            sub
        }
    };

    language_mod.add(class_name, class)?;
    Ok(())
}

/// This macro processes `for_each_builtin_structs` and generates a single `register_all`
/// function that registers all public structs as NamedTuples in the `slint.language` submodule.
macro_rules! declare_python_public_structs {
    // Top-level arm: matches the full list of struct definitions emitted by
    // `for_each_builtin_structs!`. For each struct, it delegates to the
    // `@register` arm which decides whether to register or skip it based
    // on whether it's a BuiltinPublicStruct or BuiltinPrivateStruct.
    ($(
        $(#[doc = $struct_doc:literal])*
        $(#[non_exhaustive])?
        $(#[derive(Copy, Eq)])?
        struct $Name:ident {
            @name = $NameTy:ident :: $NameVariant:ident,
            export {
                $( $(#[doc = $pub_doc:literal])* $pub_field:ident : $pub_type:ident, )*
            }
            private {
                $( $(#[doc = $pri_doc:literal])* $pri_field:ident : $pri_type:ty, )*
            }
        }
    )*) => {
        /// Registers all public builtin structs as NamedTuples in `slint.language`.
        /// Called once during module initialization from `lib.rs`.
        pub fn register_all(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
            $(
                declare_python_public_structs!(@register $NameTy, $Name, py, m;
                    docs: [$(#[doc = $struct_doc])*],
                    fields: [$( $(#[doc = $pub_doc])* $pub_field : $pub_type ,)*],
                );
            )*
            Ok(())
        }
    };

    // Public struct arm: collects doc comments and field metadata, then calls
    // `register_named_tuple` to create and register the NamedTuple class.
    (@register BuiltinPublicStruct, $Name:ident, $py:ident, $m:ident;
        docs: [$(#[doc = $struct_doc:literal])*],
        fields: [$( $(#[doc = $field_doc:literal])* $pub_field:ident : $pub_type:ident ,)*],
    ) => {
        {
            let class_doc = [ $($struct_doc),* ].join("\n");
            let fields = vec![
                $(
                    (stringify!($pub_field), stringify!($pub_type), [ $($field_doc),* ].join("\n")),
                )*
            ];
            register_named_tuple($py, $m, stringify!($Name), &class_doc, &fields)?;
        }
    };

    // Private struct arm: intentionally empty — private structs are not exposed to Python.
    (@register BuiltinPrivateStruct, $_Name:ident, $py:ident, $m:ident;
        docs: [$(#[$struct_meta:meta])*],
        fields: [$( $(#[$field_meta:meta])* $pub_field:ident : $pub_type:ty ,)*],
    ) => {};
}

i_slint_common::for_each_builtin_structs!(declare_python_public_structs);
