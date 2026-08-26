// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cspell:ignore capitalizationmode keyboardmodifiers keyevent

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

/// Generate all markdown/mdx documentation files, and return the gaps the
/// safety manual shows: the runtime files that aren't completely covered and
/// the requirement paragraphs that no test declares. Every page is written
/// first, so a build can publish the manual and act on the gaps afterwards.
pub fn generate(cfg: &Config) -> Result<Vec<String>, Box<dyn std::error::Error>> {
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
    // The struct and element pages link to the type pages, so all of them are
    // written from the same maps: a type this run leaves out is never linked to.
    let enums = extract_enum_docs(cfg.include_experimental, cfg.sc_only);
    let structs = extract_builtin_structs(cfg.include_experimental, cfg.sc_only);
    // The pages of this site can also link the hand-written property-types
    // pages, unless it's the safety manual: it serves an SC-filtered copy of
    // those, which drops the sections of the types it doesn't cover. The struct
    // partials are inlined into both sites, so they never link them.
    let site_links = TypeLinks::new(&enums, &structs, !cfg.sc_only);
    let shared_links = TypeLinks::new(&enums, &structs, false);
    write_individual_enum_files(cfg, &enums)?;
    write_individual_struct_files(cfg, &structs, &shared_links)?;
    if !cfg.sc_only {
        generate_keys_docs(cfg)?;
    }
    crate::element_docs::generate(cfg, &site_links)?;

    if !cfg.sc_only || !enums.is_empty() || !structs.is_empty() {
        write_builtin_structs_and_enums(cfg, &structs, &enums)?;
    }

    let mut gaps = Vec::new();
    if cfg.sc_only {
        gaps = crate::traceability::generate(cfg)?;
        gaps.extend(crate::coverage::generate(cfg)?);
        crate::test_results::generate(cfg)?;
    }

    Ok(gaps)
}

/// The pages listing every built-in struct and enum, linked to from the field
/// documentation of the structs.
const BUILTIN_STRUCTS_SLUG: &str = "reference/property-types/builtin-structs";
const BUILTIN_ENUMS_SLUG: &str = "reference/property-types/builtin-enums";

/// An enum documented on its own type page rather than in the Builtin Enums list.
fn enum_documented_elsewhere(name: &str) -> bool {
    // `keys.md` is generated separately and documented elsewhere; the mouse
    // cursor shapes are documented on the MouseCursor type.
    name == "keys" || name == "BuiltInMouseCursor"
}

