// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore kwargs namedtuple
//! This module generates Python bindings for the public Slint language types:
//!
//! * `pub struct` declarations (via `for_each_builtin_structs!`) become
//!   `collections.namedtuple` classes with documented defaults.
//! * `pub enum` declarations (via `for_each_enums!`) become `enum.Enum`
//!   subclasses whose member `name` is a Python-safe identifier and whose
//!   `value` is the kebab-case string the Slint runtime expects.
//!
//! Both kinds are registered as attributes of the `slint.language` submodule.

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

/// The Python value of a field default declared in builtin_structs.rs.
/// Enum defaults resolve to the already registered enum member,
/// so `register_structs` must run after `register_enums`.
fn declared_default_value<'py>(
    py: Python<'py>,
    m: &Bound<'py, PyModule>,
    tokens: &str,
    rust_ty: &str,
) -> PyResult<Bound<'py, PyAny>> {
    let text: String =
        tokens.chars().filter(|c| !c.is_whitespace() && *c != '(' && *c != ')').collect();
    if let Some((enum_name, variant)) = text.split_once("::") {
        let member = to_kebab_case(variant.trim_start_matches("r#")).replace('-', "_");
        return language_submodule(py, m)?.getattr(enum_name)?.getattr(member.as_str());
    }
    if let Ok(b) = text.parse::<bool>() {
        return b.into_bound_py_any(py);
    }
    let value = text
        .parse::<f64>()
        .unwrap_or_else(|_| panic!("unsupported builtin struct field default `{tokens}`"));
    if rust_ty == "i32" {
        (value as i32).into_bound_py_any(py)
    } else {
        value.into_bound_py_any(py)
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
    fields: &[(&str, &str, String, Option<&str>)], // name, rust_type, doc, declared default
) -> PyResult<()> {
    let collections = py.import("collections")?;
    let namedtuple = collections.getattr("namedtuple")?;

    let mut field_names = Vec::new();
    let mut defaults = Vec::new();
    let mut full_doc = class_doc.to_string();

    for (name, rust_ty, doc, declared) in fields {
        field_names.push(*name);
        defaults.push(match declared {
            Some(tokens) => declared_default_value(py, m, tokens, rust_ty)?,
            None => get_default_value(py, rust_ty)?,
        });
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
/// `slint.language` submodule.
/// Each variant becomes a member whose `name` is a Python-safe identifier
/// (dashes replaced with underscores) and whose `value` is the kebab-case string
/// the Slint runtime expects (`AccessibleRole.radio_button.value` is `"radio-button"`).
/// The interpreter's enum round-trip in `value.rs` keys off `member.value`,
/// so multi-word variants round-trip correctly.
fn register_enum_class(
    py: Python<'_>,
    m: &Bound<'_, PyModule>,
    class_name: &str,
    class_doc: &str,
    variants: &[(String, String, String)], // (python_member_name, kebab_value, variant_doc)
) -> PyResult<()> {
    let enum_mod = py.import("enum")?;
    let enum_ctor = enum_mod.getattr("Enum")?;

    let members: Vec<(String, String)> =
        variants.iter().map(|(name, value, _)| (name.clone(), value.clone())).collect();

    let kwargs = PyDict::new(py);
    kwargs.set_item("module", "slint.language")?;
    let class = enum_ctor.call((class_name, members), Some(&kwargs))?;

    let mut full_doc = class_doc.to_string();
    for (name, _, doc) in variants {
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
    ($(
        $(#[doc = $struct_doc:literal])*
        $(#[non_exhaustive])?
        $(#[derive(Copy, Eq)])?
        $vis:vis struct $Name:ident {
            $( $(#[doc = $field_doc:literal])* $field:ident : $field_type:ident $(= $field_default:expr)?, )*
        }
    )*) => {
        fn register_structs(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
            $(
                if stringify!($vis) == "pub" {
                    let class_doc = [ $($struct_doc),* ].join("\n");
                    let fields = vec![
                        $(
                            (stringify!($field), stringify!($field_type), [ $($field_doc),* ].join("\n"),
                                i_slint_common::builtin_struct_field_default_tokens!($($field_default)?)),
                        )*
                    ];
                    register_named_tuple(py, m, stringify!($Name), &class_doc, &fields)?;
                }
            )*
            Ok(())
        }
    };
}

i_slint_common::for_each_builtin_structs!(declare_python_public_structs);

/// Walks `for_each_enums!` and registers every `pub enum` as an `enum.Enum`
/// subclass on `slint.language`.
/// Private enums are skipped.
///
/// Each variant's Python member name is a Python-safe identifier derived from the
/// kebab-case form (dashes replaced with underscores), and the value is the kebab-case
/// string the Slint runtime expects.
/// For single-word variants name and value coincide (`ColorScheme.dark.value == "dark"`);
/// for multi-word variants they differ (`AccessibleRole.radio_button.value == "radio-button"`).
/// The interpreter's enum round-trip in `value.rs` keys off `member.value`.
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
                    let variants: Vec<(String, String, String)> = vec![
                        $(
                            {
                                let kebab = to_kebab_case(stringify!($Value));
                                (
                                    kebab.replace('-', "_"),
                                    kebab,
                                    {
                                        let value_doc_lines: Vec<&str> = vec![$($value_doc),*];
                                        value_doc_lines.join("\n")
                                    },
                                )
                            },
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
    // Enums first: struct field defaults may reference their members
    register_enums(py, m)?;
    register_structs(py, m)?;
    Ok(())
}
