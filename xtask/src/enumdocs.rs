// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use anyhow::Context;
use std::io::Write;

pub fn generate() -> Result<(), Box<dyn std::error::Error>> {
    let mut enums: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();

    macro_rules! gen_enums {
        ($( $(#[doc = $enum_doc:literal])* $(#[non_exhaustive])? enum $Name:ident { $( $(#[doc = $value_doc:literal])* $Value:ident,)* })*) => {
            $(
                let mut entry = format!("## `{}`\n\n", stringify!($Name));
                $(entry += &format!("{}\n", $enum_doc);)*
                entry += "\n";
                $(
                    let mut has_val = false;
                    entry += &format!("* **`{}`**:", to_kebab_case(stringify!($Value)));
                    $(
                        if has_val {
                            entry += "\n   ";
                        }
                        entry += &format!("{}", $value_doc);
                        has_val = true;
                    )*
                    entry += "\n";
                )*
                entry += "\n";
                enums.insert(stringify!($Name).to_string(), entry);
            )*
        }
    }

    #[allow(unused)] // for 'has_val'
    {
        i_slint_common::for_each_enums!(gen_enums);
    }

    let root = super::root_dir();

    let path = root.join("docs/language/src/builtins/builtin_enums.md");
    let mut file = std::fs::File::create(&path).context(format!("error creating {path:?}"))?;

    file.write_all(
        br#"<!-- Generated with `cargo xtask enumdocs` from internal/commons/enums.rs -->
# Builtin Enums

Enum value can be referenced by using the name of the enum and the name of the value
separated by a dot. (eg: `TextHorizontalAlignment.left`)

The name of the enum can be omitted in bindings of the type of that enum, or if the
return value of a callback is of that enum.

The default value of each enum type is always the first value.

"#,
    )?;

    for (_, v) in enums {
        // BTreeMap<i64, String>
        write!(file, "{v}")?;
    }

    Ok(())
}

/// Convert a ascii pascal case string to kebab case
fn to_kebab_case(str: &str) -> String {
    let mut result = Vec::with_capacity(str.len());
    for x in str.as_bytes() {
        if x.is_ascii_uppercase() {
            if !result.is_empty() {
                result.push(b'-');
            }
            result.push(x.to_ascii_lowercase());
        } else {
            result.push(*x);
        }
    }
    String::from_utf8(result).unwrap()
}
