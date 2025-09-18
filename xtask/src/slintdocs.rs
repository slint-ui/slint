// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cspell:ignore slintdocs pipenv pipfile

use anyhow::{Context, Result};
use std::fs::create_dir_all;
use std::io::{BufWriter, Write};
use std::path::Path;
use xshell::{cmd, Shell};

pub fn generate() -> Result<(), Box<dyn std::error::Error>> {
    generate_enum_docs()?;
    generate_builtin_struct_docs()?;

    let root = super::root_dir();

    let docs_source_dir = root.join("docs/astro");

    {
        let sh = Shell::new()?;
        let _p = sh.push_dir(&docs_source_dir);
        cmd!(sh, "pnpm install --frozen-lockfile --ignore-scripts").run()?;
        cmd!(sh, "pnpm run build").run()?;
    }

    Ok(())
}

fn write_individual_enum_files(
    root_dir: &Path,
    enums: &std::collections::BTreeMap<String, EnumDoc>,
) -> Result<(), Box<dyn std::error::Error>> {
    let enums_dir = root_dir.join("docs/astro/src/content/collections/enums");
    create_dir_all(&enums_dir).context(format!(
        "Failed to create folder holding individual enum doc files {enums_dir:?}"
    ))?;

    for (k, e) in enums {
        let path = enums_dir.join(format!("{k}.md"));
        let mut file = BufWriter::new(
            std::fs::File::create(&path).context(format!("error creating {path:?}"))?,
        );

        write!(
            file,
            r#"---
title: {0}
description: {0} content
---

<!-- Generated with `cargo xtask slintdocs` from internal/commons/enums.rs -->

`{0}`

{1}
"#,
            k, e.description
        )?;
        for v in &e.values {
            write!(
                file,
                r#"* **`{}`**: {}
"#,
                v.key, v.description
            )?;
        }

        file.flush()?;
    }
    Ok(())
}

pub struct EnumValueDoc {
    key: String,
    description: String,
}

pub struct EnumDoc {
    pub description: String,
    pub values: Vec<EnumValueDoc>,
}

pub fn extract_enum_docs() -> std::collections::BTreeMap<String, EnumDoc> {
    let mut enums: std::collections::BTreeMap<String, EnumDoc> = std::collections::BTreeMap::new();

    macro_rules! gen_enums {
        ($( $(#[doc = $enum_doc:literal])* $(#[non_exhaustive])? enum $Name:ident { $( $(#[doc = $value_doc:literal])* $Value:ident,)* })*) => {
            $(
                let name = stringify!($Name).to_string();
                let mut description = String::new();
                $( description += &format!("{}\n", $enum_doc); )*

                let mut values = Vec::new();

                $(
                    let mut value_docs = String::new();
                    $(
                        value_docs += $value_doc;
                    )*
                    values.push(EnumValueDoc { key: to_kebab_case(stringify!($Value)), description: value_docs });
                )*

                enums.insert(name, EnumDoc { description, values});
            )*
        }
    }

    #[allow(unused)] // for 'has_val'
    {
        i_slint_common::for_each_enums!(gen_enums);
    }

    enums
}

pub fn generate_enum_docs() -> Result<(), Box<dyn std::error::Error>> {
    let enums = extract_enum_docs();

    write_individual_enum_files(&super::root_dir(), &enums)?;

    Ok(())
}

pub struct StructFieldDoc {
    key: String,
    description: String,
    type_name: String,
}

pub struct StructDoc {
    pub description: String,
    pub fields: Vec<StructFieldDoc>,
}

pub fn extract_builtin_structs() -> std::collections::BTreeMap<String, StructDoc> {
    // `Point` should be in the documentation, but it's not inside of `for_each_builtin_structs`,
    // so we manually create its entry first.
    let mut structs = std::collections::BTreeMap::from([(
        "Point".to_string(),
        StructDoc {
            description: "This structure represents a point with x and y coordinate".to_string(),
            fields: vec![
                StructFieldDoc {
                    key: "x".to_string(),
                    description: String::new(),
                    type_name: "length".to_string(),
                },
                StructFieldDoc {
                    key: "y".to_string(),
                    description: String::new(),
                    type_name: "length".to_string(),
                },
            ],
        },
    )]);

    macro_rules! map_type {
        (i32) => {
            stringify!(int)
        };
        (f32) => {
            stringify!(float)
        };
        (SharedString) => {
            stringify!(string)
        };
        (Coord) => {
            "length"
        };
        (Image) => {
            "image"
        };
        ($pub_type:ident) => {
            stringify!($pub_type)
        };
    }

    macro_rules! gen_structs {
        ($(
            $(#[doc = $struct_doc:literal])*
            $(#[non_exhaustive])?
            $(#[derive(Copy, Eq)])?
            struct $Name:ident {
                @name = $inner_name:literal
                export {
                    $( $(#[doc = $pub_doc:literal])* $pub_field:ident : $pub_type:ident, )*
                }
                private {
                    $( $(#[doc = $pri_doc:literal])* $pri_field:ident : $pri_type:ty, )*
                }
            }
        )*) => {
            $(
                let name = stringify!($Name).to_string();
                let mut description = String::new();
                $(description += &format!("{}\n", $struct_doc);)*

                let mut fields = Vec::new();
                $(
                    let key = stringify!($pub_field).to_string();
                    let type_name = map_type!($pub_type).to_string();
                    let mut f_description = String::new();
                    $(
                        f_description += &format!("{}", $pub_doc);
                    )*
                    fields.push(StructFieldDoc { key, description: f_description, type_name });
                )*
                structs.insert(name, StructDoc { description, fields });
            )*
        }
    }

    i_slint_common::for_each_builtin_structs!(gen_structs);

    // `StateInfo` should not be in the documentation, so remove it again
    structs.remove("StateInfo");
    // Internal type
    structs.remove("MenuEntry");
    // Experimental type
    structs.remove("DropEvent");

    structs
}

fn write_individual_struct_files(
    root_dir: &Path,
    structs: std::collections::BTreeMap<String, StructDoc>,
) -> Result<(), Box<dyn std::error::Error>> {
    let structs_dir = root_dir.join("docs/astro/src/content/collections/structs");
    create_dir_all(&structs_dir).context(format!(
        "Failed to create folder holding individual structs doc files {structs_dir:?}"
    ))?;

    for (s, v) in &structs {
        let path = structs_dir.join(format!("{s}.md"));
        let mut file = BufWriter::new(
            std::fs::File::create(&path).context(format!("error creating {path:?}"))?,
        );

        write!(
            file,
            r#"---
title: {0}
description: {0} content
---

<!-- Generated with `cargo xtask slintdocs` from internal/common/builtin_structs.rs -->

`{0}`

{1}
"#,
            s, v.description
        )?;

        for f in &v.fields {
            write!(
                file,
                r#"- **`{}`** (_{}_): {}
"#,
                f.key, f.type_name, f.description
            )?;
        }

        file.flush()?;
    }

    Ok(())
}

pub fn generate_builtin_struct_docs() -> Result<(), Box<dyn std::error::Error>> {
    let structs = extract_builtin_structs();
    write_individual_struct_files(&super::root_dir(), structs)
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
