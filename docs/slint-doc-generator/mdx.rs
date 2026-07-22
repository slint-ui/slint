// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::Config;
use anyhow::Context;
use std::fs::create_dir_all;
use std::io::{BufWriter, Write};

/// Whether `dir` is the directory this tool generates into, and may therefore
/// delete wholesale: everything below it is machine-written. A `Config`
/// pointing anywhere else would take hand-written content with it.
fn is_generated_dir(dir: &std::path::Path, astro_dir: &std::path::Path) -> bool {
    dir.starts_with(astro_dir) && dir.file_name().is_some_and(|name| name == "generated")
}

/// Generate all markdown/mdx documentation files.
pub fn generate(cfg: &Config) -> Result<(), Box<dyn std::error::Error>> {
    // Start from an empty directory: a page left behind by an earlier run
    // (of a version that named or grouped it differently) still renders, and
    // its paragraph ids still count as duplicates of the current ones.
    assert!(
        is_generated_dir(&cfg.generated_dir, &cfg.astro_dir),
        "refusing to clear {:?}: not a `generated` directory inside {:?}",
        cfg.generated_dir,
        cfg.astro_dir,
    );
    if cfg.generated_dir.exists() {
        std::fs::remove_dir_all(&cfg.generated_dir)
            .context(format!("error clearing {:?}", cfg.generated_dir))?;
    }
    generate_enum_docs(cfg)?;
    generate_builtin_struct_docs(cfg)?;
    if !cfg.sc_only {
        generate_keys_docs(cfg)?;
    }
    crate::element_docs::generate(cfg)?;

    let enums = extract_enum_docs(cfg.include_experimental, cfg.sc_only);
    let structs = extract_builtin_structs(cfg.include_experimental, cfg.sc_only);
    if !cfg.sc_only || !enums.is_empty() || !structs.is_empty() {
        write_global_structs_enums_index(cfg, &structs, &enums)?;
    }

    if cfg.sc_only {
        crate::traceability::generate(cfg)?;
    }

    Ok(())
}

fn write_global_structs_enums_index(
    cfg: &Config,
    structs: &std::collections::BTreeMap<String, StructDoc>,
    enums: &std::collections::BTreeMap<String, EnumDoc>,
) -> Result<(), Box<dyn std::error::Error>> {
    let generated_dir = cfg.reference_dir();
    create_dir_all(&generated_dir)?;
    let path = generated_dir.join("global-structs-enums.mdx");
    let mut file =
        BufWriter::new(std::fs::File::create(&path).context(format!("error creating {path:?}"))?);

    writeln!(
        file,
        r#"---
title: Global Structs and Enums
description: Global Structs and Enums
slug: reference/global-structs-enums
---
"#
    )?;

    for name in structs.keys() {
        writeln!(
            file,
            "import {0} from \"/src/{1}/reference/structs/_{0}.md\"",
            name,
            crate::GENERATED_DIR
        )?;
    }

    if !structs.is_empty() {
        writeln!(file)?;
    }

    for name in enums.keys() {
        // `keys.md` is generated separately and documented elsewhere.
        if name == "keys" {
            continue;
        }
        // Documented in the MouseCursor type.
        if name == "BuiltInMouseCursor" {
            continue;
        }
        writeln!(
            file,
            "import {0} from \"/src/{1}/reference/enums/_{0}.md\"",
            name,
            crate::GENERATED_DIR
        )?;
    }

    writeln!(file)?;
    writeln!(file, "## Structs")?;
    writeln!(file)?;

    for name in structs.keys() {
        writeln!(file, "### {name}")?;
        writeln!(file, "<{name} />")?;
        writeln!(file)?;
    }

    writeln!(file, "## Enums")?;
    writeln!(file)?;

    for name in enums.keys() {
        if name == "keys" {
            continue;
        }
        if name == "BuiltInMouseCursor" {
            continue;
        }
        writeln!(file, "### {name}")?;
        writeln!(file, "<{name} />")?;
        writeln!(file)?;
    }

    file.flush()?;

    Ok(())
}

