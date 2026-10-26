// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::ui;
use i_slint_compiler::diagnostics::SourceFile;
use i_slint_editor_preview::{ByteFormat, util};
use jsonc_parser::{ParseOptions, ast, common::Ranged};
use serde::{Deserialize, Serialize};
use slint::{ComponentHandle, Model};

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

pub(super) fn setup(editor: &ui::EditorUi) {
    let project = editor.global::<ui::Project>();
    project.on_update_canvas_preset(|index, width, height| {
        let proposal = (|| {
            let mut presets = current_presets()?;
            let preset = usize::try_from(index)
                .ok()
                .and_then(|index| presets.get_mut(index))
                .ok_or("Canvas preset no longer exists")?;
            *preset = CanvasPreset {
                width: width
                    .trim()
                    .parse()
                    .map_err(|_| "Canvas width must be a positive integer")?,
                height: height
                    .trim()
                    .parse()
                    .map_err(|_| "Canvas height must be a positive integer")?,
            };
            Ok(presets)
        })();
        submit_proposal(proposal)
    });
    project.on_add_canvas_preset(|| {
        submit_proposal(current_presets().map(|mut presets| {
            presets.push(CanvasPreset { width: 390, height: 720 });
            presets
        }))
    });
    project.on_select_canvas_preset(select);
}

fn presets_from_model(
    presets: slint::ModelRc<ui::CanvasPreset>,
) -> Result<Vec<CanvasPreset>, String> {
    presets
        .iter()
        .map(|preset| {
            if [preset.width, preset.height].into_iter().any(|dimension| {
                !dimension.is_finite()
                    || dimension <= 0.
                    || dimension.fract() != 0.
                    || f64::from(dimension) > f64::from(i32::MAX)
            }) {
                return Err("Canvas width and height must be positive integers".into());
            }
            Ok(CanvasPreset { width: preset.width as i32, height: preset.height as i32 })
        })
        .collect()
}

fn current_presets() -> Result<Vec<CanvasPreset>, String> {
    super::PREVIEW_STATE.with_borrow(|state| {
        let editor = state.editor_ui.as_ref().ok_or("No project is open")?;
        presets_from_model(editor.global::<ui::Project>().get_canvas_presets())
    })
}

fn submit_proposal(proposal: Result<Vec<CanvasPreset>, String>) -> bool {
    let result = proposal.and_then(|presets| update(&presets));
    if let Err(error) = &result {
        super::PREVIEW_STATE.with_borrow(|state| {
            if let Some(editor) = &state.editor_ui {
                editor.global::<ui::Project>().set_canvas_presets_error(error.into());
            }
        });
    }
    result.is_ok()
}

fn default_presets() -> Vec<CanvasPreset> {
    vec![CanvasPreset { width: 390, height: 720 }]
}

fn project_url(state: &super::PreviewState) -> Option<lsp_types::Url> {
    state.current_project_root.as_ref()?.join(i_slint_compiler::project_file::FILE_NAME).ok()
}

pub(super) fn reset() {
    super::PREVIEW_STATE.with_borrow(|state| {
        let Some(editor) = &state.editor_ui else { return };
        let project = editor.global::<ui::Project>();
        project.set_canvas_presets(preset_model(default_presets()));
        project.set_project_file_state(ui::ProjectFileState::Missing);
        project.set_canvas_presets_error(Default::default());
    });
    reload_settings();
}

pub(super) fn reload_settings() {
    let url = super::PREVIEW_STATE.with_borrow(|state| {
        let editor = state.editor_ui.as_ref()?;
        let root = state.current_project_root.as_ref()?.to_file_path().ok()?;
        editor
            .global::<ui::Project>()
            .set_selected_canvas_preset(state.settings.selected_canvas_preset(&root));
        project_url(state)
    });
    if let Some(url) = url {
        refresh(&url);
    }
}

