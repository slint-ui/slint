// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore ename sname
// Generate mdx documentation files for builtin elements using data from
// the compiler's TypeRegister.

use i_slint_compiler::doc_comments::ElementDocEntry;
use i_slint_compiler::langtype::{
    BuiltinElement, BuiltinPropertyDefault, BuiltinPropertyInfo, ElementType, Type,
};
use i_slint_compiler::object_tree::PropertyVisibility;

use std::collections::HashSet;
use std::fmt::Write as FmtWrite;
use std::fs::create_dir_all;
use std::io::{BufWriter, Write};

use crate::Config;
use crate::mdx;

// -- SC annotation --

/// Find each standalone occurrence of `\sc` (not followed by an identifier
/// character so we don't collide with hypothetical markers like `\scope`).
fn find_sc_markers(doc: &str) -> impl Iterator<Item = (usize, usize)> + '_ {
    doc.match_indices("\\sc").filter_map(|(start, _)| {
        let end = start + 3;
        match doc.as_bytes().get(end).copied() {
            None => Some((start, end)),
            Some(b) if !b.is_ascii_alphanumeric() && b != b'_' => Some((start, end)),
            _ => None,
        }
    })
}

/// Whether a doc string carries the `\sc` marker, identifying content that's
/// part of the Slint SC safety-certified surface.
pub fn is_sc_covered(doc: &str) -> bool {
    find_sc_markers(doc).next().is_some()
}

/// Remove every `\sc` marker from a doc string so it never leaks into rendered output.
pub fn strip_sc(doc: &str) -> String {
    let ranges: Vec<(usize, usize)> = find_sc_markers(doc).collect();
    if ranges.is_empty() {
        return doc.to_string();
    }
    let mut out = String::with_capacity(doc.len());
    let mut cursor = 0;
    for (s, e) in ranges {
        out.push_str(&doc[cursor..s]);
        cursor = e;
    }
    out.push_str(&doc[cursor..]);
    out.trim_end().to_string()
}

// -- Annotation helpers --

/// Split at `\footer`, returning (description, footer).
fn split_footer(doc: &str) -> (String, String) {
    match doc.find("\\footer") {
        Some(pos) => (
            doc[..pos].trim_end().to_string(),
            doc[pos + "\\footer".len()..].trim_start_matches('\n').to_string(),
        ),
        None => (doc.to_string(), String::new()),
    }
}

/// Extract and remove `\default value` line from doc text.
fn extract_default(doc: &str) -> (String, Option<String>) {
    let mut lines: Vec<&str> = doc.lines().collect();
    for i in (0..lines.len()).rev() {
        if let Some(val) = lines[i].strip_prefix("\\default ") {
            lines.remove(i);
            return (lines.join("\n").trim_end().to_string(), Some(val.trim().to_string()));
        }
    }
    (doc.to_string(), None)
}

/// Extract and remove `\group:name` annotation.
/// Returns `Some(name)` when a group was specified, or `None` when absent.
fn extract_group(doc: &mut String) -> Option<String> {
    let mut result = None;
    let mut lines: Vec<&str> = doc.lines().collect();
    for i in (0..lines.len()).rev() {
        if let Some(val) = lines[i].strip_prefix("\\group:") {
            result = Some(val.trim().to_string());
            lines.remove(i);
        }
    }
    *doc = lines.join("\n").trim_end().to_string();
    result
}

/// Strip `\draft` and return whether it was present.
fn strip_annotation(doc: &mut String, tag: &str) -> bool {
    if doc.contains(tag) {
        *doc = doc.replace(tag, "").trim().to_string();
        true
    } else {
        false
    }
}

// -- Code fence screenshot transformation --

/// Attribute keys that are consumed by the screenshot system and removed
/// from the code fence info string. Everything else (e.g. `playground`)
/// stays on the fence.
const SCREENSHOT_ATTRS: &[&str] = &["imageAlt", "width", "height", "needsBackground", "scale"];