fn write_builtin_structs_and_enums(
    cfg: &Config,
    structs: &std::collections::BTreeMap<String, StructDoc>,
    enums: &std::collections::BTreeMap<String, EnumDoc>,
) -> Result<(), Box<dyn std::error::Error>> {
    let generated_dir = cfg.reference_dir();
    create_dir_all(&generated_dir)?;

    let structs_path = generated_dir.join("builtin-structs.mdx");
    let mut file = BufWriter::new(
        std::fs::File::create(&structs_path).context(format!("error creating {structs_path:?}"))?,
    );
    writeln!(
        file,
        r#"---
title: Built-in Structs
description: The built-in struct types provided by Slint.
slug: {BUILTIN_STRUCTS_SLUG}
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
    writeln!(file)?;
    for name in structs.keys() {
        writeln!(file, "## {name}")?;
        writeln!(file, "<{name} />")?;
        writeln!(file)?;
    }
    file.flush()?;

    let enums_path = generated_dir.join("builtin-enums.mdx");
    let mut file = BufWriter::new(
        std::fs::File::create(&enums_path).context(format!("error creating {enums_path:?}"))?,
    );
    writeln!(
        file,
        r#"---
title: Built-in Enums
description: The built-in enumeration types provided by Slint.
slug: {BUILTIN_ENUMS_SLUG}
---
"#
    )?;
    for name in enums.keys() {
        if enum_documented_elsewhere(name) {
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
    for name in enums.keys() {
        if enum_documented_elsewhere(name) {
            continue;
        }
        writeln!(file, "## {name}")?;
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
            // A struct field default links to the value it takes, and every enum
            // shares the Builtin Enums page, so the id carries the enum name.
            writeln!(
                file,
                r#"* **<span id="{}">`{}`</span>**: {}"#,
                enum_value_anchor(k, &v.key),
                v.key,
                v.description
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

pub fn extract_enum_docs(
    _include_experimental: bool,
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

    if sc_only {
        enums.retain(|_, e| crate::element_docs::is_sc_covered(&e.description));
    }
    for e in enums.values_mut() {
        e.description = crate::element_docs::strip_sc(&e.description);
        for v in &mut e.values {
            v.description = crate::element_docs::strip_sc(&v.description);
        }
    }

    enums
}

pub struct StructFieldDoc {
    key: String,
    description: String,
    type_name: String,
    /// The value the field takes when it isn't set, written in `.slint` syntax,
    /// or `None` for the zero value of the field's type.
    default_value: Option<String>,
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
                        default_value: None,
                    },
                    StructFieldDoc {
                        key: "y".to_string(),
                        description: String::new(),
                        type_name: "length".to_string(),
                        default_value: None,
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
                        default_value: None,
                    },
                    StructFieldDoc {
                        key: "height".to_string(),
                        description: String::new(),
                        type_name: "length".to_string(),
                        default_value: None,
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
                    let key = stringify!($field).replace('_', "-");
                    let type_name = map_type!($field_type).to_string();
                    let mut f_description = String::new();
                    $(
                        f_description += &format!("{}", $field_doc);
                    )*
                    let default_value =
                        i_slint_common::builtin_struct_field_default_tokens!($($field_default)?)
                            .map(declared_default);
                    fields.push(StructFieldDoc { key, description: f_description, type_name, default_value });
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

/// The documentation of the built-in types, for linking a type named in a
/// struct field or a callback signature to the section that describes it.
pub struct TypeLinks {
    /// The enums and structs this run writes a section for. A type left out of
    /// it gets no link rather than one to a missing anchor.
    enums: std::collections::BTreeSet<String>,
    structs: std::collections::BTreeSet<String>,
    /// Whether the primitive types link to the hand-written property-types
    /// pages as well.
    primitives: bool,
}

impl TypeLinks {
    fn new(
        enums: &std::collections::BTreeMap<String, EnumDoc>,
        structs: &std::collections::BTreeMap<String, StructDoc>,
        primitives: bool,
    ) -> Self {
        Self {
            enums: enums.keys().filter(|name| !enum_documented_elsewhere(name)).cloned().collect(),
            structs: structs.keys().cloned().collect(),
            primitives,
        }
    }

    /// The section documenting `type_name`, or `None` for a type this run
    /// leaves out. The link is written from the site root; `remarkBaseLinks`
    /// adds the base of whichever site renders it.
    ///
    /// The generated pages come first: `link-data.json` keys pages of every
    /// kind, and some of them under the name of a type it documents elsewhere
    /// (`KeyEvent` points at the keyboard input overview).
    fn href(&self, type_name: &str) -> Option<String> {
        let page = if self.enums.contains(type_name) {
            BUILTIN_ENUMS_SLUG
        } else if self.structs.contains(type_name) {
            BUILTIN_STRUCTS_SLUG
        } else {
            return self.primitives.then(|| primitive_href(type_name)).flatten();
        };
        Some(format!("/{page}/#{}", type_name.to_lowercase()))
    }

    /// The documentation of `value` among the values `type_name` can take, or
    /// `None` for an enum this run leaves out.
    fn enum_value_href(&self, type_name: &str, value: &str) -> Option<String> {
        self.enums
            .contains(type_name)
            .then(|| format!("/{BUILTIN_ENUMS_SLUG}/#{}", enum_value_anchor(type_name, value)))
    }

    /// `type_name` as markdown, linked to its documentation when it has any.
    pub fn linked(&self, type_name: &str) -> String {
        self.href(type_name)
            .map_or_else(|| type_name.to_string(), |href| format!("[{type_name}]({href})"))
    }
}

/// The section documenting a primitive type, from `link-data.json`: the map
/// the docs site resolves its own `slint:` links with, so these links follow a
/// page that moves. It keys the types by their name in `.slint`.
fn primitive_href(type_name: &str) -> Option<String> {
    static LINK_MAP: std::sync::LazyLock<serde_json::Value> = std::sync::LazyLock::new(|| {
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../internal/core-macros/link-data.json"
        )))
        .expect("failed to parse link-data.json")
    });
    let href = LINK_MAP.get(type_name)?["href"]
        .as_str()
        .unwrap_or_else(|| panic!("link-data.json has no href for {type_name}"));
    Some(format!("/{href}"))
}

/// The id of one value on the page documenting `enum_name`.
fn enum_value_anchor(enum_name: &str, value: &str) -> String {
    format!("{}-{value}", enum_name.to_lowercase())
}

/// The `.slint` form of a field default declared in builtin_structs.rs.
fn declared_default(tokens: &str) -> String {
    let text: String =
        tokens.chars().filter(|c| !c.is_whitespace() && *c != '(' && *c != ')').collect();
    match text.split_once("::") {
        // Enum values are written in kebab case in .slint
        Some((_, variant)) => to_kebab_case(variant.trim_start_matches("r#")),
        // bool and number literals are written the same way
        None => text,
    }
}

/// The list entry documenting one field of a struct, with the field's type
/// linked to its own documentation when it has any.
fn struct_field_line(field: &StructFieldDoc, links: &TypeLinks) -> String {
    let name = &field.type_name;
    let type_name =
        links.href(name).map_or_else(|| format!("_{name}_"), |href| format!("[_{name}_]({href})"));
    let default_value = field.default_value.as_ref().map_or_else(String::new, |value| {
        // An enum value links to its own documentation, next to the values it could
        // take instead.
        links.enum_value_href(name, value).map_or_else(
            || format!(" Defaults to `{value}`."),
            |href| format!(" Defaults to [`{value}`]({href})."),
        )
    });
    format!("- **`{}`** ({}): {}{}", field.key, type_name, field.description, default_value)
}

fn write_individual_struct_files(
    cfg: &Config,
    structs: &std::collections::BTreeMap<String, StructDoc>,
    links: &TypeLinks,
) -> Result<(), Box<dyn std::error::Error>> {
    let structs_dir = cfg.reference_dir().join("structs");
    create_dir_all(&structs_dir).context(format!(
        "Failed to create folder holding individual structs doc files {structs_dir:?}"
    ))?;

    for (s, v) in structs {
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
            writeln!(file, "{}", struct_field_line(f, links))?;
        }

        file.flush()?;
    }

    Ok(())
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

#[test]
fn test_type_links() {
    let make_links = |primitives| TypeLinks {
        enums: ["CapitalizationMode".to_string()].into(),
        structs: ["KeyboardModifiers".to_string(), "KeyEvent".to_string()].into(),
        primitives,
    };
    let links = make_links(true);
    let href = |type_name| links.href(type_name);

    assert_eq!(
        href("CapitalizationMode").as_deref(),
        Some("/reference/property-types/builtin-enums/#capitalizationmode")
    );
    assert_eq!(
        href("KeyboardModifiers").as_deref(),
        Some("/reference/property-types/builtin-structs/#keyboardmodifiers")
    );
    // The primitive types are documented on the hand-written pages, the mouse
    // cursor shapes among them.
    assert_eq!(href("int").as_deref(), Some("/reference/property-types/numeric-types/#int"));
    assert_eq!(
        href("BuiltInMouseCursor").as_deref(),
        Some("/reference/property-types/other-types/#mousecursor")
    );
    // A key spelled otherwise than the type would drop the link, unnoticed.
    for type_name in [
        "data-transfer",
        "enum",
        "image",
        "physical-length",
        "relative-font-size",
        "string",
        "struct",
        "styled-text",
    ] {
        assert!(href(type_name).is_some(), "link-data.json has no entry for {type_name}");
    }
    // The generated section wins over the map's own `KeyEvent` entry.
    assert_eq!(
        href("KeyEvent").as_deref(),
        Some("/reference/property-types/builtin-structs/#keyevent")
    );

    // Content shared with the safety manual links the generated pages only.
    let shared_links = make_links(false);
    assert_eq!(shared_links.href("int"), None);
    assert_eq!(
        shared_links.href("CapitalizationMode").as_deref(),
        Some("/reference/property-types/builtin-enums/#capitalizationmode")
    );

    assert_eq!(
        links.linked("element ref"),
        "element ref",
        "a type with no documentation of its own is written plain"
    );
    assert_eq!(links.linked("int"), "[int](/reference/property-types/numeric-types/#int)");

    let field = StructFieldDoc {
        key: "field".to_string(),
        description: "The docs".to_string(),
        type_name: "CapitalizationMode".to_string(),
        default_value: None,
    };
    assert_eq!(
        struct_field_line(&field, &links),
        "- **`field`** ([_CapitalizationMode_](/reference/property-types/builtin-enums/#capitalizationmode)): The docs"
    );
    // A declared default value is documented after the description.
    let with_default = StructFieldDoc {
        key: "field".to_string(),
        description: "The docs".to_string(),
        type_name: "CapitalizationMode".to_string(),
        default_value: Some(declared_default("(CapitalizationMode::Sentences)")),
    };
    assert_eq!(
        struct_field_line(&with_default, &links),
        "- **`field`** ([_CapitalizationMode_](/reference/property-types/builtin-enums/#capitalizationmode)): The docs Defaults to [`sentences`](/reference/property-types/builtin-enums/#capitalizationmode-sentences)."
    );
    let field = StructFieldDoc { type_name: "element ref".to_string(), ..field };
    assert_eq!(struct_field_line(&field, &links), "- **`field`** (_element ref_): The docs");
}
