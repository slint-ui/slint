// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::settings::DisplaySize;
use super::ui;
use i_slint_compiler::diagnostics::SourceFile;
use i_slint_editor_preview::{ByteFormat, util};
use jsonc_parser::{ParseOptions, ast, common::Ranged};
use slint::ComponentHandle;

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

fn validate_size(size: &DisplaySize) -> Result<(), String> {
    if size.width <= 0 || size.height <= 0 {
        return Err("Window width and height must be positive integers".into());
    }
    Ok(())
}

pub(super) fn read_window_size(source: &str) -> Result<DisplaySize, String> {
    let root = parse(source)?;
    let settings = property(object(&root, "Project file")?, "metadata")
        .and_then(|metadata| child_property(metadata, "metadata", "visual-editor"))?;
    let size = child_property(settings, "visual-editor", "window-size")?;
    // Read the first preset from projects saved by earlier editor versions.
    let size = match size {
        Some(size) => Some(size),
        None => child_property(settings, "visual-editor", "canvas-presets")?
            .map(|value| value.as_array().ok_or("'canvas-presets' must be an array"))
            .transpose()?
            .and_then(|array| array.elements.first()),
    };
    let Some(size) = size else { return Ok(DisplaySize::default()) };
    let size = object(size, "Window size")?;
    let dimension = |name| {
        property(size, name)?
            .and_then(ast::Value::as_number_lit)
            .and_then(|number| number.value.parse::<i32>().ok())
            .ok_or_else(|| format!("Window '{name}' must be a positive integer"))
    };
    let size = DisplaySize { width: dimension("width")?, height: dimension("height")? };
    validate_size(&size)?;
    Ok(size)
}