fn is_screenshot_attr(key: &str) -> bool {
    SCREENSHOT_ATTRS.contains(&key)
}

/// State tracker for auto-generating screenshot image paths.
struct ScreenshotCounter {
    element_slug: String,
    next: usize,
    /// When set, strip screenshot fence attributes instead of wrapping with
    /// `<CodeSnippetMD>`. Used by SC mode where no PNGs are generated.
    skip_screenshots: bool,
}

impl ScreenshotCounter {
    fn new(element_name: &str, skip_screenshots: bool) -> Self {
        Self { element_slug: mdx::to_kebab_case(element_name), next: 1, skip_screenshots }
    }

    fn path_for(&self, n: usize) -> String {
        format!("/src/assets/generated/{}-{n}.png", self.element_slug)
    }

    fn next_path(&mut self) -> String {
        let path = self.path_for(self.next);
        self.next += 1;
        path
    }
}

/// Parse `key="value"` or bare-flag attributes from a code fence info string.
/// Standalone quoted strings (e.g. `"color: red;"`) are preserved as bare flags
/// for Expressive Code text markers.
fn parse_fence_attrs(info: &str) -> Vec<(String, String)> {
    let mut attrs = Vec::new();
    let mut rest = info;
    while !rest.is_empty() {
        rest = rest.trim_start();
        if rest.is_empty() {
            break;
        }
        // Standalone quoted strings (Expressive Code text markers like "color: red;").
        if (rest.starts_with('"') || rest.starts_with('\''))
            && !rest[1..].starts_with(['=', '"', '\''])
        {
            let quote = rest.as_bytes()[0];
            let end =
                rest[1..].find(|c: char| c as u8 == quote).map(|i| i + 2).unwrap_or(rest.len());
            attrs.push((rest[..end].to_string(), String::new()));
            rest = &rest[end..];
            continue;
        }
        let key_end = rest.find(|c: char| c == '=' || c.is_whitespace()).unwrap_or(rest.len());
        let key = rest[..key_end].to_string();
        rest = &rest[key_end..];
        if rest.starts_with('=') {
            rest = &rest[1..];
            if rest.starts_with('"') {
                rest = &rest[1..];
                let end = rest.find('"').unwrap_or(rest.len());
                attrs.push((key, rest[..end].to_string()));
                rest = &rest[(end + 1).min(rest.len())..];
            } else {
                let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
                attrs.push((key, rest[..end].to_string()));
                rest = &rest[end..];
            }
        } else {
            attrs.push((key, String::new()));
        }
    }
    attrs
}

