// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

fn main() {
    napi_build::setup();

    // workaround bug that the `#[napi]` macro generate some invalid `#[cfg(feature="...")]`
    println!("cargo:rustc-check-cfg=cfg(feature,values(\"noop\", \"used_linker\"))");

    generate_language_module();
    generate_window_event_module();
}

/// Collect the public language types (`pub enum` enums and `pub struct` structs)
/// and emit `typescript/generated/language.ts`. Enums become `as const` maps from the Rust
/// variant identifier to the kebab-case string the Slint runtime accepts; structs become
/// TS type aliases. A type-only `namespace language { … }` declaration provides the named
/// types so users can write `let s: language.ColorScheme = language.ColorScheme.Dark;` or
/// type a callback parameter as `language.PointerEvent`.
fn generate_language_module() {
    struct EnumEntry {
        name: &'static str,
        docs: Vec<&'static str>,
        values: Vec<(&'static str, Vec<&'static str>)>,
    }
    let mut enums: Vec<EnumEntry> = Vec::new();
    macro_rules! collect_public_enums {
        ($(
            $(#[doc = $enum_doc:literal])*
            $(#[non_exhaustive])?
            $vis:vis enum $Name:ident {
                $( $(#[doc = $value_doc:literal])* $Value:ident, )*
            }
        )*) => {
            $(
                if stringify!($vis) == "pub" {
                    enums.push(EnumEntry {
                        name: stringify!($Name),
                        docs: vec![$($enum_doc),*],
                        values: vec![$( (stringify!($Value), vec![$($value_doc),*]) ),*],
                    });
                }
            )*
        };
    }
    i_slint_common::for_each_enums!(collect_public_enums);

    let mut structs: Vec<StructEntry> = Vec::new();
    macro_rules! collect_public_structs {
        ($(
            $(#[doc = $struct_doc:literal])*
            $(#[non_exhaustive])?
            $(#[derive(Copy, Eq)])?
            $vis:vis struct $Name:ident {
                $( $(#[doc = $field_doc:literal])* $field:ident : $field_type:ty $(= $field_default:expr)?, )*
            }
        )*) => {
            $(
                if stringify!($vis) == "pub" {
                    structs.push(StructEntry {
                        name: stringify!($Name),
                        docs: vec![$($struct_doc),*],
                        fields: vec![$(
                            (stringify!($field), stringify!($field_type), vec![$($field_doc),*],
                                i_slint_common::builtin_struct_field_default_tokens!($($field_default)?))
                        ),*],
                    });
                }
            )*
        };
    }
    i_slint_common::for_each_builtin_structs!(collect_public_structs);

    let mut in_language: HashSet<&'static str> = HashSet::new();
    for e in &enums {
        in_language.insert(e.name);
    }
    for s in &structs {
        in_language.insert(s.name);
    }
    let mut struct_names: HashSet<&'static str> = HashSet::new();
    for s in &structs {
        struct_names.insert(s.name);
    }
    // First variant of each public enum, as the kebab-case literal we'd write in JS. Mirrors
    // the Rust `Default for Enum` impl in `internal/core/items.rs` which always returns
    // the first variant.
    let mut enum_defaults: HashMap<&'static str, String> = HashMap::new();
    for e in &enums {
        if let Some((first, _)) = e.values.first() {
            enum_defaults.insert(e.name, to_kebab_case(first));
        }
    }

    let mut ts =
        generated_header(&["internal/common/enums.rs", "and internal/common/builtin_structs.rs"]);

    // DataTransfer is referenced by DropEvent's `data` field; the type lives at the
    // package top level rather than under `language`. Import it through the loader
    // (binding.cjs) so it resolves whichever native binary variant was built.
    ts.push_str("import { DataTransfer } from \"../../binding.cjs\";\n\n");

    ts.push_str("const _data = {\n");
    for entry in &enums {
        write_jsdoc(&mut ts, "    ", &entry.docs);
        ts.push_str(&format!("    {}: {{\n", entry.name));
        for (variant, value_docs) in &entry.values {
            write_jsdoc(&mut ts, "        ", value_docs);
            ts.push_str(&format!(
                "        {variant}: \"{kebab}\",\n",
                kebab = to_kebab_case(variant)
            ));
        }
        ts.push_str("    },\n");
    }
    for s in &structs {
        emit_struct_factory(&mut ts, s, &enum_defaults, &struct_names);
    }
    ts.push_str("} as const;\n\n");

    ts.push_str("/**\n");
    ts.push_str(" * Built-in enums and structs from the Slint language.\n");
    ts.push_str(
        " * Enum values are accessed via `language.ColorScheme.Dark`; struct values via the\n",
    );
    ts.push_str(
        " * factory call `language.PointerEvent({ button: … })`. Enum and struct types are\n",
    );
    ts.push_str(
        " * available in type position as `language.ColorScheme` / `language.PointerEvent`.\n",
    );
    ts.push_str(" */\n");
    ts.push_str("export const language = _data;\n\n");

    ts.push_str("/** Named types for the enum values in {@link language} and the built-in language structs. */\n");
    ts.push_str("// biome-ignore lint/style/useConst: declaration-merging namespace, type-only.\n");
    ts.push_str("export namespace language {\n");
    for entry in &enums {
        // The TS type alias is a computed union, so TypeDoc can't list "members" from it
        // the way it would for a TS `enum`. Append each variant (with its kebab-case value
        // and first-line description) to the JSDoc body so the type's documentation page
        // explains what the variants are.
        let mut docs: Vec<String> = entry.docs.iter().map(|s| s.to_string()).collect();
        if !entry.values.is_empty() {
            if !docs.is_empty() {
                docs.push(String::new());
            }
            docs.push(" Variants:".to_string());
            for (variant, value_docs) in &entry.values {
                let kebab = to_kebab_case(variant);
                let desc = value_docs
                    .iter()
                    .map(|s| s.trim())
                    .collect::<Vec<_>>()
                    .join(" ")
                    .trim()
                    .to_string();
                let sep = if desc.is_empty() { "" } else { " — " };
                docs.push(format!(
                    " - `language.{enum_name}.{variant}` (`\"{kebab}\"`){sep}{desc}",
                    enum_name = entry.name,
                ));
            }
        }
        write_jsdoc(&mut ts, "    ", &docs);
        ts.push_str(&format!(
            "    export type {name} = (typeof _data.{name})[keyof typeof _data.{name}];\n",
            name = entry.name
        ));
    }
    for s in &structs {
        write_jsdoc(&mut ts, "    ", &s.docs);
        ts.push_str(&format!("    export type {name} = {{\n", name = s.name));
        for (field, rust_ty, field_docs, declared_default) in &s.fields {
            let mut docs: Vec<String> = field_docs.iter().map(|s| s.to_string()).collect();
            if let Some(declared) = declared_default {
                docs.push(format!(
                    " Defaults to `{}`.",
                    field_default(rust_ty, Some(declared), &enum_defaults, &struct_names)
                ));
            }
            write_jsdoc(&mut ts, "        ", &docs);
            ts.push_str(&format!(
                "        {field}: {ts_ty};\n",
                ts_ty = map_field_type(rust_ty, &in_language),
            ));
        }
        ts.push_str("    };\n");
    }
    ts.push_str("}\n");

    write_if_changed(&generated_dir().join("language.ts"), &ts);
}

/// Map a Rust field type (as stringified by the macro) to the TS type used in the generated
/// language module. Fails the build with a clear message when a new struct adds an unmapped
/// type — better than silently emitting `unknown`.
fn map_field_type(rust_ty: &str, in_language: &HashSet<&'static str>) -> String {
    let t = rust_ty.trim();
    match t {
        "bool" => "boolean".to_string(),
        "i32" | "f32" | "f64" | "Coord" => "number".to_string(),
        "SharedString" => "string".to_string(),
        // Types exposed by the binding outside the `language` namespace.
        "DataTransfer" => "DataTransfer".to_string(),
        "LogicalPosition" => "{ x: number; y: number }".to_string(),
        ident if in_language.contains(ident) => ident.to_string(),
        other => panic!(
            "Unmapped struct field type `{other}` in for_each_builtin_structs!. \
             Extend `map_field_type` in api/node/build.rs."
        ),
    }
}

/// Compute the JS default expression for a struct field of the given Rust type.
/// A default value declared in builtin_structs.rs wins; otherwise primitives
/// fall back to zero/empty/false, enum fields use the first-variant kebab string (matching
/// the Rust `Default` impl), and nested struct fields recurse via `_data.<Name>()`.
fn field_default(
    rust_ty: &str,
    declared: Option<&str>,
    enum_defaults: &HashMap<&'static str, String>,
    structs: &HashSet<&'static str>,
) -> String {
    if let Some(declared) = declared {
        let text: String =
            declared.chars().filter(|c| !c.is_whitespace() && *c != ')' && *c != '(').collect();
        return match text.split_once("::") {
            // Enum values are kebab-case strings in the JS API
            Some((_, variant)) => {
                format!("\"{}\"", to_kebab_case(variant.trim_start_matches("r#")))
            }
            // bool and number literals are the same in JS
            None => text,
        };
    }
    let t = rust_ty.trim();
    match t {
        "bool" => "false".to_string(),
        "i32" | "f32" | "f64" | "Coord" => "0".to_string(),
        "SharedString" => "\"\"".to_string(),
        "DataTransfer" => "new DataTransfer()".to_string(),
        "LogicalPosition" => "{ x: 0, y: 0 }".to_string(),
        ident if enum_defaults.contains_key(ident) => format!("\"{}\"", enum_defaults[ident]),
        ident if structs.contains(ident) => format!("_data.{ident}()"),
        other => panic!(
            "Unmapped struct field type `{other}` for default-value computation. \
             Extend `field_default` in api/node/build.rs."
        ),
    }
}

/// Emit a `Foo: (props?: Partial<language.Foo>): language.Foo => Object.freeze({ … })` factory
/// inside `_data`. The factory exists so consumers can build values without specifying every
/// field, which gives us forward-compatibility (the Rust-side analogue of `#[non_exhaustive]`).
fn emit_struct_factory(
    out: &mut String,
    s: &StructEntry,
    enum_defaults: &HashMap<&'static str, String>,
    struct_names: &HashSet<&'static str>,
) {
    out.push_str("    /**\n");
    out.push_str(
        "     * Build a value of this struct. Any field you omit takes a documented default,\n",
    );
    out.push_str(
        "     * which lets Slint add fields later without breaking existing call-sites.\n",
    );
    if !s.docs.is_empty() {
        out.push_str("     *\n");
        for line in &s.docs {
            out.push_str("     *");
            out.push_str(line);
            out.push('\n');
        }
    }
    out.push_str("     */\n");
    out.push_str(&format!(
        "    {name}: (props?: Partial<language.{name}>): language.{name} => Object.freeze({{",
        name = s.name
    ));
    for (field, rust_ty, _, declared_default) in &s.fields {
        out.push_str(&format!(
            " {field}: {default},",
            default = field_default(rust_ty, *declared_default, enum_defaults, struct_names)
        ));
    }
    out.push_str(" ...props }),\n");
}

/// Mirror of `StructEntry` defined inside `generate_language_module`; declared at module
/// scope so helper fns can reference it.
struct StructEntry {
    name: &'static str,
    docs: Vec<&'static str>,
    /// (name, rust type, docs, declared default value tokens)
    fields: Vec<(&'static str, &'static str, Vec<&'static str>, Option<&'static str>)>,
}

/// Emit the rustdoc lines as a JSDoc block at the given indent. No-op if `docs` is empty.
/// Each Rust `///` line carries a leading space; we keep it (it's how the original prose is
/// formatted) and just wrap with `/**` … `*/`.
fn write_jsdoc<S: AsRef<str>>(out: &mut String, indent: &str, docs: &[S]) {
    if docs.is_empty() {
        return;
    }
    out.push_str(indent);
    out.push_str("/**\n");
    for line in docs {
        out.push_str(indent);
        out.push_str(" *");
        out.push_str(line.as_ref());
        out.push('\n');
    }
    out.push_str(indent);
    out.push_str(" */\n");
}

/// Convert `CamelCase` to `kebab-case`. Matches `i_slint_compiler::generator::to_kebab_case`
/// so generated values line up with `Enumeration::values` in the type register.
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

/// Emit `typescript/generated/window-event.ts`: one documented TS interface per
/// `WindowEvent` variant, plus the discriminated union of them.
///
/// The variants, fields and most of the prose come from `WindowEvent` in
/// `internal/core/platform.rs`; the JS spelling of each field and the JS-specific prose from
/// the `#[napi]` enum in `rust/types/window_event.rs`. Both are cross-checked, so a variant
/// added to the core enum fails this build until the binding catches up.
fn generate_window_event_module() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let core_path = manifest_dir.join("../../internal/core/platform.rs");
    let napi_path = manifest_dir.join("rust/types/window_event.rs");
    println!("cargo:rerun-if-changed={}", core_path.display());
    println!("cargo:rerun-if-changed={}", napi_path.display());

    let core = parse_rust_enum(&core_path, "WindowEvent");
    let napi = parse_rust_enum(&napi_path, "JsWindowEvent");

    // The generator spells out the `type` literals itself, so pin how napi spells them.
    assert_eq!(
        napi_arg(&napi.attrs, "js_name").as_deref(),
        Some("WindowEvent"),
        "JsWindowEvent must keep `js_name = \"WindowEvent\"`"
    );
    assert_eq!(
        napi_arg(&napi.attrs, "discriminant_case").as_deref(),
        Some("kebab-case"),
        "JsWindowEvent must keep `discriminant_case = \"kebab-case\"`"
    );

    let core_variants: Vec<&RustVariant> = core.variants.iter().filter(|v| !v.hidden).collect();
    let napi_names: Vec<&str> = napi.variants.iter().map(|v| v.name.as_str()).collect();
    let core_names: Vec<&str> = core_variants.iter().map(|v| v.name.as_str()).collect();
    assert_eq!(
        core_names, napi_names,
        "JsWindowEvent in api/node/rust/types/window_event.rs is out of sync with \
         WindowEvent in internal/core/platform.rs"
    );

    let mut ts = generated_header(&[
        "the WindowEvent enum in internal/core/platform.rs",
        "and api/node/rust/types/window_event.rs",
    ]);
    // `Window` is referenced from the `@see` links, which TypeDoc only resolves for types
    // the module imports.
    ts.push_str("import type { Point, Size, Window } from \"../index.ts\";\n");
    ts.push_str("import type { language } from \"./language\";\n\n");

    for (core_variant, napi_variant) in core_variants.iter().zip(&napi.variants) {
        let mut docs = jsdoc_for(core_variant.docs.as_slice(), napi_variant.docs.as_slice());
        assert!(
            !docs.is_empty(),
            "WindowEvent::{} needs a doc comment, in the core enum or in JsWindowEvent",
            core_variant.name
        );
        docs.push(String::new());
        docs.push(" @see {@link Window.dispatchEvent}".into());
        write_jsdoc(&mut ts, "", &docs);
        ts.push_str(&format!("export interface {}Event {{\n", core_variant.name));
        ts.push_str(&format!("    type: \"{}\";\n", to_kebab_case(&core_variant.name)));

        assert_eq!(
            core_variant.fields.len(),
            napi_variant.fields.len(),
            "WindowEvent::{} and JsWindowEvent::{} have different fields",
            core_variant.name,
            napi_variant.name
        );
        for (core_field, napi_field) in core_variant.fields.iter().zip(&napi_variant.fields) {
            // A tuple variant has no field name in the core enum, so the binding names it.
            let js_name =
                napi_field.js_name.clone().or_else(|| napi_field.name.clone()).unwrap_or_else(
                    || panic!("JsWindowEvent::{} has an unnamed field", napi_variant.name),
                );
            if let Some(core_name) = &core_field.name {
                assert_eq!(
                    core_name.as_str(),
                    napi_field.name.as_deref().unwrap_or_default(),
                    "field of WindowEvent::{} renamed in JsWindowEvent",
                    core_variant.name
                );
                assert_eq!(
                    to_camel_case(core_name),
                    js_name,
                    "JsWindowEvent::{}::{core_name} needs `#[napi(js_name = \"{}\")]`",
                    napi_variant.name,
                    to_camel_case(core_name)
                );
            }
            let (ts_type, expected_napi_type) = map_event_field_type(&core_field.ty);
            assert_eq!(
                expected_napi_type, napi_field.ty,
                "JsWindowEvent::{}::{js_name} should be a {expected_napi_type} to match \
                 {} in the core enum",
                napi_variant.name, core_field.ty
            );
            let docs = jsdoc_for(core_field.docs.as_slice(), napi_field.docs.as_slice());
            assert!(
                !docs.is_empty(),
                "WindowEvent::{}::{js_name} needs a doc comment, in the core enum or in \
                 JsWindowEvent",
                core_variant.name
            );
            write_jsdoc(&mut ts, "    ", &docs);
            ts.push_str(&format!("    {js_name}: {ts_type};\n"));
        }
        ts.push_str("}\n\n");
    }

    write_jsdoc(&mut ts, "", &jsdoc_for(core.docs.as_slice(), napi.docs.as_slice()));
    let union = core_names.iter().map(|name| format!("    | {name}Event")).collect::<Vec<_>>();
    ts.push_str(&format!("export type WindowEvent =\n{};\n", union.join("\n")));

    write_if_changed(&generated_dir().join("window-event.ts"), &ts);
}

/// Map a `WindowEvent` field type to the TS type used in the generated interface, and to the
/// type the matching `JsWindowEvent` field must have.
fn map_event_field_type(rust_ty: &str) -> (&'static str, &'static str) {
    match rust_ty {
        "LogicalPosition" => ("Point", "SlintPoint"),
        "LogicalSize" => ("Size", "SlintSize"),
        "PointerEventButton" => ("language.PointerEventButton", "JsPointerEventButton"),
        "SharedString" => ("string", "String"),
        "f32" => ("number", "f64"),
        "bool" => ("boolean", "bool"),
        other => panic!(
            "Unmapped WindowEvent field type `{other}`. Extend `map_event_field_type` in \
             api/node/build.rs, and import the TS type it maps to in the generated module."
        ),
    }
}

/// The binding's own doc comment wins, because it's already written for JavaScript.
/// Otherwise the core enum's rustdoc is translated.
fn jsdoc_for(core_docs: &[String], napi_docs: &[String]) -> Vec<String> {
    if !napi_docs.is_empty() {
        return napi_docs.to_vec();
    }
    rustdoc_to_jsdoc(core_docs)
}

/// Turn rustdoc into JSDoc. Intra-doc links become their label, since the targets are Rust
/// paths, and everything from the first heading or fence on is dropped: that only ever
/// introduces a Rust example.
fn rustdoc_to_jsdoc(docs: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in docs {
        let trimmed = line.trim_start();
        if trimmed.starts_with('#') || trimmed.starts_with("```") {
            break;
        }
        out.push(unwrap_intra_doc_links(line));
    }
    while out.last().is_some_and(|last| last.trim().is_empty()) {
        out.pop();
    }
    out
}

/// Replace `[label](target)` and `[label]` with `label` unless the target is a URL.
fn unwrap_intra_doc_links(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(open) = rest.find('[') {
        let Some(close) = rest[open..].find(']').map(|i| open + i) else { break };
        let label = &rest[open + 1..close];
        let after = &rest[close + 1..];
        let (target, tail) = match after.strip_prefix('(') {
            Some(paren) => match paren.find(')') {
                Some(end) => (Some(&paren[..end]), &paren[end + 1..]),
                None => break,
            },
            None => (None, after),
        };
        if target.is_some_and(|t| t.contains("://")) {
            out.push_str(&rest[..close + 1]);
            rest = after;
            continue;
        }
        out.push_str(&rest[..open]);
        out.push_str(label);
        rest = tail;
    }
    out.push_str(rest);
    out
}

/// A `pub enum` parsed out of a Rust source file.
struct RustEnum {
    docs: Vec<String>,
    attrs: Vec<syn::Attribute>,
    variants: Vec<RustVariant>,
}

struct RustVariant {
    name: String,
    docs: Vec<String>,
    /// Whether the variant is `#[doc(hidden)]`.
    hidden: bool,
    fields: Vec<RustField>,
}

struct RustField {
    /// `None` for a field of a tuple variant.
    name: Option<String>,
    js_name: Option<String>,
    /// Last segment of the field's type path.
    ty: String,
    docs: Vec<String>,
}

fn parse_rust_enum(path: &std::path::Path, name: &str) -> RustEnum {
    let source = std::fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("cannot read {}: {err}", path.display()));
    let file = syn::parse_file(&source)
        .unwrap_or_else(|err| panic!("cannot parse {}: {err}", path.display()));
    let item = file
        .items
        .iter()
        .find_map(|item| match item {
            syn::Item::Enum(item) if item.ident == name => Some(item),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no `enum {name}` in {}", path.display()));

    RustEnum {
        docs: doc_lines(&item.attrs),
        attrs: item.attrs.clone(),
        variants: item
            .variants
            .iter()
            .map(|variant| RustVariant {
                name: variant.ident.to_string(),
                docs: doc_lines(&variant.attrs),
                hidden: is_doc_hidden(&variant.attrs),
                fields: variant
                    .fields
                    .iter()
                    .map(|field| RustField {
                        name: field.ident.as_ref().map(|ident| ident.to_string()),
                        js_name: napi_arg(&field.attrs, "js_name"),
                        ty: type_name(&field.ty),
                        docs: doc_lines(&field.attrs),
                    })
                    .collect(),
            })
            .collect(),
    }
}

/// The text of the `///` lines, each still carrying rustdoc's leading space.
fn doc_lines(attrs: &[syn::Attribute]) -> Vec<String> {
    attrs
        .iter()
        .filter_map(|attr| match &attr.meta {
            syn::Meta::NameValue(meta) if meta.path.is_ident("doc") => match &meta.value {
                syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(text), .. }) => Some(text.value()),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

fn is_doc_hidden(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        attr.path().is_ident("doc")
            && matches!(&attr.meta, syn::Meta::List(list) if list.tokens.to_string() == "hidden")
    })
}

/// The value of one `key = "value"` argument of a `#[napi(…)]` attribute.
fn napi_arg(attrs: &[syn::Attribute], key: &str) -> Option<String> {
    let mut found = None;
    for attr in attrs.iter().filter(|attr| attr.path().is_ident("napi")) {
        let _ = attr.parse_nested_meta(|meta| {
            // Every `key = "value"` has to be consumed, matching or not: the parser stops
            // at the first one left unread.
            let is_match = meta.path.is_ident(key);
            if let Ok(value) = meta.value()
                && let Ok(text) = value.parse::<syn::LitStr>()
                && is_match
            {
                found = Some(text.value());
            }
            Ok(())
        });
    }
    found
}

/// The last segment of a type path, which is all the type maps need.
fn type_name(ty: &syn::Type) -> String {
    match ty {
        syn::Type::Path(path) => {
            path.path.segments.last().expect("empty type path").ident.to_string()
        }
        _ => panic!("unsupported field type; only path types are handled"),
    }
}

/// Convert `snake_case` to `camelCase`, the way `#[napi(js_name = "…")]` spells fields.
fn to_camel_case(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut capitalize = false;
    for c in s.chars() {
        if c == '_' {
            capitalize = true;
        } else if capitalize {
            out.extend(c.to_uppercase());
            capitalize = false;
        } else {
            out.push(c);
        }
    }
    out
}

fn generated_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("typescript").join("generated")
}

/// The banner every generated module starts with. These files are gitignored, so REUSE
/// never sees them and they need no license header.
fn generated_header(sources: &[&str]) -> String {
    format!(
        "// AUTO-GENERATED by api/node/build.rs from {}. Do not edit.\n\n",
        sources.join("\n// ")
    )
}

/// Write `contents` only if it differs, so we don't invalidate downstream builds, and watch
/// the result so it regenerates when edited or removed by hand.
fn write_if_changed(path: &std::path::Path, contents: &str) {
    println!("cargo:rerun-if-changed={}", path.display());
    let dir = path.parent().expect("generated file has a parent directory");
    std::fs::create_dir_all(dir).unwrap_or_else(|err| panic!("create {}: {err}", dir.display()));
    let needs_write =
        std::fs::read_to_string(path).map(|existing| existing != contents).unwrap_or(true);
    if needs_write {
        std::fs::write(path, contents)
            .unwrap_or_else(|err| panic!("write {}: {err}", path.display()));
    }
}
