// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_compiler::diagnostics::SourceFile;
use i_slint_editor_preview::{ByteFormat, util};
use jsonc_parser::{ParseOptions, ast, common::Ranged};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub(super) struct CanvasPreset {
    pub width: i32,
    pub height: i32,
}

#[derive(Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
struct VisualEditorSettings {
    #[serde(default)]
    canvas_presets: Vec<CanvasPreset>,
}

fn parse(source: &str) -> Result<ast::Value<'_>, String> {
    jsonc_parser::parse_to_ast(
        source,
        &Default::default(),
        &ParseOptions {
            allow_comments: true,
            allow_loose_object_property_names: false,
            allow_trailing_commas: false,
            allow_missing_commas: false,
            allow_single_quoted_strings: false,
            allow_hexadecimal_numbers: false,
            allow_unary_plus_numbers: false,
            allow_bare_decimal_point_numbers: false,
            allow_non_finite_numbers: false,
            allow_extended_string_escapes: false,
        },
    )
    .map_err(|error| error.to_string())?
    .value
    .ok_or_else(|| "Project file must contain a JSON object".into())
}

fn property<'tree, 'source>(
    object: &'tree ast::Object<'source>,
    name: &str,
) -> Result<Option<&'tree ast::Value<'source>>, String> {
    let mut properties = object.properties.iter().filter(|property| property.name.as_str() == name);
    let value = properties.next().map(|property| &property.value);
    if properties.next().is_some() {
        return Err(format!("Duplicate '{name}' property"));
    }
    Ok(value)
}

fn object<'tree, 'source>(
    value: &'tree ast::Value<'source>,
    name: &str,
) -> Result<&'tree ast::Object<'source>, String> {
    value.as_object().ok_or_else(|| format!("'{name}' must be an object"))
}

fn child_property<'tree, 'source>(
    value: Option<&'tree ast::Value<'source>>,
    parent: &str,
    name: &str,
) -> Result<Option<&'tree ast::Value<'source>>, String> {
    value.map(|value| property(object(value, parent)?, name)).transpose().map(Option::flatten)
}

fn validate_presets(presets: &[CanvasPreset]) -> Result<(), String> {
    if presets.iter().any(|preset| preset.width <= 0 || preset.height <= 0) {
        return Err("Canvas width and height must be positive integers".into());
    }
    Ok(())
}

pub(super) fn read_canvas_presets(source: &str) -> Result<Vec<CanvasPreset>, String> {
    let root = parse(source)?;
    let presets = property(object(&root, "Project file")?, "metadata")
        .and_then(|metadata| child_property(metadata, "metadata", "visual-editor"))
        .and_then(|settings| child_property(settings, "visual-editor", "canvas-presets"))?;
    let Some(presets) = presets else { return Ok(Vec::new()) };
    let presets = presets.as_array().ok_or("'canvas-presets' must be an array")?;
    let presets = presets
        .elements
        .iter()
        .map(|preset| {
            let preset = object(preset, "Canvas preset")?;
            let dimension = |name| {
                property(preset, name)?
                    .and_then(ast::Value::as_number_lit)
                    .and_then(|number| number.value.parse::<i32>().ok())
                    .ok_or_else(|| format!("Canvas '{name}' must be a positive integer"))
            };
            Ok(CanvasPreset { width: dimension("width")?, height: dimension("height")? })
        })
        .collect::<Result<Vec<_>, String>>()?;
    validate_presets(&presets)?;
    Ok(presets)
}

pub(super) fn update_canvas_presets_edit(
    source_file: &SourceFile,
    version: Option<i32>,
    presets: &[CanvasPreset],
    format: ByteFormat,
) -> Result<lsp_types::WorkspaceEdit, String> {
    validate_presets(presets)?;
    let source = source_file.source().ok_or("Project file has no contents")?;
    let settings =
        serde_json::to_string(&VisualEditorSettings { canvas_presets: presets.to_vec() })
            .map_err(|error| error.to_string())?;
    let root = parse(source)?;
    let root = object(&root, "Project file")?;
    let newline = if source.contains("\r\n") { "\r\n" } else { "\n" };
    let (root_indent, property_indent) = object_indentation(source, root, "", "    ");
    let indent = property_indent
        .strip_prefix(&root_indent)
        .filter(|indent| !indent.is_empty())
        .unwrap_or("    ");
    let (range, replacement) = match property(root, "metadata")? {
        Some(metadata) => {
            let metadata = object(metadata, "metadata")?;
            match property(metadata, "visual-editor")? {
                Some(value) => (value.range().start..value.range().end, settings),
                None => {
                    let (base, child) =
                        object_indentation(source, metadata, &property_indent, indent);
                    insert_property(
                        source,
                        metadata,
                        "visual-editor",
                        &settings,
                        &base,
                        &child,
                        newline,
                    )
                }
            }
        }
        None => {
            let value = format!(
                "{{{newline}{property_indent}{indent}\"visual-editor\": {settings}{newline}{property_indent}}}"
            );
            insert_property(
                source,
                root,
                "metadata",
                &value,
                &root_indent,
                &property_indent,
                newline,
            )
        }
    };
    let edit = lsp_types::TextEdit {
        range: lsp_types::Range::new(
            util::text_size_to_lsp_position(source_file, (range.start as u32).into(), format),
            util::text_size_to_lsp_position(source_file, (range.end as u32).into(), format),
        ),
        new_text: replacement,
    };
    let document = lsp_types::TextDocumentEdit {
        text_document: lsp_types::OptionalVersionedTextDocumentIdentifier {
            uri: source_file.path().to_url().ok_or("Project file has no URL")?,
            version,
        },
        edits: vec![lsp_types::OneOf::Left(edit)],
    };
    Ok(i_slint_editor_preview::editing::create_workspace_edit_from_text_document_edits(vec![
        document,
    ]))
}