/// Transform code fences with screenshot attributes into `<CodeSnippetMD>` tags.
///
/// A fence like `` ```slint imageAlt="example" width="200" height="200" ``
/// becomes a `<CodeSnippetMD>` wrapper with an auto-generated `imagePath`.
/// When `counter.skip_screenshots` is true, screenshot attributes are stripped
/// instead and the fence is emitted as a plain ```slint``` block. Also strips
/// the `\sc` marker so it never reaches the rendered output.
#[allow(clippy::while_let_on_iterator)] // inner loop also advances `lines`
fn transform_code_fences(text: &str, counter: &mut ScreenshotCounter) -> String {
    let stripped = strip_sc(text);
    let text = stripped.as_str();
    let skip_screenshots = counter.skip_screenshots;
    let mut result = String::with_capacity(text.len());
    let mut lines = text.lines().peekable();
    while let Some(line) = lines.next() {
        let trimmed = line.trim_start();
        let backtick_count = trimmed.len() - trimmed.trim_start_matches('`').len();
        if backtick_count >= 3 {
            let after_backticks = &trimmed[backtick_count..];
            if after_backticks.starts_with("slint")
                && (after_backticks.len() == 5
                    || after_backticks[5..].starts_with(|c: char| c.is_whitespace()))
            {
                let info = after_backticks[5..].trim();
                let attrs = parse_fence_attrs(info);

                if attrs.iter().any(|(k, _)| is_screenshot_attr(k)) {
                    let indent = &line[..line.len() - trimmed.len()];
                    let backticks = &trimmed[..backtick_count];

                    let get =
                        |key: &str| attrs.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str());

                    // Keep non-screenshot attributes (e.g. `playground`) on the fence.
                    let fence_attrs: Vec<&str> = attrs
                        .iter()
                        .filter(|(k, _)| !is_screenshot_attr(k))
                        .map(|(k, _)| k.as_str())
                        .collect();

                    if !skip_screenshots {
                        let image_path = counter.next_path();

                        // Build <CodeSnippetMD ...> opening tag.
                        let mut tag = format!("<CodeSnippetMD imagePath=\"{image_path}\"");
                        if let Some(alt) = get("imageAlt") {
                            write!(tag, " imageAlt=\"{alt}\"").unwrap();
                        }
                        if let Some(w) = get("width") {
                            write!(tag, " imageWidth=\"{w}\"").unwrap();
                        }
                        if let Some(h) = get("height") {
                            write!(tag, " imageHeight=\"{h}\"").unwrap();
                        }
                        if get("needsBackground").is_some() {
                            tag.push_str(" needsBackground=\"true\"");
                        }
                        if let Some(s) = get("scale") {
                            write!(tag, " scale=\"{s}\"").unwrap();
                        }
                        tag.push('>');

                        result.push_str(indent);
                        result.push_str(&tag);
                        result.push('\n');
                    }

                    result.push_str(indent);
                    result.push_str(backticks);
                    result.push_str("slint");
                    if !fence_attrs.is_empty() {
                        write!(result, " {}", fence_attrs.join(" ")).unwrap();
                    }
                    result.push('\n');

                    // Copy code body until closing backticks.
                    for body_line in lines.by_ref() {
                        result.push_str(body_line);
                        result.push('\n');
                        let body_trimmed = body_line.trim_start();
                        if body_trimmed.starts_with(backticks)
                            && body_trimmed.len() == backtick_count
                        {
                            break;
                        }
                    }

                    if !skip_screenshots {
                        result.push_str(indent);
                        result.push_str("</CodeSnippetMD>\n");
                    }
                    continue;
                }
            }
        }
        result.push_str(line);
        result.push('\n');
    }
    // Remove trailing newline added by the loop.
    if result.ends_with('\n') && !text.ends_with('\n') {
        result.pop();
    }
    result
}

// -- Type formatting helpers --

/// Format a type name for documentation output. Same as `Type::Display`
/// except enumerations omit the `enum` prefix.
fn format_type_name(ty: &Type) -> String {
    match ty {
        Type::Enumeration(e) => e.name.to_string(),
        _ => ty.to_string(),
    }
}

/// Format a default value expression for documentation output.
fn format_default_expr(expr: &i_slint_compiler::expression_tree::Expression) -> String {
    use i_slint_compiler::expression_tree::Expression;
    match expr {
        Expression::EnumerationValue(v) => {
            v.enumeration.values.get(v.value).map(|s| s.to_string()).unwrap_or_default()
        }
        Expression::Cast { from, to } => {
            if matches!(to, Type::Color | Type::Brush)
                && let Expression::NumberLiteral(n, _) = from.as_ref()
            {
                let argb = *n as u32;
                return format!("#{:06x}", argb & 0xFFFFFF);
            }
            format_default_expr(from)
        }
        _ => {
            let mut s = String::new();
            i_slint_compiler::expression_tree::pretty_print(&mut s, expr).unwrap();
            s
        }
    }
}

/// Format a callback or function signature from a `Function` type.
fn format_signature(func: &i_slint_compiler::langtype::Function) -> String {
    let params: Vec<String> = func
        .arg_names
        .iter()
        .zip(func.args.iter())
        .filter(|(_, ty)| !matches!(ty, Type::ElementReference))
        .map(|(name, ty)| {
            if name.is_empty() {
                format_type_name(ty)
            } else {
                format!("{name}: {}", format_type_name(ty))
            }
        })
        .collect();
    let ret = if matches!(func.return_type, Type::Void) {
        String::new()
    } else {
        format!(" -> {}", format_type_name(&func.return_type))
    };
    format!("({}){ret}", params.join(", "))
}

