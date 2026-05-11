// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// Extract documentation from builtins.slint and generate mdx files.

use i_slint_compiler::parser::{self, SyntaxKind, identifier_text, syntax_nodes};

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Write as FmtWrite;
use std::fs::create_dir_all;
use std::io::{BufWriter, Write};

use crate::mdx;

#[derive(Clone, Debug, PartialEq)]
enum MemberKind {
    Property,
    Callback,
    Function,
    /// Raw text from `//!` comments, inserted between members.
    SectionHeader,
}

#[derive(Clone, Debug)]
struct MemberDoc {
    name: String,
    kind: MemberKind,
    /// For properties: the type name. For callbacks/functions: the signature.
    type_name: String,
    description: String,
    /// "in", "out", "in-out", or "" (only for properties).
    direction: String,
    default_value: String,
    /// True when the member had a `///` doc comment. Members without
    /// doc comments are internal and should not appear in the output.
    has_doc_comment: bool,
}

struct ElementDoc {
    name: String,
    is_global: bool,
    description: String,
    footer: String,
    skip_inherited: bool,
    /// When true, sub-elements are not documented on this page (they
    /// belong to another element's page instead).
    skip_children: bool,
    /// Marks experimental elements. The Astro content config excludes
    /// draft pages when experimental features are disabled.
    /// Extracted from `\draft` annotation.
    draft: bool,
    /// Subdirectory within the generated output folder, e.g. "elements".
    /// Extracted from `\group` annotation. `None` means no page is generated.
    group: Option<String>,
    members: Vec<MemberDoc>,
    /// Direct child component names (sub-elements).
    children: Vec<String>,
}

// -- Doc comment helpers --

