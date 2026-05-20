// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore kwargs namedtuple
//! This module generates Python bindings for the public Slint language types:
//!
//! * `BuiltinPublicStruct` structs (via `for_each_builtin_structs!`) become
//!   `collections.namedtuple` classes with documented defaults.
//! * `pub enum` declarations (via `for_each_enums!`) become `enum.Enum`
//!   subclasses whose member name and value are the kebab-case strings the
//!   Slint runtime expects.
//!
//! Both kinds are registered as attributes of the `slint.language` submodule.

use i_slint_compiler::generator::python::ident;
use i_slint_compiler::generator::to_kebab_case;
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

/// Returns the `slint.language` submodule, creating and registering it on the parent
/// module + `sys.modules` on first call so `from slint.language import …` works.
fn language_submodule<'py>(
    py: Python<'py>,
    m: &Bound<'py, PyModule>,
) -> PyResult<Bound<'py, PyModule>> {
    match m.getattr("language") {
        Ok(existing) => Ok(existing.cast_into::<PyModule>()?),
        Err(_) => {
            let sub = PyModule::new(py, "slint.language")?;
            m.add("language", &sub)?;
            let sys = py.import("sys")?;
            let modules = sys.getattr("modules")?;
            modules.set_item("slint.language", &sub)?;
            Ok(sub)
        }
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

    language_submodule(py, m)?.add(class_name, class)?;
    Ok(())
}

/// Dynamically creates a Python `enum.Enum` subclass and registers it in the
/// `slint.language` submodule. Each variant becomes a member whose name AND value
/// are the kebab-case string the Slint runtime expects (`ColorScheme.dark.value`
/// is `"dark"`). The interpreter's enum round-trip already keys off
/// `member.name`, so name == value keeps the existing path working without any
/// changes to `value.rs`.
fn register_enum_class(
    py: Python<'_>,
    m: &Bound<'_, PyModule>,
    class_name: &str,
    class_doc: &str,
    variants: &[(String, String)], // (python_member_name, variant_doc)
) -> PyResult<()> {
    let enum_mod = py.import("enum")?;
    let enum_ctor = enum_mod.getattr("Enum")?;

    let members: Vec<(String, String)> =
        variants.iter().map(|(name, _)| (name.clone(), name.clone())).collect();

    let kwargs = PyDict::new(py);
    kwargs.set_item("module", "slint.language")?;
    let class = enum_ctor.call((class_name, members), Some(&kwargs))?;

    let mut full_doc = class_doc.to_string();
    for (name, doc) in variants {
        if !doc.is_empty() {
            use std::fmt::Write;
            let _ = write!(full_doc, "\n\n* ``{name}``: {doc}");
        }
    }
    class.setattr("__doc__", full_doc)?;

    language_submodule(py, m)?.add(class_name, &class)?;
    Ok(())
}

/// This macro processes `for_each_builtin_structs` and generates a `register_structs`
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
        fn register_structs(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
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

/// Walks `for_each_enums!` and registers every `pub enum` as an `enum.Enum`
/// subclass on `slint.language`. Private enums are skipped.
///
/// Each variant's Python member name is the kebab-case string the Slint runtime expects
/// (so `member.name == member.value`). The interpreter's enum round-trip in `value.rs`
/// sends `member.name`, so keeping name == value here avoids touching that code path.
/// Multi-word variants would need a different strategy (kebab isn't a valid Python
/// identifier), but the current public set is single-word only — if a multi-word variant
/// ever joins, `register_enum_class` will need to convert to snake case for the name and
/// consumers will need `member.value` instead.
macro_rules! declare_python_public_enums {
    ($(
        $(#[doc = $enum_doc:literal])*
        $(#[non_exhaustive])?
        $vis:vis enum $Name:ident {
            $( $(#[doc = $value_doc:literal])* $Value:ident, )*
        }
    )*) => {
        fn register_enums(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
            $(
                if stringify!($vis) == "pub" {
                    let class_doc_lines: Vec<&str> = vec![$($enum_doc),*];
                    let class_doc = class_doc_lines.join("\n");
                    let variants: Vec<(String, String)> = vec![
                        $(
                            (
                                ident(to_kebab_case(stringify!($Value)).as_str()).to_string(),
                                {
                                    let value_doc_lines: Vec<&str> = vec![$($value_doc),*];
                                    value_doc_lines.join("\n")
                                },
                            ),
                        )*
                    ];
                    register_enum_class(py, m, stringify!($Name), &class_doc, &variants)?;
                }
            )*
            Ok(())
        }
    };
}

i_slint_common::for_each_enums!(declare_python_public_enums);

/// Entry point called once from `lib.rs` during module initialization.
pub fn register_all(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    register_structs(py, m)?;
    register_enums(py, m)?;
    Ok(())
}