/// MDX treats `{` as a JSX expression and `<` as a tag start in headings; use a code
/// heading when the signature would otherwise confuse the parser.
fn write_mdx_signature_heading(
    file: &mut impl Write,
    markdown_heading: &str,
    name: &str,
    func: &i_slint_compiler::langtype::Function,
) -> std::io::Result<()> {
    let sig = format_signature(func);
    let title = format!("{name}{sig}");
    if title.contains('{') || title.contains('<') {
        writeln!(file, "{markdown_heading} `{title}`")?;
    } else {
        writeln!(file, "{markdown_heading} {title}")?;
    }
    Ok(())
}

/// Convert a `PropertyVisibility` to the direction string used in docs.
fn visibility_to_direction(v: PropertyVisibility) -> &'static str {
    match v {
        PropertyVisibility::Input => "in",
        PropertyVisibility::Output => "out",
        PropertyVisibility::InOut => "in-out",
        _ => "",
    }
}

// -- Writing helpers --

/// Whether a builtin element has any documentation worth showing.
fn has_documentation(builtin: &BuiltinElement) -> bool {
    match builtin.docs.first() {
        Some(ElementDocEntry::Text(t)) if !t.is_empty() => true,
        _ => builtin.properties.values().any(|p| p.docs.is_some()),
    }
}

/// Extract and clean the element description from the first doc entry.
fn element_description(builtin: &BuiltinElement) -> String {
    match builtin.docs.first() {
        Some(ElementDocEntry::Text(t)) => {
            let mut d = t.clone();
            extract_group(&mut d);
            strip_annotation(&mut d, "\\draft");
            strip_annotation(&mut d, "\\skip_inherited");
            strip_annotation(&mut d, "\\skip_children");
            let (desc, _) = split_footer(&d);
            desc
        }
        _ => String::new(),
    }
}

/// Collect all text from a builtin element for import detection.
fn collect_builtin_text(builtin: &BuiltinElement, text: &mut String) {
    for entry in &builtin.docs {
        match entry {
            ElementDocEntry::Text(t) => {
                text.push(' ');
                text.push_str(t);
            }
            ElementDocEntry::Member(name) => {
                if let Some(info) = builtin.properties.get(name.as_str())
                    && let Some(doc) = &info.docs
                {
                    text.push(' ');
                    text.push_str(doc);
                }
            }
        }
    }
}

/// Collect all text from an element and its descendants for import detection.
fn collect_all_text(builtin: &BuiltinElement, skip_children: bool) -> String {
    let mut text = String::new();
    collect_builtin_text(builtin, &mut text);
    if !skip_children {
        let mut seen = HashSet::new();
        fn collect_children(
            parent: &BuiltinElement,
            text: &mut String,
            seen: &mut HashSet<String>,
        ) {
            for (name, child) in &parent.additional_accepted_child_types {
                if !seen.insert(name.to_string()) {
                    continue;
                }
                collect_builtin_text(child, text);
                collect_children(child, text, seen);
            }
        }
        collect_children(builtin, &mut text, &mut seen);
    }
    text
}

