// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! This module generates Python bindings for public Slint structs using the
//! `for_each_builtin_structs` macro, reusing documentation from `builtin_structs.rs`.
//!
//! The pattern follows `cbindgen.rs`: a macro consumes `for_each_builtin_structs`,
//! matches on `BuiltinPublicStruct` variants, and generates `#[pyclass]` wrappers
//! with the original doc comments. Private structs are skipped.
#![allow(unsafe_op_in_unsafe_fn)]

use pyo3::prelude::*;

fn map_type_to_python(ty: &str) -> (&'static str, &'static str) {
    match ty {
        "bool" => ("bool", "False"),
        "SharedString" => ("str", "\"\""),
        "i32" => ("int", "0"),
        "f32" | "Coord" => ("float", "0.0"),
        _ => ("typing.Any", "None"),
    }
}

fn register_named_tuple(
    py: Python<'_>,
    m: &Bound<'_, PyModule>,
    class_name: &str,
    class_doc: &str,
    fields: &[(&str, &str, String)], // name, rust_type, doc
) -> PyResult<()> {
    let mut fields_code = String::new();
    for (name, rust_ty, doc) in fields {
        let (py_ty, default) = map_type_to_python(rust_ty);
        fields_code.push_str(&format!("    {}: {} = {}\n", name, py_ty, default));
        if !doc.is_empty() {
            fields_code.push_str("    \"\"\"\n");
            for line in doc.lines() {
                fields_code.push_str(&format!("    {}\n", line));
            }
            fields_code.push_str("    \"\"\"\n");
        }
    }

    let code = format!(
        r#"
import typing
class {}(typing.NamedTuple):
    """
    {}
    """
{}
"#,
        class_name, class_doc, fields_code
    );

    let code_c = std::ffi::CString::new(code)
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
    let file_name = std::ffi::CString::new(format!("{}.py", class_name))
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
    let module_name = std::ffi::CString::new(class_name)
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;

    let temp_module = PyModule::from_code(py, &code_c, &file_name, &module_name)?;
    let class = temp_module.getattr(class_name)?;
    m.add(class_name, class)?;
    Ok(())
}

/// This macro processes `for_each_builtin_structs` and generates `#[pyclass]` wrappers
/// for public structs only (those matched with `BuiltinPublicStruct`).
macro_rules! declare_python_public_structs {
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
        $(
            declare_python_public_structs!(@check $NameTy, $NameVariant, $Name;
                docs: [$(#[doc = $struct_doc])*],
                fields: [$( $(#[doc = $pub_doc])* $pub_field : $pub_type ,)*],
            );
        )*
    };

    (@check BuiltinPublicStruct, $Variant:ident, $Name:ident;
        docs: [$(#[doc = $struct_doc:literal])*],
        fields: [$( $(#[doc = $field_doc:literal])* $pub_field:ident : $pub_type:ident ,)*],
    ) => {
        paste::paste! {
            #[allow(non_snake_case)]
            pub fn [< register_ $Name >](py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
                let class_doc = [ $($struct_doc),* ].join("\n");
                let fields = vec![
                    $(
                        (stringify!($pub_field), stringify!($pub_type), [ $($field_doc),* ].join("\n")),
                    )*
                ];

                register_named_tuple(py, m, stringify!($Name), &class_doc, &fields)
            }
        }
    };

    // Skip all private structs
    (@check BuiltinPrivateStruct, $_variant:ident, $Name:ident;
        docs: [$(#[$struct_meta:meta])*],
        fields: [$( $(#[$field_meta:meta])* $pub_field:ident : $pub_type:ty ,)*],
    ) => {};
}

i_slint_common::for_each_builtin_structs!(declare_python_public_structs);
