// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! This module generates Python `typing.NamedTuple` bindings for public Slint
//! structs using the `for_each_builtin_structs` macro, reusing documentation
//! from `builtin_structs.rs`.
//!
//! The pattern follows `cbindgen.rs`: a macro consumes `for_each_builtin_structs`,
//! matches on `BuiltinPublicStruct` variants, and generates NamedTuple classes
//! with the original doc comments. Private structs are skipped.

use pyo3::prelude::*;
use std::fmt::Write;

fn map_type_to_python(ty: &str) -> (&'static str, &'static str) {
    match ty {
        "bool" => ("bool", "False"),
        "SharedString" => ("str", "\"\""),
        "i32" => ("int", "0"),
        "f32" | "Coord" => ("float", "0.0"),
        _ => ("typing.Any", "None"),
    }
}

/// Dynamically creates a Python `typing.NamedTuple` class and registers it
/// in the `slint.language` submodule.
///
/// This works by generating Python source code for the class, compiling it
/// via `PyModule::from_code`, and then moving the resulting class object
/// into the `slint.language` submodule.
fn register_named_tuple(
    py: Python<'_>,
    m: &Bound<'_, PyModule>,
    class_name: &str,
    class_doc: &str,
    fields: &[(&str, &str, String)], // name, rust_type, doc
) -> PyResult<()> {
    // Phase 1: Build Python source code for the NamedTuple class.
    // Each field gets a type annotation and a default value, plus an optional docstring.
    let mut fields_code = String::new();
    for (name, rust_ty, doc) in fields {
        let (py_ty, default) = map_type_to_python(rust_ty);
        let _ = writeln!(fields_code, "    {name}: {py_ty} = {default}");
        if !doc.is_empty() {
            fields_code.push_str("    \"\"\"\n");
            for line in doc.lines() {
                let _ = writeln!(fields_code, "    {line}");
            }
            fields_code.push_str("    \"\"\"\n");
        }
    }

    let code = format!(
        r#"
import typing
class {class_name}(typing.NamedTuple):
    """
    {class_doc}
    """
{fields_code}
"#
    );

    // Phase 2: Compile the generated source into a temporary Python module
    // and extract the class object from it.
    let to_cstring = |s: String| {
        std::ffi::CString::new(s)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
    };

    let temp_module = PyModule::from_code(
        py,
        &to_cstring(code)?,
        &to_cstring(format!("{class_name}.py"))?,
        &to_cstring(class_name.to_string())?,
    )?;
    let class = temp_module.getattr(class_name)?;

    // Set the module path so repr/pickle/introspection report "slint.language"
    class.setattr("__module__", "slint.language")?;

    // Phase 3: Register the class in the "slint.language" submodule.
    // The submodule is created lazily on the first call and reused for subsequent structs.
    let language_mod = match m.getattr("language") {
        Ok(existing) => existing.cast_into::<PyModule>()?,
        Err(_) => {
            let sub = PyModule::new(py, "language")?;
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