fn write_slint_property(
    file: &mut impl Write,
    name: &str,
    info: &BuiltinPropertyInfo,
    heading: &str,
    enums: &HashSet<String>,
    structs: &HashSet<String>,
    sc: &mut ScreenshotCounter,
) -> std::io::Result<()> {
    let type_name = format_type_name(&info.ty);
    let raw_doc = info.docs.as_deref().unwrap_or("");
    let (description, doc_default) = extract_default(raw_doc);
    let mut default_value = match &info.default_value {
        BuiltinPropertyDefault::Expr(expr) => format_default_expr(expr),
        _ => String::new(),
    };
    if default_value.is_empty()
        && let Some(d) = doc_default
    {
        default_value = d;
    }

    let (type_attr, is_enum, is_struct) = if enums.contains(&type_name) {
        ("enum", true, false)
    } else if structs.contains(&type_name) {
        ("struct", false, true)
    } else {
        (type_name.as_str(), false, false)
    };

    writeln!(file, "{heading} {name}")?;
    write!(file, "<SlintProperty propName=\"{name}\" typeName=\"{type_attr}\"")?;
    if is_enum {
        write!(file, " enumName=\"{type_name}\"")?;
    }
    if is_struct {
        write!(file, " structName=\"{type_name}\"")?;
    }
    let direction = visibility_to_direction(info.property_visibility);
    if direction == "out" || direction == "in-out" {
        write!(file, " propertyVisibility=\"{direction}\"")?;
    }
    if !default_value.is_empty() {
        write!(file, " defaultValue=\"{}\"", default_value.replace('"', "&quot;"))?;
    }
    if description.is_empty() {
        writeln!(file, "/>")?;
    } else {
        writeln!(file, ">")?;
        writeln!(file, "{}", transform_code_fences(&description, sc).trim_end())?;
        writeln!(file, "</SlintProperty>")?;
    }
    writeln!(file)?;
    Ok(())
}

/// Write a documented member (property, callback, or function) with auto-generated
/// section headings when entering a new kind for the first time.
fn write_member(
    file: &mut impl Write,
    name: &str,
    info: &BuiltinPropertyInfo,
    in_properties: &mut bool,
    in_callbacks: &mut bool,
    in_functions: &mut bool,
    enums: &HashSet<String>,
    structs: &HashSet<String>,
    sc: &mut ScreenshotCounter,
) -> std::io::Result<()> {
    match &info.ty {
        ty if ty.is_property_type() => {
            if !*in_properties && !*in_callbacks && !*in_functions {
                writeln!(file, "## Properties")?;
                writeln!(file)?;
                *in_properties = true;
            }
            write_slint_property(file, name, info, "###", enums, structs, sc)?;
        }
        Type::Callback(func) => {
            if !*in_callbacks {
                writeln!(file, "## Callbacks")?;
                writeln!(file)?;
                *in_callbacks = true;
            }
            write_mdx_signature_heading(file, "###", name, func)?;
            if let Some(doc) = &info.docs
                && !doc.is_empty()
            {
                writeln!(file, "{}", transform_code_fences(doc, sc).trim_end())?;
            }
            writeln!(file)?;
        }
        Type::Function(func) => {
            if !*in_functions {
                writeln!(file, "## Functions")?;
                writeln!(file)?;
                *in_functions = true;
            }
            write_mdx_signature_heading(file, "###", name, func)?;
            if let Some(doc) = &info.docs {
                writeln!(file, "{}", transform_code_fences(doc, sc).trim_end())?;
            }
            writeln!(file)?;
        }
        _ => {}
    }
    Ok(())
}

/// Normalize whitespace in `//!` section text: ensure each markdown heading
/// is preceded by a blank line, skip headings inside code fences, and
/// collapse runs of multiple blank lines into one.
fn normalize_section_text(text: &str) -> String {
    let mut result = String::with_capacity(text.len() + 32);
    let mut blank_run = 0u32;
    let mut in_code_fence = false;
    for line in text.lines() {
        if line.starts_with("```") || line.starts_with("~~~") {
            in_code_fence = !in_code_fence;
        }
        if line.trim().is_empty() {
            blank_run += 1;
            continue;
        }
        // Before a non-blank line, emit at most one blank line.
        if blank_run > 0 && !result.is_empty() {
            result.push('\n');
        }
        blank_run = 0;
        // Ensure a blank line before headings (outside code fences).
        if line.starts_with('#')
            && !in_code_fence
            && !result.is_empty()
            && !result.ends_with("\n\n")
        {
            result.push('\n');
        }
        result.push_str(line);
        result.push('\n');
    }
    if result.ends_with('\n') && !text.ends_with('\n') {
        result.pop();
    }
    result
}