pub(super) fn refresh(url: &lsp_types::Url) {
    let snapshot = super::PREVIEW_STATE.with_borrow(|state| {
        if project_url(state).as_ref() != Some(url) {
            return None;
        }
        Some((
            state.editor_ui.as_ref()?.clone_strong(),
            state.source_code.get(url).map(|source| source.code.clone()),
        ))
    });
    let Some((editor, source)) = snapshot else { return };
    let project = editor.global::<ui::Project>();
    let Some(source) = source else {
        project.set_project_file_state(ui::ProjectFileState::Missing);
        project.set_canvas_presets_error(Default::default());
        return;
    };
    if let Err(error) = i_slint_compiler::project_file::ProjectFile::parse(
        i_slint_compiler::source_path::SourcePath::from_url(url),
        source.as_bytes().to_vec(),
    ) {
        project.set_project_file_state(ui::ProjectFileState::Invalid);
        project.set_canvas_presets_error(error.to_string().into());
        return;
    }
    project.set_project_file_state(ui::ProjectFileState::Available);
    match read_canvas_presets(&source) {
        Ok(presets) => {
            project.set_canvas_presets_error(Default::default());
            publish_presets(&project, presets);
        }
        Err(error) => {
            project.set_canvas_presets_error(error.into());
            publish_presets(&project, default_presets());
        }
    }
}

fn preset_model(presets: Vec<CanvasPreset>) -> slint::ModelRc<ui::CanvasPreset> {
    std::rc::Rc::new(slint::VecModel::from(
        presets
            .into_iter()
            .map(|preset| ui::CanvasPreset {
                width: preset.width as f32,
                height: preset.height as f32,
            })
            .collect::<Vec<_>>(),
    ))
    .into()
}

fn publish_presets(project: &ui::Project<'_>, presets: Vec<CanvasPreset>) {
    let presets = if presets.is_empty() { default_presets() } else { presets };
    let model = preset_model(presets);
    if !project.get_canvas_presets().iter().eq(model.iter()) {
        project.set_canvas_presets(model);
    }
    select(project.get_selected_canvas_preset());
}

fn select(index: i32) {
    let update = super::PREVIEW_STATE.with_borrow_mut(|state| {
        let editor = state.editor_ui.as_ref()?;
        let project = editor.global::<ui::Project>();
        let last_index = project.get_canvas_presets().row_count().saturating_sub(1) as i32;
        let index = index.clamp(0, last_index);
        project.set_selected_canvas_preset(index);
        let root = state.current_project_root.as_ref()?.to_file_path().ok()?;
        state.settings.set_selected_canvas_preset(&root, index).then(|| state.settings.serialize())
    });
    if let Some(contents) = update {
        super::send_settings_update(contents);
    }
}

