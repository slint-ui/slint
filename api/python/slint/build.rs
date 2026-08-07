// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::fs::File;
use std::io::{BufWriter, Write};

/// The names of the enums declared in the `slint.language` submodule
/// (only `pub` enums are registered; see `register_enums` in language.rs).
macro_rules! collect_public_enums {
    ($( $(#[$enum_attr:meta])* $vis:vis enum $Name:ident { $($_body:tt)* })*) => {
        fn public_enum_names() -> std::collections::HashSet<&'static str> {
            let mut names = std::collections::HashSet::new();
            $(if stringify!($vis) == "pub" {
                names.insert(stringify!($Name));
            })*
            names
        }
    };
}

i_slint_common::for_each_enums!(collect_public_enums);

fn map_type(
    ty: &str,
    public_enums: &std::collections::HashSet<&'static str>,
    has_declared_default: bool,
) -> String {
    // Enums in the same submodule; resolved as forward references via
    // `from __future__ import annotations`. Fields without a declared default
    // value default to None at runtime.
    if public_enums.contains(ty) {
        return if has_declared_default { ty.into() } else { format!("{ty} | None") };
    }
    match ty {
        "bool" => "bool",
        "SharedString" => "str",
        "i32" => "int",
        "f32" | "Coord" => "float",
        // Types exposed by the binding outside the `language` submodule.
        "DataTransfer" => "DataTransfer | None",
        "LogicalPosition" => "LogicalPosition | None",
        _ => "typing.Any",
    }
    .into()
}

fn map_default(ty: &str) -> &str {
    match ty {
        "bool" => "False",
        "SharedString" => "\"\"",
        "i32" => "0",
        "f32" | "Coord" => "0.0",
        _ => "None",
    }
}

/// The Python form of a field default value declared in builtin_structs.rs
fn declared_default(tokens: &str) -> String {
    let text: String = tokens.chars().filter(|c| !c.is_whitespace()).collect();
    match text.split_once("::") {
        // Enum members are named after the kebab-case value, with underscores
        Some((enum_name, variant)) => {
            let member = to_kebab_case(variant.trim_start_matches("r#")).replace('-', "_");
            format!("{enum_name}.{member}")
        }
        None => match text.as_str() {
            "true" => "True".into(),
            "false" => "False".into(),
            // Number literals are the same in Python
            _ => text,
        },
    }
}

macro_rules! generate_builtin_structs_pyi {
    ($(
        $(#[doc = $struct_doc:literal])*
        $(#[non_exhaustive])?
        $(#[derive(Copy, Eq)])?
        $vis:vis struct $Name:ident {
            $( $(#[doc = $field_doc:literal])* $field:ident : $field_type:ident $(= $field_default:expr)?, )*
        }
    )*) => {
        fn generate_pyi(writer: &mut impl Write) {
            // REUSE-IgnoreStart
            writeln!(writer, "# Copyright © SixtyFPS GmbH <info@slint.dev>").unwrap();
            writeln!(writer, "# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0").unwrap();
            // REUSE-IgnoreEnd
            writeln!(writer, "").unwrap();
            writeln!(writer, "from __future__ import annotations").unwrap();
            writeln!(writer, "import typing").unwrap();
            writeln!(writer, "from slint import DataTransfer, LogicalPosition").unwrap();

            let public_enums = public_enum_names();
            $(
                if stringify!($vis) == "pub" {
                    writeln!(writer, "\nclass {}(typing.NamedTuple):", stringify!($Name)).unwrap();
                    let struct_doc = vec![$($struct_doc),*].join("\n").trim().to_string();
                    if !struct_doc.is_empty() {
                        writeln!(writer, "    \"\"\"").unwrap();
                        for line in struct_doc.lines() {
                            if line.is_empty() {
                                writeln!(writer).unwrap();
                            } else {
                                writeln!(writer, "    {}", line).unwrap();
                            }
                        }
                        writeln!(writer, "    \"\"\"").unwrap();
                    }
                    writeln!(writer, "").unwrap();
                    $(
                        let declared = i_slint_common::builtin_struct_field_default_tokens!($($field_default)?);
                        let field_type = map_type(stringify!($field_type), &public_enums, declared.is_some());
                        let default = declared
                            .map(declared_default)
                            .unwrap_or_else(|| map_default(stringify!($field_type)).to_string());
                        writeln!(writer, "    {}: {} = {}", stringify!($field), field_type, default).unwrap();
                        let field_doc_str = vec![$($field_doc),*].join("\n").trim().to_string();
                        if !field_doc_str.is_empty() {
                            writeln!(writer, "    \"\"\"").unwrap();
                            for line in field_doc_str.lines() {
                                if line.is_empty() {
                                    writeln!(writer).unwrap();
                                } else {
                                    writeln!(writer, "    {}", line).unwrap();
                                }
                            }
                            writeln!(writer, "    \"\"\"").unwrap();
                        }
                    )*
                }
            )*
        }
    };
}

i_slint_common::for_each_builtin_structs!(generate_builtin_structs_pyi);

/// Convert a Rust CamelCase variant identifier (e.g. `NoDrop`) into the kebab-case string
/// the Slint runtime stores in `Enumeration::values` (e.g. `"no-drop"`).
/// Matches the helper in `i_slint_compiler::generator::to_kebab_case`.
fn to_kebab_case(s: &str) -> String {
    let mut out = Vec::with_capacity(s.len());
    for b in s.as_bytes() {
        if b.is_ascii_uppercase() {
            if !out.is_empty() {
                out.push(b'-');
            }
            out.push(b.to_ascii_lowercase());
        } else {
            out.push(*b);
        }
    }
    String::from_utf8(out).unwrap()
}

macro_rules! generate_public_enums_pyi {
    ($(
        $(#[doc = $enum_doc:literal])*
        $(#[non_exhaustive])?
        $vis:vis enum $Name:ident {
            $( $(#[doc = $value_doc:literal])* $Value:ident, )*
        }
    )*) => {
        fn generate_enums_pyi(writer: &mut impl Write) {
            $(
                if stringify!($vis) == "pub" {
                    writeln!(writer, "\nclass {}(enum.Enum):", stringify!($Name)).unwrap();
                    let class_doc_lines: Vec<&str> = vec![$($enum_doc),*];
                    let class_doc = class_doc_lines.join("\n").trim().to_string();
                    if !class_doc.is_empty() {
                        writeln!(writer, "    \"\"\"").unwrap();
                        for line in class_doc.lines() {
                            if line.is_empty() {
                                writeln!(writer).unwrap();
                            } else {
                                writeln!(writer, "    {}", line).unwrap();
                            }
                        }
                        writeln!(writer, "    \"\"\"").unwrap();
                    }
                    writeln!(writer, "").unwrap();
                    $({
                        let kebab = to_kebab_case(stringify!($Value));
                        let member_name = kebab.replace('-', "_");
                        writeln!(writer, "    {} = \"{}\"", member_name, kebab).unwrap();
                        let value_doc_lines: Vec<&str> = vec![$($value_doc),*];
                        let value_doc = value_doc_lines.join("\n").trim().to_string();
                        if !value_doc.is_empty() {
                            writeln!(writer, "    \"\"\"").unwrap();
                            for line in value_doc.lines() {
                                if line.is_empty() {
                                    writeln!(writer).unwrap();
                                } else {
                                    writeln!(writer, "    {}", line).unwrap();
                                }
                            }
                            writeln!(writer, "    \"\"\"").unwrap();
                        }
                    })*
                }
            )*
        }
    };
}

i_slint_common::for_each_enums!(generate_public_enums_pyi);

fn main() {
    let pyi_path = std::path::Path::new("slint/language.pyi");
    if let Some(parent) = pyi_path.parent() {
        std::fs::create_dir_all(parent).expect("Failed to create slint/ directory");
    }
    let file = File::create(pyi_path).expect("Failed to create language.pyi");
    let mut writer = BufWriter::new(file);
    generate_pyi(&mut writer);
    writeln!(&mut writer, "\nimport enum").unwrap();
    generate_enums_pyi(&mut writer);
}