/// Write members directly from a builtin element's doc entries.
/// Automatically inserts `## Properties` / `## Callbacks` / `## Functions`
/// headings unless a `//!` section header already provides them.
fn write_members(
    file: &mut impl Write,
    builtin: &BuiltinElement,
    enums: &HashSet<String>,
    structs: &HashSet<String>,
    sc: &mut ScreenshotCounter,
    cfg: &Config,
) -> std::io::Result<()> {
    let start = if builtin.docs.is_empty() { 0 } else { 1 };

    // If the first body entry is a text section, don't auto-generate "## Properties".
    let has_leading_section =
        builtin.docs.get(start).is_some_and(|e| matches!(e, ElementDocEntry::Text(_)));
    let mut in_properties = has_leading_section;
    let mut in_callbacks = false;
    let mut in_functions = false;
    for entry in &builtin.docs[start..] {
        match entry {
            ElementDocEntry::Text(text) => {
                // Free-form text sections aren't carrying a `\sc` marker —
                // they belong to the surrounding element-level prose. Suppress
                // them in SC mode so SC pages only show the explicitly covered
                // members.
                if cfg.sc_only {
                    continue;
                }
                let trimmed = text.trim_matches('\n');
                for line in trimmed.lines() {
                    if line.starts_with("## Properties") {
                        in_properties = true;
                    }
                    if line.starts_with("## Callbacks") {
                        in_callbacks = true;
                    }
                    if line.starts_with("## Functions") {
                        in_functions = true;
                    }
                }
                // Ensure blank lines before markdown headings so the
                // output matches the expected MDX spacing.
                let spaced = normalize_section_text(trimmed);
                writeln!(file, "{}", transform_code_fences(&spaced, sc))?;
                writeln!(file)?;
            }
            ElementDocEntry::Member(name) => {
                let Some(info) = builtin.properties.get(name.as_str()) else { continue };
                let Some(doc) = info.docs.as_deref() else { continue };
                if cfg.sc_only && !is_sc_covered(doc) {
                    continue;
                }
                write_member(
                    file,
                    name,
                    info,
                    &mut in_properties,
                    &mut in_callbacks,
                    &mut in_functions,
                    enums,
                    structs,
                    sc,
                )?;
            }
        }
    }

    Ok(())
}