pub(super) fn update_window_size_edit(
    source_file: &SourceFile,
    version: Option<i32>,
    size: &DisplaySize,
    format: ByteFormat,
) -> Result<lsp_types::WorkspaceEdit, String> {
    validate_size(size)?;
    let source = source_file.source().ok_or("Project file has no contents")?;
    let size = serde_json::to_string(size).map_err(|error| error.to_string())?;
    let settings = format!("{{\"window-size\":{size}}}");
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
                Some(value) => {
                    let settings = object(value, "visual-editor")?;
                    match property(settings, "window-size")? {
                        Some(value) => (value.range().start..value.range().end, size),
                        None => {
                            let (base, child) = object_indentation(
                                source,
                                settings,
                                &format!("{property_indent}{indent}"),
                                indent,
                            );
                            insert_property(
                                source,
                                settings,
                                "window-size",
                                &size,
                                &base,
                                &child,
                                newline,
                            )
                        }
                    }
                }
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

pub(super) fn setup(editor: &ui::EditorUi) {
    editor.global::<ui::Project>().on_update_window_size(|width, height| {
        let result = (|| {
            let size = DisplaySize {
                width: super::displays::dimension(&width)?,
                height: super::displays::dimension(&height)?,
            };
            update(&size)
        })();
        super::PREVIEW_STATE.with_borrow(|state| {
            if let Some(editor) = &state.editor_ui {
                editor
                    .global::<ui::Project>()
                    .set_window_size_error(result.as_ref().err().map_or("", String::as_str).into());
            }
        });
        result.is_ok()
    });
}

fn project_url(state: &super::PreviewState) -> Option<lsp_types::Url> {
    state.current_project_root.as_ref()?.join(i_slint_compiler::project_file::FILE_NAME).ok()
}

fn publish_size(project: &ui::Project<'_>, size: DisplaySize) {
    project.set_window_size(slint::LogicalSize::new(size.width as f32, size.height as f32));
}

pub(super) fn reset() {
    super::PREVIEW_STATE.with_borrow(|state| {
        let Some(editor) = &state.editor_ui else { return };
        let project = editor.global::<ui::Project>();
        publish_size(&project, DisplaySize::default());
        project.set_project_file_state(ui::ProjectFileState::Missing);
        project.set_window_size_error(Default::default());
    });
    reload_settings();
}

pub(super) fn reload_settings() {
    let url = super::PREVIEW_STATE.with_borrow(project_url);
    if let Some(url) = url {
        refresh(&url);
    }
    super::displays::refresh();
}

pub(super) fn refresh(url: &lsp_types::Url) {
    let snapshot = super::PREVIEW_STATE.with_borrow(|state| {
        if project_url(state).as_ref() != Some(url) {
            return None;
        }
        Some((
            state.editor_ui.as_ref()?.clone_strong(),
            state.source_code.get(url).map(|s| s.code.clone()),
        ))
    });
    let Some((editor, source)) = snapshot else { return };
    let project = editor.global::<ui::Project>();
    let was_missing = project.get_project_file_state() == ui::ProjectFileState::Missing;
    let result = if let Some(source) = source {
        i_slint_compiler::project_file::ProjectFile::parse(
            i_slint_compiler::source_path::SourcePath::from_url(url),
            source.as_bytes().to_vec(),
        )
        .map_err(|error| error.to_string())
        .and_then(|_| read_window_size(&source))
    } else {
        project.set_project_file_state(ui::ProjectFileState::Missing);
        publish_size(&project, DisplaySize::default());
        project.set_window_size_error(Default::default());
        super::displays::refresh();
        return;
    };
    match result {
        Ok(size) => {
            project.set_project_file_state(ui::ProjectFileState::Available);
            project.set_window_size_error(Default::default());
            publish_size(&project, size);
            if was_missing {
                let current = super::PREVIEW_STATE.with_borrow(|state| {
                    Some((
                        state.current_project_root.clone()?,
                        state.current_previewed_component.clone()?,
                    ))
                });
                if let Some((root, component)) = current {
                    super::apply_project_to_file_tree(&root);
                    super::apply_preview_to_file_tree(&component);
                }
            }
        }
        Err(error) => {
            project.set_project_file_state(ui::ProjectFileState::Invalid);
            project.set_window_size_error(error.into());
        }
    }
    super::displays::refresh();
}

fn update(size: &DisplaySize) -> Result<(), String> {
    validate_size(size)?;
    let snapshot = super::PREVIEW_STATE.with_borrow(|state| {
        let url = project_url(state)?;
        Some((
            url.clone(),
            state.source_code.get(&url).map(|s| (s.version, s.code.clone())),
            state.current_previewed_component.as_ref().map(|component| component.url.clone()),
        ))
    });
    let (url, source, component) = snapshot.ok_or("No project is open")?;
    let Some((version, source)) = source else {
        let root = url.to_file_path().map_err(|_| "Project is not a local directory")?;
        let main = component
            .and_then(|url| url.to_file_path().ok())
            .and_then(|path| pathdiff::diff_paths(path, root.parent()?))
            .ok_or("No main file is selected")?;
        let contents = serde_json::json!({
            "edition": "1.19", "main": main,
            "metadata": { "visual-editor": { "window-size": size } }
        });
        let edit = lsp_types::WorkspaceEdit {
            changes: Some(
                [(
                    url,
                    vec![lsp_types::TextEdit {
                        range: lsp_types::Range::default(),
                        new_text: format!("{contents:#}\n"),
                    }],
                )]
                .into(),
            ),
            ..Default::default()
        };
        return super::PREVIEW_STATE.with_borrow(|state| {
            state
                .to_lsp
                .borrow()
                .as_ref()
                .ok_or("No editor session")?
                .send(&super::PreviewToLspMessage::SendWorkspaceEdit {
                    label: Some("Set project window size".into()),
                    edit,
                })
                .map_err(|error| error.to_string())
        });
    };
    let source_file = std::sync::Arc::new(i_slint_compiler::diagnostics::SourceFileInner::new(
        i_slint_compiler::source_path::SourcePath::from_url(&url),
        source,
    ));
    let format = super::document_cache().map_or(ByteFormat::Utf8, |cache| cache.format);
    let edit = update_window_size_edit(&source_file, version, size, format)?;
    if !super::send_workspace_edit("Set project window size".into(), edit, true) {
        return Err("Could not save project window size".into());
    }
    Ok(())
}

pub(super) fn workspace_edit_finished(edit: &lsp_types::WorkspaceEdit, applied: bool) {
    if applied {
        return;
    }
    super::PREVIEW_STATE.with_borrow(|state| {
        let Some(url) = project_url(state) else { return };
        if i_slint_editor_preview::editing::text_edit::EditIterator::new(edit)
            .any(|(document, _)| document.uri == url)
            && let Some(editor) = &state.editor_ui
        {
            editor
                .global::<ui::Project>()
                .set_window_size_error("Could not save project window size".into());
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use i_slint_compiler::diagnostics::SourceFileInner;

    fn size() -> DisplaySize {
        DisplaySize { width: 720, height: 1280 }
    }

    #[test]
    fn edits_preserve_surrounding_source() {
        // cspell:ignore metad deditor

        let settings = r#"{"window-size":{"width":720,"height":1280}}"#;
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
                        i_slint_editor_preview::test::test_file_name("slint-project.json"),
                        source.clone(),
                    ));
                    let edit =
                        update_window_size_edit(&source_file, Some(7), &size(), format).unwrap();
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
                    assert_eq!(read_window_size(&result).unwrap(), size());
                }
            }
        }
    }

    #[test]
    fn absent_settings_and_invalid_settings() {
        for source in ["{}", r#"{"metadata":{}}"#, r#"{"metadata":{"visual-editor":{}}}"#] {
            assert_eq!(read_window_size(source).unwrap(), DisplaySize::default());
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
            r#"{"metadata":{"visual-editor":{"window-size":{},"window-size":{}}}}"#,
            r#"{"metadata":{},"metadata":{}}"#,
            r#"{"metadata":{"visual-editor":{},"visual-editor":{}}}"#,
            r#"{"metadata":{"visual-editor":{"window-size":null}}}"#,
            r#"{"metadata":{"visual-editor":{"window-size":{"width":0,"height":2}}}}"#,
            r#"{"metadata":{"visual-editor":{"window-size":{"width":1.5,"height":2}}}}"#,
            r#"{"metadata":{"visual-editor":{"window-size":{"width":2147483648,"height":2}}}}"#,
            r#"{"metadata":{"visual-editor":{"window-size":{"height":2}}}}"#,
        ] {
            assert!(read_window_size(source).is_err(), "{source}");
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
                i_slint_editor_preview::test::test_file_name("slint-project.json"),
                source.into(),
            ));
            assert!(
                update_window_size_edit(&source_file, None, &size(), ByteFormat::Utf8).is_err()
            );
        }
        let source_file = std::sync::Arc::new(SourceFileInner::new(
            i_slint_editor_preview::test::test_file_name("slint-project.json"),
            "{}".into(),
        ));
        for size in [DisplaySize { width: -1, height: 10 }, DisplaySize { width: 10, height: 0 }] {
            assert!(update_window_size_edit(&source_file, None, &size, ByteFormat::Utf8).is_err());
        }
    }
}