fn line_indentation(source: &str, offset: usize) -> Option<&str> {
    let line_start = source[..offset].rfind('\n').map_or(0, |offset| offset + 1);
    let prefix = &source[line_start..offset];
    prefix.bytes().all(|byte| matches!(byte, b' ' | b'\t')).then_some(prefix)
}

fn object_indentation(
    source: &str,
    object: &ast::Object<'_>,
    fallback: &str,
    indent: &str,
) -> (String, String) {
    let base = line_indentation(source, object.range.start)
        .or_else(|| line_indentation(source, object.range.end - 1))
        .unwrap_or(fallback)
        .to_string();
    let child = object
        .properties
        .iter()
        .find_map(|property| line_indentation(source, property.range.start))
        .map_or_else(|| format!("{base}{indent}"), str::to_string);
    (base, child)
}

fn insert_property(
    source: &str,
    object: &ast::Object<'_>,
    name: &str,
    value: &str,
    base: &str,
    indent: &str,
    newline: &str,
) -> (std::ops::Range<usize>, String) {
    let closing = object.range.end - 1;
    let closing_indent = line_indentation(source, closing);
    let insertion = closing - closing_indent.map_or(0, str::len);
    let last = object.properties.last();
    let start = last.map_or(object.range.start + 1, |property| property.value.range().end);
    let mut replacement = if last.is_some() { ",".to_owned() } else { String::new() };
    replacement.push_str(&source[start..insertion]);
    if !replacement.ends_with('\n') {
        replacement.push_str(newline);
    }
    replacement.push_str(&format!("{indent}\"{name}\": {value}{newline}"));
    if closing_indent.is_none() {
        replacement.push_str(base);
    }
    (start..insertion, replacement)
}

#[cfg(test)]
mod tests {
    use super::*;
    use i_slint_compiler::{diagnostics::SourceFileInner, source_path::SourcePath};

    fn presets() -> Vec<CanvasPreset> {
        vec![CanvasPreset { width: 720, height: 1280 }, CanvasPreset { width: 1920, height: 1080 }]
    }