/// Write a sub-element section. Recurse into the sub-element's own children.
#[allow(clippy::too_many_arguments)]
fn write_sub_element(
    file: &mut impl Write,
    child_name: &str,
    child: &BuiltinElement,
    enums: &HashSet<String>,
    structs: &HashSet<String>,
    seen: &mut HashSet<String>,
    sc: &mut ScreenshotCounter,
    cfg: &Config,
) -> std::io::Result<()> {
    if !seen.insert(child_name.to_string()) {
        return Ok(());
    }
    if !has_documentation(child) {
        return Ok(());
    }
    if cfg.sc_only
        && !child
            .docs
            .first()
            .is_some_and(|e| matches!(e, ElementDocEntry::Text(t) if is_sc_covered(t)))
    {
        return Ok(());
    }

    let skip_children = matches!(child.docs.first(), Some(ElementDocEntry::Text(t)) if t.contains("\\skip_children"));
    let description = element_description(child);

    writeln!(file, "## `{child_name}`")?;
    writeln!(file)?;
    if !description.is_empty() {
        writeln!(file, "{}", transform_code_fences(&description, sc).trim_end())?;
        writeln!(file)?;
    }

    // Collect documented members by kind, preserving doc entry order.
    let start = if child.docs.is_empty() { 0 } else { 1 };
    let mut props: Vec<(&str, &BuiltinPropertyInfo)> = Vec::new();
    let mut cbs: Vec<(&str, &BuiltinPropertyInfo)> = Vec::new();
    let mut fns: Vec<(&str, &BuiltinPropertyInfo)> = Vec::new();

    for entry in &child.docs[start..] {
        if let ElementDocEntry::Member(name) = entry
            && let Some(info) = child.properties.get(name.as_str())
        {
            let Some(doc) = info.docs.as_deref() else { continue };
            if cfg.sc_only && !is_sc_covered(doc) {
                continue;
            }
            match &info.ty {
                ty if ty.is_property_type() => props.push((name, info)),
                Type::Callback(_) => cbs.push((name, info)),
                Type::Function(_) => fns.push((name, info)),
                _ => {}
            }
        }
    }

    // Use grouped (####) headings when there are multiple member kinds.
    let kind_count =
        [!props.is_empty(), !cbs.is_empty(), !fns.is_empty()].iter().filter(|&&b| b).count();
    let grouped = kind_count > 1;
    let h = if grouped { "####" } else { "###" };

    if !props.is_empty() {
        if grouped {
            writeln!(file, "### Properties of `{child_name}`")?;
            writeln!(file)?;
        }
        for (name, info) in &props {
            write_slint_property(file, name, info, h, enums, structs, sc)?;
        }
    }
    if !cbs.is_empty() {
        writeln!(file, "### Callbacks of `{child_name}`")?;
        writeln!(file)?;
        for (name, info) in &cbs {
            let Type::Callback(func) = &info.ty else { continue };
            write_mdx_signature_heading(file, h, name, func)?;
            if let Some(doc) = &info.docs
                && !doc.is_empty()
            {
                writeln!(file, "{}", transform_code_fences(doc, sc).trim_end())?;
            }
            writeln!(file)?;
        }
    }
    if !fns.is_empty() {
        writeln!(file, "### Functions of `{child_name}`")?;
        writeln!(file)?;
        for (name, info) in &fns {
            let Type::Function(func) = &info.ty else { continue };
            write_mdx_signature_heading(file, h, name, func)?;
            if let Some(doc) = &info.docs {
                writeln!(file, "{}", transform_code_fences(doc, sc).trim_end())?;
            }
            writeln!(file)?;
        }
    }

    // Recurse into grandchildren.
    if !skip_children {
        for (gc_name, gc) in &child.additional_accepted_child_types {
            write_sub_element(file, gc_name, gc, enums, structs, seen, sc, cfg)?;
        }
    }

    Ok(())
}