fn update(presets: &[CanvasPreset]) -> Result<(), String> {
    validate_presets(presets)?;
    let snapshot = super::PREVIEW_STATE.with_borrow(|state| {
        let editor = state.editor_ui.as_ref()?.clone_strong();
        let url = project_url(state)?;
        Some((
            editor,
            url.clone(),
            state.source_code.get(&url).map(|source| (source.version, source.code.clone())),
        ))
    });
    let (editor, url, source) = snapshot.ok_or("No project is open")?;
    let project = editor.global::<ui::Project>();
    if project.get_project_file_state() == ui::ProjectFileState::Invalid {
        return Err("Repair the project file before saving canvas presets".into());
    }
    let Some((version, source)) = source else {
        publish_presets(&project, presets.to_vec());
        project.set_canvas_presets_error(Default::default());
        return Ok(());
    };
    let source_file = std::sync::Arc::new(i_slint_compiler::diagnostics::SourceFileInner::new(
        i_slint_compiler::source_path::SourcePath::from_url(&url),
        source,
    ));
    let format = super::document_cache()
        .map_or(i_slint_editor_preview::ByteFormat::Utf8, |cache| cache.format);
    let edit = update_canvas_presets_edit(&source_file, version, presets, format)?;
    if !super::send_workspace_edit("Update canvas presets".into(), edit, true) {
        return Err("Could not submit canvas preset changes".into());
    }
    project.set_canvas_presets_error(Default::default());
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
                .set_canvas_presets_error("Could not save canvas presets".into());
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use i_slint_compiler::{diagnostics::SourceFileInner, source_path::SourcePath};

    fn project_editor(root: &std::path::Path) -> ui::EditorUi {
        let editor = crate::preview::tests::availability_test_editor();
        crate::preview::tests::apply_availability_test_source(
            &editor,
            "export component Main inherits Rectangle {}",
            true,
        );
        crate::preview::PREVIEW_STATE.with_borrow_mut(|state| {
            state.current_project_root = Some(lsp_types::Url::from_directory_path(root).unwrap())
        });
        setup(&editor);
        reset();
        editor
    }

    fn contents(widths: &[i32]) -> String {
        format!(
            "{{\"slint-edition\":\"1.19\",\"metadata\":{{\"visual-editor\":{}}}}}",
            serde_json::to_string(&VisualEditorSettings {
                canvas_presets: widths
                    .iter()
                    .map(|width| CanvasPreset { width: *width, height: 1080 })
                    .collect()
            })
            .unwrap()
        )
    }

    #[test]
    fn canvas_presets_follow_received_contents_and_missing_files_use_memory() {
        let directory = tempfile::tempdir().unwrap();
        let editor = project_editor(directory.path());
        let project = editor.global::<ui::Project>();
        let path = directory.path().join("slint-project.json");
        let url = lsp_types::Url::from_file_path(&path).unwrap();
        assert_eq!(project.get_project_file_state(), ui::ProjectFileState::Missing);
        assert!(submit_proposal(Ok(vec![CanvasPreset { width: 640, height: 480 }])));
        assert_eq!(project.get_canvas_presets().row_data(0).unwrap().width, 640.);
        assert!(!path.exists());
        crate::preview::set_contents(
            &i_slint_live_preview::protocol::VersionedUrl::new(url.clone(), Some(1)),
            contents(&[720, 1920]),
        );
        assert_eq!(project.get_project_file_state(), ui::ProjectFileState::Available);
        let proposed = vec![CanvasPreset { width: 800, height: 600 }];
        assert!(submit_proposal(Ok(proposed.clone())));
        assert!(!submit_proposal(Ok(proposed)));
        assert_eq!(project.get_canvas_presets().row_data(0).unwrap().width, 720.);
        crate::preview::set_contents(
            &i_slint_live_preview::protocol::VersionedUrl::new(url.clone(), Some(0)),
            contents(&[1440]),
        );
        assert_eq!(project.get_canvas_presets().row_data(0).unwrap().width, 1440.);
        for invalid in ["{", r#"{"unknown":true}"#, r#"{"main":42}"#] {
            crate::preview::set_contents(
                &i_slint_live_preview::protocol::VersionedUrl::new(url.clone(), None),
                invalid.into(),
            );
            assert_eq!(project.get_project_file_state(), ui::ProjectFileState::Invalid);
            assert_eq!(project.get_canvas_presets().row_data(0).unwrap().width, 1440.);
            assert!(!submit_proposal(Ok(default_presets())));
        }
        crate::preview::PREVIEW_STATE.with_borrow_mut(|state| state.workspace_edit_sent = false);
        crate::preview::set_contents(
            &i_slint_live_preview::protocol::VersionedUrl::new(url.clone(), None),
            r#"{"slint-edition":"1.19","metadata":{"visual-editor":null}}"#.into(),
        );
        assert_eq!(project.get_project_file_state(), ui::ProjectFileState::Available);
        assert!(!project.get_canvas_presets_error().is_empty());
        assert!(submit_proposal(Ok(default_presets())));
        crate::preview::set_contents(
            &i_slint_live_preview::protocol::VersionedUrl::new(url.clone(), None),
            contents(&[500]),
        );
        assert!(project.get_canvas_presets_error().is_empty());
        crate::preview::delete_document(&url);
        assert_eq!(project.get_project_file_state(), ui::ProjectFileState::Missing);
        assert_eq!(project.get_canvas_presets().row_data(0).unwrap().width, 500.);
        assert!(submit_proposal(Ok(default_presets())));
        assert!(!path.exists());
        assert!(!submit_proposal(Ok(vec![CanvasPreset { width: 0, height: 1 }])));
        let other = tempfile::tempdir().unwrap();
        crate::preview::reset_project_state(Some(
            lsp_types::Url::from_directory_path(other.path()).unwrap(),
        ));
        assert_eq!(project.get_project_file_state(), ui::ProjectFileState::Missing);
        assert_eq!(project.get_canvas_presets().row_data(0).unwrap().width, 390.);
    }

    #[test]
    fn preset_selection_survives_reopen_and_clamps_after_contents_arrive() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        let editor = project_editor(&root);
        let project = editor.global::<ui::Project>();
        let path = root.join("main.slint");
        std::fs::write(&path, "export component Main {}").unwrap();
        crate::preview::PREVIEW_STATE.with_borrow_mut(|state| {
            let mut recent =
                crate::preview::settings::Project::from_root(&root, &path, Some("Main".into()))
                    .unwrap();
            recent.selected_canvas_preset = 1;
            state.settings.add_recent_project(recent);
        });
        reload_settings();
        assert_eq!(project.get_selected_canvas_preset(), 1);
        let url = lsp_types::Url::from_file_path(root.join("slint-project.json")).unwrap();
        crate::preview::set_contents(
            &i_slint_live_preview::protocol::VersionedUrl::new(url.clone(), None),
            contents(&[720, 1920]),
        );
        assert_eq!(project.get_selected_canvas_preset(), 1);
        assert_eq!(project.get_canvas_size().width, 1920.);
        for (requested, expected) in [(-5, 0), (99, 1)] {
            project.invoke_select_canvas_preset(requested);
            assert_eq!(project.get_selected_canvas_preset(), expected);
        }
        let serialized =
            crate::preview::PREVIEW_STATE.with_borrow(|state| state.settings.serialize());
        crate::preview::reset_project_state(Some(
            lsp_types::Url::from_directory_path(&root).unwrap(),
        ));
        crate::preview::set_user_settings(
            crate::preview::settings::SETTINGS_FILE.into(),
            serialized,
        );
        assert_eq!(project.get_selected_canvas_preset(), 1);
        crate::preview::set_contents(
            &i_slint_live_preview::protocol::VersionedUrl::new(url.clone(), None),
            contents(&[720, 1920]),
        );
        assert_eq!(project.get_selected_canvas_preset(), 1);
        crate::preview::set_contents(
            &i_slint_live_preview::protocol::VersionedUrl::new(url, None),
            contents(&[720]),
        );
        assert_eq!(project.get_selected_canvas_preset(), 0);
        assert_eq!(
            crate::preview::PREVIEW_STATE
                .with_borrow(|state| state.settings.selected_canvas_preset(&root)),
            0
        );
    }

    #[test]
    fn preset_model_changes_only_with_values_or_project_reset() {
        let directory = tempfile::tempdir().unwrap();
        let editor = project_editor(directory.path());
        let project = editor.global::<ui::Project>();
        let url =
            lsp_types::Url::from_file_path(directory.path().join("slint-project.json")).unwrap();
        crate::preview::set_contents(
            &i_slint_live_preview::protocol::VersionedUrl::new(url.clone(), None),
            contents(&[720, 1920]),
        );
        let original = project.get_canvas_presets();
        crate::preview::set_contents(
            &i_slint_live_preview::protocol::VersionedUrl::new(url.clone(), None),
            format!("{}\n// comment", contents(&[720, 1920])),
        );
        assert_eq!(project.get_canvas_presets(), original);
        crate::preview::set_contents(
            &i_slint_live_preview::protocol::VersionedUrl::new(url, None),
            contents(&[720, 1280]),
        );
        assert_ne!(project.get_canvas_presets(), original);
        publish_presets(&project, default_presets());
        let defaults = project.get_canvas_presets();
        let other = tempfile::tempdir().unwrap();
        crate::preview::reset_project_state(Some(
            lsp_types::Url::from_directory_path(other.path()).unwrap(),
        ));
        assert_ne!(project.get_canvas_presets(), defaults);
        assert_eq!(project.get_canvas_presets().row_data(0).unwrap().width, 390.);
    }

    #[test]
    fn out_of_bounds_canvas_presets_use_struct_defaults() {
        let directory = tempfile::tempdir().unwrap();
        let editor = project_editor(directory.path());
        let project = editor.global::<ui::Project>();
        project.set_canvas_presets(preset_model(vec![CanvasPreset { width: 720, height: 1280 }]));
        for index in [-1, 1, 99] {
            project.set_selected_canvas_preset(index);
            assert_eq!(
                (project.get_canvas_size().width, project.get_canvas_size().height),
                (390., 720.)
            );
        }
        project.set_canvas_presets(Default::default());
        assert_eq!(
            (project.get_canvas_size().width, project.get_canvas_size().height),
            (390., 720.)
        );
        project.invoke_select_canvas_preset(99);
        assert_eq!(project.get_selected_canvas_preset(), 0);
    }

    #[test]
    fn identical_preset_update_does_not_block_a_subsequent_edit() {
        let directory = tempfile::tempdir().unwrap();
        project_editor(directory.path());
        let url =
            lsp_types::Url::from_file_path(directory.path().join("slint-project.json")).unwrap();
        crate::preview::set_contents(
            &i_slint_live_preview::protocol::VersionedUrl::new(url, None),
            contents(&[390]),
        );
        assert!(submit_proposal(Ok(vec![CanvasPreset { width: 390, height: 1080 }])));
        assert!(!crate::preview::PREVIEW_STATE.with_borrow(|state| state.workspace_edit_sent));
        assert!(submit_proposal(Ok(default_presets())));
        assert!(crate::preview::PREVIEW_STATE.with_borrow(|state| state.workspace_edit_sent));
    }

    #[test]
    fn fractional_ui_lengths_are_rejected() {
        for width in [1.5, f32::NAN, f32::INFINITY, 2147483648.] {
            let model = std::rc::Rc::new(slint::VecModel::from(vec![ui::CanvasPreset {
                width,
                height: 720.,
            }]))
            .into();
            assert!(presets_from_model(model).is_err());
        }
    }

    #[test]
    fn row_and_add_callbacks_submit_fresh_proposals() {
        let directory = tempfile::tempdir().unwrap();
        let editor = project_editor(directory.path());
        let project = editor.global::<ui::Project>();
        let original = project.get_canvas_presets();
        assert!(project.invoke_update_canvas_preset(0, "480".into(), "320".into()));
        assert_eq!(original.row_data(0).unwrap().width, 390.);
        assert_eq!(project.get_canvas_presets().row_data(0).unwrap().width, 480.);
        let url =
            lsp_types::Url::from_file_path(directory.path().join("slint-project.json")).unwrap();
        crate::preview::set_contents(
            &i_slint_live_preview::protocol::VersionedUrl::new(url.clone(), None),
            contents(&[720]),
        );
        let published = project.get_canvas_presets();
        assert!(project.invoke_update_canvas_preset(0, "800".into(), "600".into()));
        assert_eq!(published.row_data(0).unwrap().width, 720.);
        assert_eq!(project.get_canvas_presets().row_data(0).unwrap().width, 720.);
        crate::preview::PREVIEW_STATE.with_borrow_mut(|state| state.workspace_edit_sent = false);
        assert!(project.invoke_add_canvas_preset());
        assert_eq!(published.row_count(), 1);
        assert_eq!(project.get_canvas_presets().row_count(), 1);
        for (index, width, height) in
            [(0, "0", "1"), (0, "1.5", "1"), (0, "2147483648", "1"), (5, "1", "1")]
        {
            assert!(!project.invoke_update_canvas_preset(index, width.into(), height.into()));
            assert_eq!(project.get_canvas_presets().row_data(0).unwrap().width, 720.);
        }
    }

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
