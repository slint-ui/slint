// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::fs::File;
use std::io::{BufWriter, Write};

fn map_type(ty: &str) -> &str {
    match ty {
        "bool" => "bool",
        "SharedString" => "str",
        "i32" => "int",
        "f32" | "Coord" => "float",
        _ => "typing.Any",
    }
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

macro_rules! generate_builtin_structs_pyi {
    ($(
        $(#[doc = $struct_doc:literal])*
        $(#[non_exhaustive])?
        $(#[derive(Copy, Eq)])?
        struct $Name:ident {
            @name = $NameTy:ident :: $Variant:ident,
            export {
                $( $(#[doc = $pub_doc:literal])* $pub_field:ident : $pub_type:ident, )*
            }
            private {
                $($private:tt)*
            }
        }
    )*) => {
        fn generate_pyi(writer: &mut impl Write) {
            // REUSE-IgnoreStart
            writeln!(writer, "# Copyright © SixtyFPS GmbH <info@slint.dev>").unwrap();
            writeln!(writer, "# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0").unwrap();
            // REUSE-IgnoreEnd
            writeln!(writer, "").unwrap();
            writeln!(writer, "import typing").unwrap();

            $(
                generate_builtin_structs_pyi!(@check writer, $NameTy, $Name,
                    [$($struct_doc),*],
                    [$([$($pub_doc),*] $pub_field : $pub_type),*]
                );
            )*
        }
    };
    (@check $writer:expr, BuiltinPublicStruct, $Name:ident,
        [$($struct_doc:literal),*],
        [$([$($pub_doc:literal),*] $pub_field:ident : $pub_type:ident),*]
    ) => {
        writeln!($writer, "\nclass {}(typing.NamedTuple):", stringify!($Name)).unwrap();
        let struct_doc = vec![$($struct_doc),*].join("\n").trim().to_string();
        if !struct_doc.is_empty() {
            writeln!($writer, "    \"\"\"").unwrap();
            for line in struct_doc.lines() {
                if line.is_empty() {
                    writeln!($writer).unwrap();
                } else {
                    writeln!($writer, "    {}", line).unwrap();
                }
            }
            writeln!($writer, "    \"\"\"").unwrap();
        }
        writeln!($writer, "").unwrap();
        $(
            writeln!($writer, "    {}: {} = {}", stringify!($pub_field), map_type(stringify!($pub_type)), map_default(stringify!($pub_type))).unwrap();
            let field_doc = vec![$($pub_doc),*].join("\n").trim().to_string();
            if !field_doc.is_empty() {
                writeln!($writer, "    \"\"\"").unwrap();
                for line in field_doc.lines() {
                    if line.is_empty() {
                        writeln!($writer).unwrap();
                    } else {
                        writeln!($writer, "    {}", line).unwrap();
                    }
                }
                writeln!($writer, "    \"\"\"").unwrap();
            }
        )*
    };
    (@check $writer:expr, BuiltinPrivateStruct, $Name:ident,
        [$($struct_doc:literal),*],
        [$([$($pub_doc:literal),*] $pub_field:ident : $pub_type:ident),*]
    ) => {};
}

i_slint_common::for_each_builtin_structs!(generate_builtin_structs_pyi);

fn main() {
    let pyi_path = std::path::Path::new("slint/language.pyi");
    if let Some(parent) = pyi_path.parent() {
        std::fs::create_dir_all(parent).expect("Failed to create slint/ directory");
    }
    let file = File::create(pyi_path).expect("Failed to create language.pyi");
    let mut writer = BufWriter::new(file);
    generate_pyi(&mut writer);
}