fn write_individual_enum_files(
    cfg: &Config,
    enums: &std::collections::BTreeMap<String, EnumDoc>,
) -> Result<(), Box<dyn std::error::Error>> {
    let enums_dir = cfg.reference_dir().join("enums");
    create_dir_all(&enums_dir).context(format!(
        "Failed to create folder holding individual enum doc files {enums_dir:?}"
    ))?;

    for (k, e) in enums {
        let path = enums_dir.join(format!("_{k}.md"));
        let mut file = BufWriter::new(
            std::fs::File::create(&path).context(format!("error creating {path:?}"))?,
        );

        write!(
            file,
            r#"---
title: {0}
description: {0} content
---

<!-- Generated with slint-doc-generator from internal/commons/enums.rs -->

"#,
            k
        )?;
        // BuiltInMouseCursor is embedded inline in the MouseCursor type documentation, where its
        // internal name must not appear; emit only the description and the values.
        if k != "BuiltInMouseCursor" {
            writeln!(file, "`{k}`\n")?;
        }
        writeln!(file, "{}", e.description)?;
        for v in &e.values {
            writeln!(file, r#"* **`{}`**: {}"#, v.key, v.description)?;
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

pub fn extract_enum_docs(
    include_experimental: bool,
    sc_only: bool,
) -> std::collections::BTreeMap<String, EnumDoc> {
    let mut enums: std::collections::BTreeMap<String, EnumDoc> = std::collections::BTreeMap::new();

    macro_rules! gen_enums {
        ($( $(#[doc = $enum_doc:literal])* $(#[non_exhaustive])? $vis:vis enum $Name:ident { $( $(#[doc = $value_doc:literal])* $Value:ident,)* })*) => {
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

    if !include_experimental {
        enums.retain(|name, _| !name.starts_with("FlexboxLayout"));
    }

    if sc_only {
        enums.retain(|_, e| crate::element_docs::is_sc_covered(&e.description));
        for e in enums.values_mut() {
            e.description = crate::element_docs::strip_sc(&e.description);
            for v in &mut e.values {
                v.description = crate::element_docs::strip_sc(&v.description);
            }
        }
    } else {
        // Even outside SC mode, the marker should never leak into output.
        for e in enums.values_mut() {
            e.description = crate::element_docs::strip_sc(&e.description);
            for v in &mut e.values {
                v.description = crate::element_docs::strip_sc(&v.description);
            }
        }
    }

    enums
}

pub fn generate_enum_docs(cfg: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let enums = extract_enum_docs(cfg.include_experimental, cfg.sc_only);
    write_individual_enum_files(cfg, &enums)?;
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

pub fn extract_builtin_structs(
    _include_experimental: bool,
    sc_only: bool,
) -> std::collections::BTreeMap<String, StructDoc> {
    // `Point` should be in the documentation, but it's not inside of `for_each_builtin_structs`,
    // so we manually create its entry first.
    let mut structs = std::collections::BTreeMap::from([
        (
            "Point".to_string(),
            StructDoc {
                description: "This structure represents a point with x and y coordinate"
                    .to_string(),
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
        ),
        (
            "Size".to_string(),
            StructDoc {
                description: "This structure represents a size with width and height".to_string(),
                fields: vec![
                    StructFieldDoc {
                        key: "width".to_string(),
                        description: String::new(),
                        type_name: "length".to_string(),
                    },
                    StructFieldDoc {
                        key: "height".to_string(),
                        description: String::new(),
                        type_name: "length".to_string(),
                    },
                ],
            },
        ),
    ]);

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
        (DataTransfer) => {
            "data-transfer"
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
            $vis:vis struct $Name:ident {
                $( $(#[doc = $field_doc:literal])* $field:ident : $field_type:ident $(= $field_default:expr)?, )*
            }
        )*) => {
            $(
                let name = stringify!($Name).to_string();
                let mut description = String::new();
                $(description += &format!("{}\n", $struct_doc);)*

                let mut fields = Vec::new();
                $(
                    let key = stringify!($field).to_string();
                    let type_name = map_type!($field_type).to_string();
                    let mut f_description = String::new();
                    $(
                        f_description += &format!("{}", $field_doc);
                    )*
                    fields.push(StructFieldDoc { key, description: f_description, type_name });
                )*
                structs.insert(name, StructDoc { description, fields });
            )*
        }
    }

    i_slint_common::for_each_builtin_structs!(gen_structs);

    // Internal type
    structs.remove("MenuEntry");

    if sc_only {
        structs.retain(|_, s| crate::element_docs::is_sc_covered(&s.description));
    }
    for s in structs.values_mut() {
        s.description = crate::element_docs::strip_sc(&s.description);
        for f in &mut s.fields {
            f.description = crate::element_docs::strip_sc(&f.description);
        }
    }

    structs
}

fn write_individual_struct_files(
    cfg: &Config,
    structs: std::collections::BTreeMap<String, StructDoc>,
) -> Result<(), Box<dyn std::error::Error>> {
    let structs_dir = cfg.reference_dir().join("structs");
    create_dir_all(&structs_dir).context(format!(
        "Failed to create folder holding individual structs doc files {structs_dir:?}"
    ))?;

    for (s, v) in &structs {
        let path = structs_dir.join(format!("_{s}.md"));
        let mut file = BufWriter::new(
            std::fs::File::create(&path).context(format!("error creating {path:?}"))?,
        );

        write!(
            file,
            r#"---
title: {0}
description: {0} content
---

<!-- Generated with slint-doc-generator from internal/common/builtin_structs.rs -->

`{0}`

{1}
"#,
            s, v.description
        )?;

        for f in &v.fields {
            writeln!(file, r#"- **`{}`** (_{}_): {}"#, f.key, f.type_name, f.description)?;
        }

        file.flush()?;
    }

    Ok(())
}

pub fn generate_builtin_struct_docs(cfg: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let structs = extract_builtin_structs(cfg.include_experimental, cfg.sc_only);
    write_individual_struct_files(cfg, structs)
}

/// Convert a ascii pascal case string to kebab case.
pub fn to_kebab_case(str: &str) -> String {
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

fn generate_keys_docs(cfg: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let enums_dir = cfg.reference_dir().join("enums");
    create_dir_all(&enums_dir).context(format!(
        "Failed to create folder holding individual enum doc files {enums_dir:?}"
    ))?;

    let path = enums_dir.join("_keys.md");
    let mut file =
        BufWriter::new(std::fs::File::create(&path).context(format!("error creating {path:?}"))?);

    writeln!(file, "---")?;
    writeln!(file, "title: keys")?;
    writeln!(file, "---")?;
    writeln!(file)?;

    macro_rules! collect_special_key {
        ($($char:literal # $name:ident # $($shifted:ident)? $(=> $($_muda:ident)? # $($qt:ident)|* # $($winit:ident $(($_pos:ident))?)|* # $($_xkb:ident)|*)?;)*) => {
            $(
                 write!(file, r#"-   **`{}`**
"#, stringify!($name)
                 )?;
            )*
        };
    }

    i_slint_common::for_each_keys!(collect_special_key);

    file.flush()?;

    Ok(())
}

#[test]
fn test_is_generated_dir() {
    let astro = std::path::Path::new("/repo/docs/astro");
    assert!(is_generated_dir(&astro.join("src/content/docs/reference/generated"), astro));
    // Hand-written content, an empty path, and a directory of another
    // project are all refused.
    assert!(!is_generated_dir(&astro.join("src/content/docs/reference"), astro));
    assert!(!is_generated_dir(std::path::Path::new(""), astro));
    assert!(!is_generated_dir(std::path::Path::new("/elsewhere/generated"), astro));
}