/// Walk backwards through sibling tokens collecting consecutive `///` lines.
fn collect_doc_comments_before(node: &parser::SyntaxNode) -> Option<String> {
    let mut lines = Vec::new();
    let mut iter = node.node.prev_sibling_or_token();

    while let Some(ref cur) = iter {
        match cur.kind() {
            SyntaxKind::Whitespace => {}
            SyntaxKind::Comment => {
                let text = cur.as_token().unwrap().text().to_string();
                if text.starts_with("///") {
                    lines.push(text);
                } else if text.starts_with("//") {
                    // Skip regular comments and //-annotations.
                } else {
                    break;
                }
            }
            SyntaxKind::ExportsList => {
                // Doc comments may sit inside a preceding ExportsList.
                if let Some(node) = cur.as_node() {
                    let mut last = node.last_child_or_token();
                    while let Some(ref child) = last {
                        match child.kind() {
                            SyntaxKind::Whitespace => {}
                            SyntaxKind::Comment => {
                                let t = child.as_token().unwrap().text().to_string();
                                if t.starts_with("///") {
                                    lines.push(t);
                                } else if t.starts_with("//") {
                                    // skip
                                } else {
                                    break;
                                }
                            }
                            _ => break,
                        }
                        last = child.prev_sibling_or_token();
                    }
                }
                break;
            }
            _ => break,
        }
        iter = cur.prev_sibling_or_token();
    }

    if lines.is_empty() {
        return None;
    }
    lines.reverse();
    Some(
        lines
            .iter()
            .map(|t| t.strip_prefix("/// ").or_else(|| t.strip_prefix("///")).unwrap_or(""))
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

/// Extract `///` doc comment before a syntax node.
/// Also checks before an ExportsList parent if the node is inside one.
fn extract_doc_comment(node: &parser::SyntaxNode) -> Option<String> {
    if let Some(doc) = collect_doc_comments_before(node) {
        return Some(doc);
    }
    if let Some(parent) = node.parent()
        && parent.kind() == SyntaxKind::ExportsList
    {
        return collect_doc_comments_before(&parent);
    }
    None
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

/// Extract and remove `\group` or `\group:name` annotation.
/// Returns `Some(name)` when a group was specified (name may be empty
/// for bare `\group`), or `None` when the annotation is absent.
fn extract_group(doc: &mut String) -> Option<String> {
    let mut result = None;
    let mut lines: Vec<&str> = doc.lines().collect();
    for i in (0..lines.len()).rev() {
        if let Some(val) = lines[i].strip_prefix("\\group:") {
            result = Some(val.trim().to_string());
            lines.remove(i);
        } else if lines[i].trim() == "\\group" {
            result = Some(String::new());
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
}

impl ScreenshotCounter {
    fn new(element_name: &str) -> Self {
        Self { element_slug: mdx::to_kebab_case(element_name), next: 1 }
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
#[allow(clippy::while_let_on_iterator)] // inner loop also advances `lines`
fn transform_code_fences(text: &str, counter: &mut ScreenshotCounter) -> String {
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

                    // Keep non-screenshot attributes (e.g. `playground`) on the fence.
                    let fence_attrs: Vec<&str> = attrs
                        .iter()
                        .filter(|(k, _)| !is_screenshot_attr(k))
                        .map(|(k, _)| k.as_str())
                        .collect();

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

                    result.push_str(indent);
                    result.push_str("</CodeSnippetMD>\n");
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

// -- AST helpers --

fn type_text(type_node: &syntax_nodes::Type) -> String {
    if let Some(qn) = type_node.QualifiedName() {
        return identifier_text(&qn).unwrap_or_default().to_string();
    }
    let node: &parser::SyntaxNode = type_node;
    node.text().to_string().trim().to_string()
}

fn property_visibility(prop: &syntax_nodes::PropertyDeclaration) -> &'static str {
    let node: &parser::SyntaxNode = prop;
    for token in node.children_with_tokens() {
        if token.kind() == SyntaxKind::Identifier {
            match token.as_token().unwrap().text() {
                "in-out" => return "in-out",
                "in" => return "in",
                "out" => return "out",
                "property" => return "",
                _ => {}
            }
        }
    }
    ""
}

fn callback_signature(cb: &syntax_nodes::CallbackDeclaration) -> String {
    let params: Vec<String> = cb
        .CallbackDeclarationParameter()
        .map(|p| {
            let pname = p
                .DeclaredIdentifier()
                .and_then(|d| identifier_text(&d))
                .map(|s| s.to_string())
                .unwrap_or_default();
            let ptype = type_text(&p.Type());
            if pname.is_empty() { ptype } else { format!("{pname}: {ptype}") }
        })
        .collect();
    let ret = cb.ReturnType().map(|r| format!(" -> {}", type_text(&r.Type())));
    format!("({}){}", params.join(", "), ret.unwrap_or_default())
}

fn function_signature(f: &syntax_nodes::Function) -> String {
    let params: Vec<String> = f
        .ArgumentDeclaration()
        .map(|a| {
            format!(
                "{}: {}",
                identifier_text(&a.DeclaredIdentifier()).unwrap_or_default(),
                type_text(&a.Type())
            )
        })
        .collect();
    let ret = f.ReturnType().map(|r| format!(" -> {}", type_text(&r.Type())));
    format!("({}){}", params.join(", "), ret.unwrap_or_default())
}

// -- Member extraction --

fn extract_members(elem_node: &syntax_nodes::Element) -> Vec<MemberDoc> {
    let mut members = Vec::new();
    let mut in_code_fence = false;

    for child in elem_node.children_with_tokens() {
        match child.kind() {
            SyntaxKind::PropertyDeclaration => {
                let p = syntax_nodes::PropertyDeclaration::from(child.into_node().unwrap());
                if p.TwoWayBinding().is_some() {
                    continue;
                }
                let doc = extract_doc_comment(&p);
                let has_doc_comment = doc.is_some();
                let raw = doc.unwrap_or_default();
                let (description, doc_default) = extract_default(&raw);
                let mut default_value = p
                    .BindingExpression()
                    .map(|b| {
                        let n: &parser::SyntaxNode = &b;
                        n.text().to_string().trim().trim_end_matches(';').trim().to_string()
                    })
                    .unwrap_or_default();
                if default_value.is_empty()
                    && let Some(d) = doc_default
                {
                    default_value = d;
                }
                members.push(MemberDoc {
                    name: identifier_text(&p.DeclaredIdentifier()).unwrap().to_string(),
                    kind: MemberKind::Property,
                    type_name: p.Type().map(|t| type_text(&t)).unwrap_or_default(),
                    description,
                    direction: property_visibility(&p).to_string(),
                    default_value,
                    has_doc_comment,
                });
            }
            SyntaxKind::CallbackDeclaration => {
                let cb = syntax_nodes::CallbackDeclaration::from(child.into_node().unwrap());
                if cb.TwoWayBinding().is_some() {
                    continue;
                }
                let doc = extract_doc_comment(&cb);
                let has_doc_comment = doc.is_some();
                members.push(MemberDoc {
                    name: identifier_text(&cb.DeclaredIdentifier()).unwrap().to_string(),
                    kind: MemberKind::Callback,
                    type_name: callback_signature(&cb),
                    description: doc.unwrap_or_default(),
                    direction: String::new(),
                    default_value: String::new(),
                    has_doc_comment,
                });
            }
            SyntaxKind::Function => {
                let f = syntax_nodes::Function::from(child.into_node().unwrap());
                let doc = extract_doc_comment(&f);
                let has_doc_comment = doc.is_some();
                members.push(MemberDoc {
                    name: identifier_text(&f.DeclaredIdentifier()).unwrap().to_string(),
                    kind: MemberKind::Function,
                    type_name: function_signature(&f),
                    description: doc.unwrap_or_default(),
                    direction: String::new(),
                    default_value: String::new(),
                    has_doc_comment,
                });
            }
            SyntaxKind::Comment => {
                if let Some(t) = child.as_token() {
                    let text = t.text();
                    if let Some(content) =
                        text.strip_prefix("//! ").or_else(|| text.strip_prefix("//!"))
                    {
                        // Track code fences: lines starting with `#` inside a
                        // code fence are not markdown headings.
                        let is_heading = content.starts_with('#') && !in_code_fence;
                        if content.starts_with("```") || content.starts_with("~~~") {
                            in_code_fence = !in_code_fence;
                        }

                        // Merge consecutive non-heading //! lines into one block.
                        if !is_heading
                            && let Some(last) = members.last_mut()
                            && last.kind == MemberKind::SectionHeader
                        {
                            last.description.push('\n');
                            last.description.push_str(content);
                            continue;
                        }
                        members.push(MemberDoc {
                            name: String::new(),
                            kind: MemberKind::SectionHeader,
                            type_name: String::new(),
                            description: content.to_string(),
                            direction: String::new(),
                            default_value: String::new(),
                            has_doc_comment: true,
                        });
                    }
                }
            }
            _ => {}
        }
    }
    members
}

// -- Inheritance resolution --

fn resolve_inheritance(
    components: &mut BTreeMap<String, ElementDoc>,
    inheritance: &HashMap<String, String>,
) {
    let names: Vec<String> = components.keys().cloned().collect();
    for name in &names {
        if components[name].skip_inherited {
            continue;
        }

        // Build ancestor chain.
        let mut chain = Vec::new();
        let mut cur = name.clone();
        while let Some(parent) = inheritance.get(&cur) {
            if chain.contains(parent) {
                break;
            }
            chain.push(parent.clone());
            cur = parent.clone();
        }
        if chain.is_empty() {
            continue;
        }

        // Collect inherited members, ancestors first.
        let mut inherited = Vec::new();
        for ancestor in chain.iter().rev() {
            if let Some(parent_elem) = components.get(ancestor) {
                for m in &parent_elem.members {
                    if m.kind == MemberKind::SectionHeader
                        || !inherited.iter().any(|im: &MemberDoc| im.name == m.name)
                    {
                        inherited.push(m.clone());
                    }
                }
            }
        }

        let elem = components.get_mut(name).unwrap();
        let own_names: HashSet<String> = elem.members.iter().map(|m| m.name.clone()).collect();
        inherited.retain(|m| m.kind == MemberKind::SectionHeader || !own_names.contains(&m.name));
        inherited.append(&mut elem.members);
        elem.members = inherited;
    }
}

/// Walk up the inheritance chain to find the first value for which
/// `getter` returns `Some`.
fn resolve_inherited<T: Clone>(
    name: &str,
    components: &BTreeMap<String, ElementDoc>,
    inheritance: &HashMap<String, String>,
    getter: fn(&ElementDoc) -> Option<&T>,
) -> Option<T> {
    let mut current = name;
    loop {
        if let Some(elem) = components.get(current)
            && let Some(val) = getter(elem)
        {
            return Some(val.clone());
        }
        match inheritance.get(current) {
            Some(parent) => current = parent,
            None => return None,
        }
    }
}

/// Convenience wrapper: resolve a `String` field, treating empty as absent.
fn resolve_inherited_field(
    name: &str,
    components: &BTreeMap<String, ElementDoc>,
    inheritance: &HashMap<String, String>,
    getter: fn(&ElementDoc) -> &String,
) -> String {
    let mut current = name;
    loop {
        if let Some(elem) = components.get(current) {
            let val = getter(elem);
            if !val.is_empty() {
                return val.clone();
            }
        }
        match inheritance.get(current) {
            Some(parent) => current = parent,
            None => return String::new(),
        }
    }
}

/// Build an `ElementDoc` for export, resolving inherited fields.
fn build_exported_doc(
    export_name: String,
    internal_name: &str,
    elem: &ElementDoc,
    components: &BTreeMap<String, ElementDoc>,
    inheritance: &HashMap<String, String>,
) -> ElementDoc {
    ElementDoc {
        name: export_name,
        is_global: elem.is_global,
        description: resolve_inherited_field(internal_name, components, inheritance, |e| {
            &e.description
        }),
        footer: resolve_inherited_field(internal_name, components, inheritance, |e| &e.footer),
        draft: elem.draft,
        group: resolve_inherited(internal_name, components, inheritance, |e| e.group.as_ref()),
        skip_inherited: false,
        skip_children: elem.skip_children,
        members: elem.members.clone(),
        children: elem.children.clone(),
    }
}

// -- Parsing builtins.slint --

fn extract_builtin_element_docs() -> (Vec<ElementDoc>, BTreeMap<String, ElementDoc>) {
    let builtins_path = crate::root_dir().join("internal/compiler/builtins.slint");
    let source = std::fs::read_to_string(&builtins_path)
        .unwrap_or_else(|e| panic!("Failed to read {builtins_path:?}: {e}"));

    let mut diag = i_slint_compiler::diagnostics::BuildDiagnostics::default();
    let root = parser::parse(source, Some(builtins_path.as_ref()), &mut diag);
    if diag.has_errors() {
        panic!("Error parsing builtins.slint: {:?}", diag.to_string_vec());
    }
    let doc: syntax_nodes::Document = root.into();

    // Map exported_name -> internal_name for `export { X as Y }`.
    let mut export_aliases = HashMap::<String, String>::new();
    for export_list in doc.ExportsList() {
        for spec in export_list.ExportSpecifier() {
            let internal = identifier_text(&spec.ExportIdentifier()).unwrap().to_string();
            let exported = identifier_text(&spec.ExportName().unwrap()).unwrap().to_string();
            export_aliases.insert(exported, internal);
        }
    }

    let mut components = BTreeMap::<String, ElementDoc>::new();
    let mut inheritance = HashMap::<String, String>::new();

    for c in doc.Component().chain(doc.ExportsList().filter_map(|e| e.Component())) {
        let id = identifier_text(&c.DeclaredIdentifier()).unwrap().to_string();
        let is_global =
            c.child_text(SyntaxKind::Identifier).is_some_and(|t| t.as_str() == "global");
        let elem_node = c.Element();

        if let Some(base) = elem_node.QualifiedName() {
            inheritance.insert(id.clone(), identifier_text(&base).unwrap().to_string());
        }

        let mut description = extract_doc_comment(&c).unwrap_or_default();
        let group = extract_group(&mut description);
        let draft = strip_annotation(&mut description, "\\draft");
        let (desc, footer) = split_footer(&description);
        description = desc;
        let skip_inherited = strip_annotation(&mut description, "\\skip_inherited");
        let skip_children = strip_annotation(&mut description, "\\skip_children");

        let children = elem_node
            .SubElement()
            .filter_map(|sub| {
                sub.Element()
                    .QualifiedName()
                    .and_then(|qn| identifier_text(&qn))
                    .map(|s| s.to_string())
            })
            .collect();

        components.insert(
            id,
            ElementDoc {
                name: String::new(),
                is_global,
                description,
                footer,
                draft,
                group,
                skip_inherited,
                skip_children,
                members: extract_members(&elem_node),
                children,
            },
        );
    }

    resolve_inheritance(&mut components, &inheritance);

    // Collect exported elements.
    let mut result = Vec::new();
    let mut seen = HashSet::new();

    // First pass: directly exported components (export component X { ... }).
    for internal_name in components.keys() {
        let is_directly_exported = doc.ExportsList().any(|e| {
            e.Component().is_some_and(|c| {
                identifier_text(&c.DeclaredIdentifier()).unwrap().as_str() == internal_name
            })
        });
        if !is_directly_exported {
            continue;
        }
        let export_name = export_aliases
            .iter()
            .find(|(_, v)| v.as_str() == internal_name)
            .map(|(k, _)| k.clone())
            .unwrap_or_else(|| internal_name.clone());
        seen.insert(export_name.clone());
        let elem = &components[internal_name];
        result.push(build_exported_doc(
            export_name,
            internal_name,
            elem,
            &components,
            &inheritance,
        ));
    }

    // Second pass: re-exported aliases (export { X as Y }).
    for (export_name, internal_name) in &export_aliases {
        if seen.contains(export_name) {
            continue;
        }
        if let Some(elem) = components.get(internal_name) {
            result.push(build_exported_doc(
                export_name.clone(),
                internal_name,
                elem,
                &components,
                &inheritance,
            ));
        }
    }

    // Third pass: non-exported components with a group (e.g. MenuBar).
    let seen_internal: HashSet<&str> = export_aliases.values().map(|s| s.as_str()).collect();
    for (name, elem) in &components {
        if seen.contains(name.as_str()) || seen_internal.contains(name.as_str()) {
            continue;
        }
        let doc = build_exported_doc(name.clone(), name, elem, &components, &inheritance);
        if doc.group.is_none() {
            continue;
        }
        seen.insert(name.clone());
        result.push(doc);
    }

    result.sort_by(|a, b| a.name.cmp(&b.name));
    (result, components)
}

// -- MDX output --

/// Build a set of internal component names that have their own doc page.
fn components_with_own_page(all: &BTreeMap<String, ElementDoc>) -> HashSet<String> {
    all.iter()
        .filter(|(_, e)| !e.description.is_empty() || e.members.iter().any(|m| m.has_doc_comment))
        .map(|(k, _)| k.clone())
        .collect()
}

fn write_slint_property(
    file: &mut impl Write,
    m: &MemberDoc,
    heading: &str,
    enums: &HashSet<String>,
    structs: &HashSet<String>,
    sc: &mut ScreenshotCounter,
) -> std::io::Result<()> {
    let (type_attr, enum_name, struct_name) = if enums.contains(&m.type_name) {
        ("enum", Some(&m.type_name), None)
    } else if structs.contains(&m.type_name) {
        ("struct", None, Some(&m.type_name))
    } else {
        (m.type_name.as_str(), None, None)
    };

    writeln!(file, "{heading} {}", m.name)?;
    write!(file, "<SlintProperty propName=\"{}\" typeName=\"{type_attr}\"", m.name)?;
    if let Some(en) = enum_name {
        write!(file, " enumName=\"{en}\"")?;
    }
    if let Some(sn) = struct_name {
        write!(file, " structName=\"{sn}\"")?;
    }
    if m.direction == "out" || m.direction == "in-out" {
        write!(file, " propertyVisibility=\"{}\"", m.direction)?;
    }
    if !m.default_value.is_empty() {
        write!(file, " defaultValue=\"{}\"", m.default_value.replace('"', "&quot;"))?;
    }
    if m.description.is_empty() {
        writeln!(file, "/>")?;
    } else {
        writeln!(file, ">")?;
        writeln!(file, "{}", transform_code_fences(&m.description, sc).trim_end())?;
        writeln!(file, "</SlintProperty>")?;
    }
    writeln!(file)?;
    Ok(())
}

/// Write properties, callbacks, functions, and section headers.
/// Automatically inserts `## Properties` / `## Callbacks` / `## Functions`
/// headings unless a `//!` section header already provides them.
fn write_members(
    file: &mut impl Write,
    members: &[MemberDoc],
    enums: &HashSet<String>,
    structs: &HashSet<String>,
    sc: &mut ScreenshotCounter,
) -> std::io::Result<()> {
    // If the first documented thing is a section header, don't auto-generate "## Properties".
    let has_leading_section = members
        .iter()
        .find(|m| m.kind == MemberKind::SectionHeader || m.kind == MemberKind::Property)
        .is_some_and(|m| m.kind == MemberKind::SectionHeader);
    let mut in_properties = has_leading_section;
    let mut in_callbacks = false;
    let mut in_functions = false;

    for m in members {
        match m.kind {
            MemberKind::SectionHeader => {
                let desc = m.description.trim_end();
                if desc.starts_with("## Properties") {
                    in_properties = true;
                }
                if desc.starts_with("## Callbacks") {
                    in_callbacks = true;
                }
                if desc.starts_with("## Functions") {
                    in_functions = true;
                }
                writeln!(file, "{}", transform_code_fences(desc, sc))?;
                writeln!(file)?;
            }
            MemberKind::Property if m.has_doc_comment => {
                if !in_properties && !in_callbacks && !in_functions {
                    writeln!(file, "## Properties")?;
                    writeln!(file)?;
                    in_properties = true;
                }
                write_slint_property(file, m, "###", enums, structs, sc)?;
            }
            MemberKind::Callback if m.has_doc_comment => {
                if !in_callbacks {
                    writeln!(file, "## Callbacks")?;
                    writeln!(file)?;
                    in_callbacks = true;
                }
                writeln!(file, "### {}{}", m.name, m.type_name)?;
                if !m.description.is_empty() {
                    writeln!(file, "{}", transform_code_fences(&m.description, sc).trim_end())?;
                }
                writeln!(file)?;
            }
            MemberKind::Function if m.has_doc_comment => {
                if !in_functions {
                    writeln!(file, "## Functions")?;
                    writeln!(file)?;
                    in_functions = true;
                }
                writeln!(file, "### {}{}", m.name, m.type_name)?;
                writeln!(file, "{}", transform_code_fences(&m.description, sc).trim_end())?;
                writeln!(file)?;
            }
            _ => {}
        }
    }
    Ok(())
}

/// Write a sub-element section. Recurse into the sub-element's own children.
fn write_sub_element(
    file: &mut impl Write,
    child_name: &str,
    child: &ElementDoc,
    all_components: &BTreeMap<String, ElementDoc>,
    enums: &HashSet<String>,
    structs: &HashSet<String>,
    seen: &mut HashSet<String>,
    sc: &mut ScreenshotCounter,
) -> std::io::Result<()> {
    if !seen.insert(child_name.to_string()) {
        return Ok(());
    }
    let has_doc = !child.description.is_empty() || child.members.iter().any(|m| m.has_doc_comment);
    if !has_doc {
        return Ok(());
    }

    writeln!(file, "## `{child_name}`")?;
    writeln!(file)?;
    if !child.description.is_empty() {
        writeln!(file, "{}", transform_code_fences(&child.description, sc).trim_end())?;
        writeln!(file)?;
    }

    let props: Vec<_> = child
        .members
        .iter()
        .filter(|m| m.kind == MemberKind::Property && m.has_doc_comment)
        .collect();
    let cbs: Vec<_> = child
        .members
        .iter()
        .filter(|m| m.kind == MemberKind::Callback && m.has_doc_comment)
        .collect();
    let fns: Vec<_> = child
        .members
        .iter()
        .filter(|m| m.kind == MemberKind::Function && m.has_doc_comment)
        .collect();

    // Use grouped (####) headings when there are multiple member kinds.
    let grouped = (props.len() + cbs.len() + fns.len()) > 0
        && [!props.is_empty(), !cbs.is_empty(), !fns.is_empty()].iter().filter(|&&b| b).count() > 1;
    let (prop_h, cb_h, fn_h) =
        if grouped { ("####", "####", "####") } else { ("###", "###", "###") };

    if !props.is_empty() {
        if grouped {
            writeln!(file, "### Properties of `{child_name}`")?;
            writeln!(file)?;
        }
        for p in &props {
            write_slint_property(file, p, prop_h, enums, structs, sc)?;
        }
    }
    if !cbs.is_empty() {
        writeln!(file, "### Callbacks of `{child_name}`")?;
        writeln!(file)?;
        for c in &cbs {
            writeln!(file, "{cb_h} {}{}", c.name, c.type_name)?;
            if !c.description.is_empty() {
                writeln!(file, "{}", transform_code_fences(&c.description, sc).trim_end())?;
            }
            writeln!(file)?;
        }
    }
    if !fns.is_empty() {
        writeln!(file, "### Functions of `{child_name}`")?;
        writeln!(file)?;
        for f in &fns {
            writeln!(file, "{fn_h} {}{}", f.name, f.type_name)?;
            writeln!(file, "{}", transform_code_fences(&f.description, sc).trim_end())?;
            writeln!(file)?;
        }
    }

    // Recurse into grandchildren.
    for gc_name in &child.children {
        if let Some(gc) = all_components.get(gc_name.as_str()) {
            write_sub_element(file, gc_name, gc, all_components, enums, structs, seen, sc)?;
        }
    }

    Ok(())
}

/// Collect all text from an element and its descendants for import detection.
fn collect_all_text(elem: &ElementDoc, all: &BTreeMap<String, ElementDoc>) -> String {
    let mut text = format!("{} {}", elem.description, elem.footer);
    for m in &elem.members {
        text.push(' ');
        text.push_str(&m.description);
    }
    if elem.skip_children {
        return text;
    }
    let mut seen = HashSet::new();
    fn collect_children(
        names: &[String],
        all: &BTreeMap<String, ElementDoc>,
        text: &mut String,
        seen: &mut HashSet<String>,
    ) {
        for name in names {
            if !seen.insert(name.clone()) {
                continue;
            }
            if let Some(child) = all.get(name.as_str()) {
                text.push(' ');
                text.push_str(&child.description);
                for m in &child.members {
                    text.push(' ');
                    text.push_str(&m.description);
                }
                collect_children(&child.children, all, text, seen);
            }
        }
    }
    collect_children(&elem.children, all, &mut text, &mut seen);
    text
}

/// Generate .mdx page files for each exported builtin element.
pub fn generate() -> Result<(), Box<dyn std::error::Error>> {
    let (elements, all_components) = extract_builtin_element_docs();
    let root_dir = crate::root_dir();
    let generated_dir = root_dir.join("docs/astro/src/content/docs/reference/generated");
    create_dir_all(&generated_dir)?;

    // Include all types for resolution, regardless of experimental flag.
    let enum_names: HashSet<String> = mdx::extract_enum_docs(true).keys().cloned().collect();
    let struct_names: HashSet<String> =
        mdx::extract_builtin_structs(true).keys().cloned().collect();
    let own_page = components_with_own_page(&all_components);

    for elem in &elements {
        if elem.draft {
            continue;
        }
        let has_docs =
            !elem.description.is_empty() || elem.members.iter().any(|m| m.has_doc_comment);
        if !has_docs {
            continue;
        }

        let filename = format!("{}.mdx", elem.name.to_ascii_lowercase());
        let path = match elem.group.as_deref() {
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
        let slug = match elem.group.as_deref() {
            Some(group) if !group.is_empty() => format!("reference/{group}/{slug_stem}"),
            _ => format!("reference/{slug_stem}"),
        };
        writeln!(file, "---")?;
        writeln!(file, "title: {}", elem.name)?;
        if elem.is_global {
            writeln!(file, "description: {} Namespace", elem.name)?;
        } else {
            writeln!(file, "description: {} element api.", elem.name)?;
        }
        writeln!(file, "slug: {slug}")?;
        writeln!(file, "---")?;

        // Imports.
        let all_text = collect_all_text(elem, &all_components);
        // Always needed.
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
        // Sorted struct/enum imports.
        let mut extra_imports = Vec::new();
        for name in &struct_names {
            if all_text.contains(&format!("<{name} />")) || all_text.contains(&format!("<{name}/>"))
            {
                extra_imports.push(format!(
                    "import {name} from '/src/content/docs/reference/generated/structs/{name}.md';"
                ));
            }
        }
        for name in &enum_names {
            if all_text.contains(&format!("<{name} />")) || all_text.contains(&format!("<{name}/>"))
            {
                extra_imports.push(format!(
                    "import {name} from '/src/content/docs/reference/generated/enums/{name}.md';"
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

        let mut sc = ScreenshotCounter::new(&elem.name);

        // Description.
        if !elem.description.is_empty() {
            writeln!(file, "{}", transform_code_fences(&elem.description, &mut sc).trim_end())?;
            writeln!(file)?;
        }

        // Members.
        write_members(&mut file, &elem.members, &enum_names, &struct_names, &mut sc)?;

        // Sub-elements (recursive, with cycle protection).
        // Skip children that have their own page.
        if !elem.skip_children {
            let mut seen_children = HashSet::new();
            for child_name in &elem.children {
                if own_page.contains(child_name) {
                    continue;
                }
                if let Some(child) = all_components.get(child_name) {
                    write_sub_element(
                        &mut file,
                        child_name,
                        child,
                        &all_components,
                        &enum_names,
                        &struct_names,
                        &mut seen_children,
                        &mut sc,
                    )?;
                }
            }
        }

        // Footer.
        if !elem.footer.is_empty() {
            writeln!(file, "{}", transform_code_fences(&elem.footer, &mut sc).trim_end())?;
        }
        file.flush()?;
    }
    Ok(())
}