/// Generate .mdx page files for each exported builtin element.
pub fn generate(cfg: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let register = i_slint_compiler::typeregister::TypeRegister::builtin_experimental(
        &i_slint_compiler::symbol_counters::SymbolCounters::shared(),
    );
    let register = register.borrow();
    let generated_dir = cfg.reference_dir();
    create_dir_all(&generated_dir)?;

    // Include all types for resolution, regardless of experimental or SC flag,
    // so property types still resolve their kind even when the target page
    // isn't generated.
    let enum_names: HashSet<String> = mdx::extract_enum_docs(true, false).keys().cloned().collect();
    let struct_names: HashSet<String> =
        mdx::extract_builtin_structs(true, false).keys().cloned().collect();

    // Collect exported elements.
    let mut elements = Vec::new();
    for (export_name, element_type) in register.all_elements() {
        match element_type {
            ElementType::Builtin(b) => {
                elements.push((export_name.to_string(), false, b));
            }
            ElementType::Component(c) if c.is_global() => {
                if let ElementType::Builtin(b) = &c.root_element.borrow().base_type {
                    elements.push((c.id.to_string(), true, b.clone()));
                }
            }
            _ => {}
        }
    }
    elements.sort_by(|a, b| a.0.cmp(&b.0));

    // Generate a page for each exported element with documentation.
    for (name, is_global, builtin) in &elements {
        let mut description = match builtin.docs.first() {
            Some(ElementDocEntry::Text(text)) => text.clone(),
            _ => continue,
        };
        if description.is_empty() {
            continue;
        }

        if cfg.sc_only && !is_sc_covered(&description) {
            continue;
        }

        // The SC reference is small, so it presents one flat list without
        // the group subdirectories used by the main docs site.
        let group = extract_group(&mut description).filter(|_| !cfg.sc_only);
        let draft = strip_annotation(&mut description, "\\draft");
        if draft {
            continue;
        }
        let (desc, footer) = split_footer(&description);
        description = desc;
        strip_annotation(&mut description, "\\skip_inherited");
        let skip_children = strip_annotation(&mut description, "\\skip_children");

        let filename = format!("{}.mdx", name.to_ascii_lowercase());
        let path = match group.as_deref() {
            Some(group) if !group.is_empty() => {
                let subdir = generated_dir.join(group);
                create_dir_all(&subdir)?;
                subdir.join(&filename)
            }
            _ => generated_dir.join(&filename),
        };
        let mut file = BufWriter::new(
            std::fs::File::create(&path).map_err(|e| format!("error creating {path:?}: {e}"))?,
        );

        // Frontmatter.
        let slug_stem = filename.strip_suffix(".mdx").unwrap();
        let slug = match group.as_deref() {
            Some(group) if !group.is_empty() => format!("reference/{group}/{slug_stem}"),
            _ => format!("reference/{slug_stem}"),
        };
        writeln!(file, "---")?;
        writeln!(file, "title: {name}")?;
        if *is_global {
            writeln!(file, "description: {name} Namespace")?;
        } else {
            writeln!(file, "description: {name} element api.")?;
        }
        writeln!(file, "slug: {slug}")?;
        writeln!(file, "---")?;

        // Imports.
        let all_text = collect_all_text(builtin, skip_children);
        writeln!(file)?;
        writeln!(
            file,
            "import SlintProperty from '@slint/common-files/src/components/SlintProperty.astro';"
        )?;
        if all_text.contains("CodeSnippetMD") || all_text.contains("imageAlt=") {
            writeln!(
                file,
                "import CodeSnippetMD from '@slint/common-files/src/components/CodeSnippetMD.astro';"
            )?;
        }
        let mut extra_imports = Vec::new();
        for sname in &struct_names {
            if all_text.contains(&format!("<{sname} />"))
                || all_text.contains(&format!("<{sname}/>"))
            {
                extra_imports.push(format!(
                    "import {sname} from '/src/{}/reference/structs/_{sname}.md';",
                    crate::GENERATED_DIR
                ));
            }
        }
        for ename in &enum_names {
            if all_text.contains(&format!("<{ename} />"))
                || all_text.contains(&format!("<{ename}/>"))
            {
                extra_imports.push(format!(
                    "import {ename} from '/src/{}/reference/enums/_{ename}.md';",
                    crate::GENERATED_DIR
                ));
            }
        }
        extra_imports.sort();
        for imp in &extra_imports {
            writeln!(file, "{imp}")?;
        }
        if all_text.contains("<Link ") {
            writeln!(file, "import Link from '@slint/common-files/src/components/Link.astro';")?;
        }
        if all_text.contains("<Tabs ") || all_text.contains("<TabItem ") {
            writeln!(file, "import {{ Tabs, TabItem }} from '@astrojs/starlight/components';")?;
        }
        writeln!(file)?;

        let mut sc = ScreenshotCounter::new(name, cfg.skip_screenshots);

        // Description.
        if !description.is_empty() {
            writeln!(file, "{}", transform_code_fences(&description, &mut sc).trim_end())?;
            writeln!(file)?;
        }

        // Members.
        write_members(&mut file, builtin, &enum_names, &struct_names, &mut sc, cfg)?;

        // Sub-elements (recursive, with cycle protection).
        if !skip_children {
            let mut seen_children = HashSet::new();
            for (child_name, child) in &builtin.additional_accepted_child_types {
                write_sub_element(
                    &mut file,
                    child_name,
                    child,
                    &enum_names,
                    &struct_names,
                    &mut seen_children,
                    &mut sc,
                    cfg,
                )?;
            }
        }

        // Footer.
        if !footer.is_empty() {
            writeln!(file, "{}", transform_code_fences(&footer, &mut sc).trim_end())?;
        }
        file.flush()?;
    }
    Ok(())
}