    #[test]
    fn edits_preserve_surrounding_source() {
        let settings =
            r#"{"canvas-presets":[{"width":720,"height":1280},{"width":1920,"height":1080}]}"#;
        let fixtures = [
            (
                r#"{
    }"#,
                r#"{
    "metadata": {
        "visual-editor": SETTINGS
    }
    }"#,
            ),
            (
                r#"{}"#,
                r#"{
    "metadata": {
        "visual-editor": SETTINGS
    }
}"#,
            ),
            (
                r#"{/* retained */}"#,
                r#"{/* retained */
    "metadata": {
        "visual-editor": SETTINGS
    }
}"#,
            ),
            (
                r#"{"main":"🦊.slint"}"#,
                r#"{"main":"🦊.slint",
    "metadata": {
        "visual-editor": SETTINGS
    }
}"#,
            ),
            (
                r#"{
  "main": "🦊.slint" // retained
}"#,
                r#"{
  "main": "🦊.slint", // retained
  "metadata": {
    "visual-editor": SETTINGS
  }
}"#,
            ),
            (
                r#"{
    "main": "main.slint" /* retained */
    // retained before closing brace
}"#,
                r#"{
    "main": "main.slint", /* retained */
    // retained before closing brace
    "metadata": {
        "visual-editor": SETTINGS
    }
}"#,
            ),
            (
                r#"{"metadata":{/* before */"other":42/* after */}}"#,
                r#"{"metadata":{/* before */"other":42,/* after */
        "visual-editor": SETTINGS
    }}"#,
            ),
            (
                r#"{
    "metadata": {
        "other": 42 // keep
    }
}"#,
                r#"{
    "metadata": {
        "other": 42, // keep
        "visual-editor": SETTINGS
    }
}"#,
            ),
            (
                r#"{
    "metadata": {
    }
}"#,
                r#"{
    "metadata": {
        "visual-editor": SETTINGS
    }
}"#,
            ),
            (
                r#"{"metadata":{}}"#,
                r#"{"metadata":{
        "visual-editor": SETTINGS
    }}"#,
            ),
            (
                r#"{"metadata":{"visual-editor" /* keep */: /* keep */ { /* discard */ "unknown":true } /* keep */,"other":42}}"#,
                r#"{"metadata":{"visual-editor" /* keep */: /* keep */ SETTINGS /* keep */,"other":42}}"#,
            ),
            (
                r#"{"metad\u0061ta":{"visual\u002deditor":null}}"#,
                r#"{"metad\u0061ta":{"visual\u002deditor":SETTINGS}}"#,
            ),
            (
                r#"{"main":"🦊.slint","metadata":{"visual-editor":{}}}"#,
                r#"{"main":"🦊.slint","metadata":{"visual-editor":SETTINGS}}"#,
            ),
            (
                "{\n\t\"main\": \"main.slint\" // keep\n}",
                "{\n\t\"main\": \"main.slint\", // keep\n\t\"metadata\": {\n\t\t\"visual-editor\": SETTINGS\n\t}\n}",
            ),
        ];
        for (source, expected) in fixtures {
            for crlf in [false, true] {
                let mut source = source.to_owned();
                let mut expected = expected.replace("SETTINGS", settings);
                if crlf {
                    source = source.replace('\n', "\r\n");
                    expected = expected.replace('\n', "\r\n");
                    if !source.contains("\r\n") {
                        source.push_str("\r\n");
                        expected.push_str("\r\n");
                    }
                }
                for format in [ByteFormat::Utf8, ByteFormat::Utf16] {
                    let source_file = std::sync::Arc::new(SourceFileInner::new(
                        SourcePath::new("/test/slint-project.json"),
                        source.clone(),
                    ));
                    let edit =
                        update_canvas_presets_edit(&source_file, Some(7), &presets(), format)
                            .unwrap();
                    let Some(lsp_types::DocumentChanges::Edits(documents)) = edit.document_changes
                    else {
                        panic!("Expected document edits");
                    };
                    assert_eq!(documents.len(), 1);
                    assert_eq!(documents[0].text_document.version, Some(7));
                    let mut editor =
                        i_slint_editor_preview::editing::text_edit::TextEditor::new(source_file)
                            .unwrap();
                    for edit in &documents[0].edits {
                        let lsp_types::OneOf::Left(edit) = edit else {
                            panic!("Expected text edit");
                        };
                        editor.apply(edit, format).unwrap();
                    }
                    let result = editor.finalize().unwrap().0;
                    assert_eq!(result, expected);
                    assert_eq!(read_canvas_presets(&result).unwrap(), presets());
                }
            }
        }
    }

    #[test]
    fn absent_settings_and_invalid_settings() {
        for source in ["{}", r#"{"metadata":{}}"#, r#"{"metadata":{"visual-editor":{}}}"#] {
            assert!(read_canvas_presets(source).unwrap().is_empty());
        }
        for source in [
            "",
            " \t\r\n",
            "// comment",
            "/* comment */",
            r#"{ "metadata": { this is not json! } }"#,
            "{",
            "[]",
            "{'metadata':{}}",
            "{metadata:{}}",
            "{\"metadata\":{},}",
            r#"{"metadata":null}"#,
            r#"{"metadata":{"visual-editor":null}}"#,
            r#"{"metadata":{"visual-editor":{"canvas-presets":[],"canvas-presets":[]}}}"#,
            r#"{"metadata":{},"metadata":{}}"#,
            r#"{"metadata":{"visual-editor":{},"visual-editor":{}}}"#,
            r#"{"metadata":{"visual-editor":{"canvas-presets":null}}}"#,
            r#"{"metadata":{"visual-editor":{"canvas-presets":[{"width":0,"height":2}]}}}"#,
            r#"{"metadata":{"visual-editor":{"canvas-presets":[{"width":1.5,"height":2}]}}}"#,
            r#"{"metadata":{"visual-editor":{"canvas-presets":[{"width":2147483648,"height":2}]}}}"#,
            r#"{"metadata":{"visual-editor":{"canvas-presets":[{"height":2}]}}}"#,
        ] {
            assert!(read_canvas_presets(source).is_err(), "{source}");
        }
    }

    #[test]
    fn rejects_ambiguous_or_invalid_updates() {
        for source in [
            "",
            " \t\r\n",
            "// comment",
            "/* comment */",
            r#"{ "metadata": { this is not json! } }"#,
            "{",
            "[]",
            r#"{"metadata":null}"#,
            r#"{"metadata":{},"metadata":{}}"#,
            r#"{"metadata":{"visual-editor":{},"visual-editor":{}}}"#,
        ] {
            let source_file = std::sync::Arc::new(SourceFileInner::new(
                SourcePath::new("/test/slint-project.json"),
                source.into(),
            ));
            assert!(
                update_canvas_presets_edit(&source_file, None, &presets(), ByteFormat::Utf8)
                    .is_err()
            );
        }
        let source_file = std::sync::Arc::new(SourceFileInner::new(
            SourcePath::new("/test/slint-project.json"),
            "{}".into(),
        ));
        for preset in
            [CanvasPreset { width: -1, height: 10 }, CanvasPreset { width: 10, height: 0 }]
        {
            assert!(
                update_canvas_presets_edit(&source_file, None, &[preset], ByteFormat::Utf8)
                    .is_err()
            );
        }
    }
}
