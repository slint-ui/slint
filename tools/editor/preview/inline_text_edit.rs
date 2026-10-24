// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::*;

pub(super) struct Session {
    pub(super) target: ElementSelection,
    initial_text: SharedString,
    original_color: slint::Brush,
    color_override: SmolStr,
    stroke_override: SmolStr,
    pending_edit: Option<(lsp_types::WorkspaceEdit, SharedString)>,
}

struct Target {
    selection: ElementSelection,
    text: SharedString,
    color: slint::Brush,
    color_override: SmolStr,
    stroke_override: SmolStr,
    scale: f32,
    grow_width: bool,
}

pub(super) fn setup(editor: &ui::EditorUi) {
    let global = editor.global::<ui::InlineTextEdit>();
    global.on_begin(begin);
    global.on_commit(commit);
}

fn source_text(node: &ElementRcNode) -> Option<SharedString> {
    node.with_element_node(|element| {
        if element.QualifiedName()?.text().to_string().trim() != "Text" {
            return None;
        }
        let binding = element.children().find(|child| {
            child.kind() == i_slint_compiler::parser::SyntaxKind::Binding
                && child.child_text(i_slint_compiler::parser::SyntaxKind::Identifier).as_deref()
                    == Some("text")
        })?;
        let expression = binding
            .child_node(i_slint_compiler::parser::SyntaxKind::BindingExpression)?
            .child_node(i_slint_compiler::parser::SyntaxKind::Expression)?;
        let literal = expression.child_text(i_slint_compiler::parser::SyntaxKind::StringLiteral)?;
        i_slint_compiler::literals::unescape_string(&literal)
            .map(|text| SharedString::from(text.as_str()))
    })
}

fn source_target(
    selection: &ElementSelection,
    instance: &ComponentInstance,
) -> Option<(ElementRcNode, SharedString, SmolStr, SmolStr)> {
    let node = selection.as_element_node_in(instance)?;
    let text = source_text(&node)?;
    if text.contains('\n') || text.contains('\r') {
        return None;
    }
    let hash = node.with_element_debug(|debug| debug.element_hash);
    if hash == 0 {
        return None;
    }
    let color_override = i_slint_compiler::passes::property_id(hash, &SmolStr::new_static("color"));
    let stroke_override =
        i_slint_compiler::passes::property_id(hash, &SmolStr::new_static("stroke"));
    Some((node, text, color_override, stroke_override))
}

pub(super) fn reseed_mask(instance: &ComponentInstance) {
    let target = PREVIEW_STATE.with_borrow(|state| {
        let session = state.text_edit_session.as_ref()?;
        source_target(&session.target, instance)
    });
    if let Some((_, _, color_override, stroke_override)) = target {
        PREVIEW_STATE.with_borrow_mut(|state| {
            if let Some(session) = state.text_edit_session.as_mut() {
                session.color_override = color_override.clone();
                session.stroke_override = stroke_override.clone();
                let mut overrides = (*state.debug_hook_overrides).borrow_mut();
                for override_id in [color_override, stroke_override] {
                    overrides.insert(
                        override_id,
                        Box::pin(i_slint_core::Property::new(Some(
                            slint_interpreter::Value::Brush(slint::Brush::default()),
                        ))),
                    );
                }
            }
        });
    }
}

fn target() -> Option<Target> {
    let selection = selected_element()?;
    let instance = component_instance()?;
    let (source_node, text, color_override, stroke_override) =
        source_target(&selection, &instance)?;
    let item = instance.component_item(
        &selection.path,
        selection.offset.into(),
        selection.instance_index,
        i_slint_core::InternalToken,
    )?;
    let geometry = instance
        .component_positions(&selection.path, selection.offset.into())
        .get(selection.instance_index)
        .copied()?;
    let size = item.geometry().size;
    let scale_x = if size.width > 0.0 { geometry.rect.width() / size.width } else { 1.0 };
    let scale_y = if size.height > 0.0 { geometry.rect.height() / size.height } else { 1.0 };
    let wrap = slint_interpreter::ComponentInstance::item_property(
        &item,
        "wrap",
        i_slint_core::InternalToken,
    );
    if !geometry.renders_as_rectangle
        || geometry.angle != 0.0
        || (scale_x - scale_y).abs() > 0.001
        || wrap.is_some_and(|wrap| {
            wrap != slint_interpreter::Value::from(i_slint_core::items::TextWrap::NoWrap)
        })
    {
        return None;
    }
    let slint_interpreter::Value::Brush(color) =
        slint_interpreter::ComponentInstance::item_property(
            &item,
            "color",
            i_slint_core::InternalToken,
        )?
    else {
        return None;
    };
    let available = PREVIEW_STATE
        .with_borrow(|state| (*state.debug_hook_overrides).borrow().contains_key(&color_override));
    if !available {
        return None;
    }
    let api = PREVIEW_STATE.with_borrow(|state| state.api.upgrade())?;
    let grow_width = !source_node.with_element_node(|element| {
        element.children().any(|child| {
            child.kind() == i_slint_compiler::parser::SyntaxKind::Binding
                && child.child_text(i_slint_compiler::parser::SyntaxKind::Identifier).as_deref()
                    == Some("width")
        })
    }) && api.get_selection().is_resizable;
    Some(Target {
        selection,
        text,
        color,
        color_override,
        stroke_override,
        scale: scale_y,
        grow_width,
    })
}

pub(super) fn notify_finished(accepted: bool) {
    let editor = PREVIEW_STATE
        .with_borrow(|state| state.editor_ui.as_ref().map(|editor| editor.clone_strong()));
    if let Some(editor) = editor
        && editor.global::<ui::InlineTextEdit>().invoke_finished(accepted)
    {
        editor.invoke_focus_editor();
    }
}

fn finish(accepted: bool, accepted_text: Option<SharedString>) {
    let session = PREVIEW_STATE.with_borrow_mut(|state| {
        if let Some(selection) = state.selected.as_mut() {
            selection.editing_text = false;
        }
        state.text_edit_session.take()
    });
    if let Some(session) = session {
        if let Some(text) = accepted_text
            && let Some(instance) = component_instance()
            && let Some((node, _, _, _)) = source_target(&session.target, &instance)
        {
            let hash = node.with_element_debug(|debug| debug.element_hash);
            let text_override =
                i_slint_compiler::passes::property_id(hash, &SmolStr::new_static("text"));
            set_override(&text_override, Some(slint_interpreter::Value::String(text)));
        }
        for override_id in [&session.color_override, &session.stroke_override] {
            set_override(override_id, None);
        }
    }
    refresh();
    notify_finished(accepted);
}

pub(super) fn selection_changed(
    selection: Option<&ElementSelection>,
    notification: &SelectionNotification,
) {
    let finish_session = PREVIEW_STATE.with_borrow(|state| {
        if selection.is_none()
            && *notification == SelectionNotification::Never
            && state.loading_state != PreviewFutureState::Pending
        {
            return false;
        }
        state.text_edit_session.as_ref().is_some_and(|session| {
            selection.is_none_or(|selection| !session.target.same_element(selection))
        })
    });
    if finish_session {
        discard();
    }
}

pub(super) fn refresh() {
    let ready =
        PREVIEW_STATE.with_borrow(|state| state.loading_state == PreviewFutureState::Pending);
    if !ready {
        let (editor, active, pending) = PREVIEW_STATE.with_borrow(|state| {
            (
                state.editor_ui.as_ref().map(|editor| editor.clone_strong()),
                state.text_edit_session.is_some(),
                state
                    .text_edit_session
                    .as_ref()
                    .is_some_and(|session| session.pending_edit.is_some()),
            )
        });
        if let Some(editor) = editor {
            let global = editor.global::<ui::InlineTextEdit>();
            global.set_active(active);
            global.set_commit_pending(pending);
        }
        return;
    }
    let target = target();
    let start = PREVIEW_STATE.with_borrow(|state| {
        state.text_edit_session.is_none()
            && state.selected.as_ref().is_some_and(|selection| selection.editing_text)
            && !undo_redo::edit_pending(state)
    });
    if start {
        if let Some(target) = &target {
            start_session(target);
        } else {
            PREVIEW_STATE.with_borrow_mut(|state| {
                if let Some(selection) = state.selected.as_mut() {
                    selection.editing_text = false;
                }
            });
        }
    }
    let (editor, pending, active, obsolete_override) = PREVIEW_STATE.with_borrow_mut(|state| {
        let pending =
            state.text_edit_session.as_ref().is_some_and(|session| session.pending_edit.is_some());
        let obsolete_override = if target.is_none() && !pending && !undo_redo::edit_pending(state) {
            if let Some(selection) = state.selected.as_mut() {
                selection.editing_text = false;
            }
            state
                .text_edit_session
                .take()
                .map(|session| (session.color_override, session.stroke_override))
        } else {
            None
        };
        if let Some(session) = state.text_edit_session.as_mut()
            && let Some(target) = &target
            && session.target.same_element(&target.selection)
        {
            session.color_override = target.color_override.clone();
            session.stroke_override = target.stroke_override.clone();
        }
        (
            state.editor_ui.as_ref().map(|editor| editor.clone_strong()),
            pending,
            state.text_edit_session.is_some(),
            obsolete_override,
        )
    });
    if let Some((color_override, stroke_override)) = &obsolete_override {
        for override_id in [color_override, stroke_override] {
            set_override(override_id, None);
        }
    }
    if active && let Some(target) = &target {
        for override_id in [&target.color_override, &target.stroke_override] {
            set_override(
                override_id,
                Some(slint_interpreter::Value::Brush(slint::Brush::default())),
            );
        }
    }
    if let Some(editor) = editor {
        let global = editor.global::<ui::InlineTextEdit>();
        global.set_can_edit(target.is_some());
        global.set_scale(target.as_ref().map(|target| target.scale).unwrap_or(1.0));
        global.set_grow_width(target.as_ref().is_some_and(|target| target.grow_width));
        global.set_active(active);
        global.set_commit_pending(pending);
    }
    if obsolete_override.is_some() {
        notify_finished(false);
    }
}

fn start_session(target: &Target) {
    PREVIEW_STATE.with_borrow_mut(|state| {
        if let Some(selection) = state.selected.as_mut() {
            selection.editing_text = true;
        }
        let mut selection = target.selection.clone();
        selection.editing_text = true;
        state.text_edit_session = Some(Session {
            target: selection,
            initial_text: target.text.clone(),
            original_color: target.color.clone(),
            color_override: target.color_override.clone(),
            stroke_override: target.stroke_override.clone(),
            pending_edit: None,
        });
    });
}

fn begin() -> ui::InlineTextEditStart {
    if let Some((text, color)) = PREVIEW_STATE.with_borrow(|state| {
        state
            .text_edit_session
            .as_ref()
            .map(|session| (session.initial_text.clone(), session.original_color.clone()))
    }) {
        return ui::InlineTextEditStart { success: true, text, color };
    }
    if file_edit_pending() {
        return ui::InlineTextEditStart::default();
    }
    let Some(target) = target() else { return ui::InlineTextEditStart::default() };
    let text = target.text.clone();
    let color = target.color.clone();
    start_session(&target);
    refresh();
    ui::InlineTextEditStart { success: true, text, color }
}

fn commit(text: SharedString) -> bool {
    let session = PREVIEW_STATE.with_borrow(|state| {
        state
            .text_edit_session
            .as_ref()
            .map(|session| (session.target.clone(), session.initial_text.clone()))
    });
    let Some((selection, initial_text)) = session else { return true };
    if PREVIEW_STATE.with_borrow(|state| {
        state.text_edit_session.as_ref().is_some_and(|session| session.pending_edit.is_some())
    }) {
        return true;
    }
    if text == initial_text {
        finish(true, None);
        return true;
    }
    if PREVIEW_STATE.with_borrow(|state| state.loading_state != PreviewFutureState::Pending) {
        return false;
    }
    let Some(document_cache) = document_cache() else { return false };
    let Some(url) = selection.path.to_url() else { return false };
    let current_source = PREVIEW_STATE.with_borrow(|state| {
        state.source_code.get(&url).is_none_or(|source| {
            document_cache
                .get_document(&url)
                .and_then(|document| document.node.as_ref())
                .and_then(|node| node.source_file.source())
                == Some(source.code.as_str())
        })
    });
    if !current_source {
        return false;
    }
    if document_cache.element_at_offset(&url, selection.offset).as_ref().and_then(source_text)
        == Some(text.clone())
    {
        finish(true, None);
        return true;
    }
    let version = document_cache.document_version(&url);
    let code = ui::convert_simple_string(text.clone()).to_string();
    let Some(edit) = properties::update_element_properties(
        &document_cache,
        i_slint_editor_preview::editing::VersionedPosition::new(
            VersionedUrl::new(url, version),
            selection.offset,
        ),
        vec![i_slint_editor_preview::editing::PropertyChange::new("text", code)],
    ) else {
        return false;
    };
    PREVIEW_STATE.with_borrow_mut(|state| {
        if let Some(session) = state.text_edit_session.as_mut() {
            session.pending_edit = Some((edit.clone(), text));
        }
    });
    let submitted = send_workspace_edit("Edit text".into(), edit, true);
    if !submitted {
        PREVIEW_STATE.with_borrow_mut(|state| {
            if let Some(session) = state.text_edit_session.as_mut() {
                session.pending_edit = None;
            }
        });
    }
    refresh();
    submitted
}

pub(super) fn workspace_edit_finished(edit: &lsp_types::WorkspaceEdit, applied: bool) -> bool {
    let submitted_text = PREVIEW_STATE.with_borrow_mut(|state| {
        let session = state.text_edit_session.as_mut()?;
        if session.pending_edit.as_ref().map(|(pending_edit, _)| pending_edit) != Some(edit) {
            return None;
        }
        let (_, text) = session.pending_edit.take()?;
        if !applied {
            state.workspace_edit_sent = false;
        }
        Some(text)
    });
    let Some(submitted_text) = submitted_text else { return false };
    if applied {
        finish(true, Some(submitted_text));
    } else {
        refresh();
        notify_finished(false);
    }
    true
}

pub(super) fn discard() {
    finish(false, None);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct PreviewToLsp(RefCell<Vec<PreviewToLspMessage>>);

    impl i_slint_editor_preview::PreviewToLsp for PreviewToLsp {
        fn send(&self, message: &PreviewToLspMessage) -> i_slint_editor_preview::Result<()> {
            self.0.borrow_mut().push(message.clone());
            Ok(())
        }
    }

    #[test]
    fn source_snapshot_and_color_mask_survive_session_changes() {
        let code = r#"
export component Win inherits Window {
    width: 200px;
    height: 100px;
    Text {
        width: 100px;
        height: 30px;
        text: "Source";
        font-italic: true;
        stroke: #ff8500;
        stroke-width: 1px;
        states [ changed when true: { text: "Runtime"; } ]
        child := Rectangle { x: 0px; y: 25px; width: 20px; height: 5px; background: #e600b3; }
    }
    plain := Text { text: "Plain"; }
}"#;
        let instance = super::super::test::interpret_test("fluent", code);
        let editor = ui::create_ui().unwrap();
        setup(&editor);
        let api = editor.global::<ui::Api>();
        api.set_selection(ui::Selection { highlight_index: 0, ..Default::default() });
        let path = i_slint_editor_preview::test::main_test_file_name();
        let offset = code.find("Text {").unwrap() as u32;
        let cache = i_slint_editor_preview::DocumentCache::new_from_raw_parts(
            instance.definition().raw_type_loader().unwrap(),
            None,
            Rc::new(RefCell::new(HashMap::from([(path.clone(), Some(24))]))),
            i_slint_editor_preview::ByteFormat::Utf8,
        );
        let to_lsp = Rc::new(PreviewToLsp::default());
        let overrides = PREVIEW_STATE.with_borrow_mut(|state| {
            *state = PreviewState::default();
            state.editor_ui = Some(editor.clone_strong());
            state.to_lsp.replace(Some(to_lsp.clone()));
            state.api = <ui::Api as slint::Global<'_, ui::EditorUi>>::as_weak(&api);
            state.handle.replace(Some(instance.clone_strong()));
            state.document_cache.replace(Some(Rc::new(cache)));
            state.selected = Some(ElementSelection {
                path: path.clone(),
                offset: offset.into(),
                instance_index: 0,
                editing_text: false,
            });
            state.debug_hook_overrides.clone()
        });
        install_debug_hook_callback(&instance, overrides);
        let runtime_property = |item: &i_slint_core::items::ItemRc, name: &str| {
            slint_interpreter::ComponentInstance::item_property(
                item,
                name,
                i_slint_core::InternalToken,
            )
        };
        let item = instance.component_item(&path, offset, 0, i_slint_core::InternalToken).unwrap();
        assert_eq!(
            runtime_property(&item, "text"),
            Some(slint_interpreter::Value::String("Runtime".into()))
        );
        let geometry = item.geometry();
        let slint_interpreter::Value::Brush(original_color) =
            runtime_property(&item, "color").unwrap()
        else {
            panic!("Text color is a brush")
        };
        assert_ne!(original_color, slint::Brush::default());
        let original_stroke = runtime_property(&item, "stroke").unwrap();
        assert_ne!(original_stroke, slint_interpreter::Value::Brush(slint::Brush::default()));
        let child_offset = (code.find("child := Rectangle").unwrap() + "child := ".len()) as u32;
        let child =
            instance.component_item(&path, child_offset, 0, i_slint_core::InternalToken).unwrap();
        let child_geometry = child.geometry();
        assert_eq!(
            slint_interpreter::ComponentInstance::item_property(
                &item,
                "wrap",
                i_slint_core::InternalToken
            ),
            Some(slint_interpreter::Value::from(i_slint_core::items::TextWrap::NoWrap))
        );
        let started = begin();
        assert!(started.success);
        assert_eq!(started.text, "Source");
        assert_eq!(started.color, original_color);
        assert_eq!(begin().color, original_color);
        assert_eq!(
            runtime_property(&item, "color"),
            Some(slint_interpreter::Value::Brush(slint::Brush::default()))
        );
        assert_eq!(
            runtime_property(&item, "stroke"),
            Some(slint_interpreter::Value::Brush(slint::Brush::default()))
        );
        assert!(child.is_visible());
        assert_eq!(child.geometry(), child_geometry);
        assert_eq!(item.geometry(), geometry);
        assert!(commit("Source".into()));
        assert_eq!(runtime_property(&item, "stroke"), Some(original_stroke.clone()));
        assert_eq!(
            runtime_property(&item, "color"),
            Some(slint_interpreter::Value::Brush(original_color.clone()))
        );
        assert_eq!(
            runtime_property(&item, "text"),
            Some(slint_interpreter::Value::String("Runtime".into()))
        );
        assert!(item.is_visible());
        assert!(!editor.global::<ui::InlineTextEdit>().get_active());
        assert!(begin().success);
        for loading_state in [
            PreviewFutureState::PreLoading,
            PreviewFutureState::Loading,
            PreviewFutureState::NeedsReload,
        ] {
            let selection = PREVIEW_STATE.with_borrow_mut(|state| {
                state.loading_state = loading_state;
                state.selected.take()
            });
            selection_changed(None, &SelectionNotification::Never);
            refresh();
            assert!(editor.global::<ui::InlineTextEdit>().get_active());
            assert!(!commit("Draft".into()));
            PREVIEW_STATE
                .with_borrow_mut(|state| state.loading_state = PreviewFutureState::Pending);
            super::super::preview_instance_ready();
            assert_eq!(selected_element(), selection);
        }
        let reloaded = super::super::test::reinterpret_test_with_sources(
            "fluent",
            HashMap::from([(
                path.clone(),
                code.replace("Runtime", "Reloaded")
                    .replace("\"Source\"", "\"Draft\"")
                    .replace("font-italic: true;", "font-italic: true; color: #1268ab;"),
            )]),
        );
        let overrides = PREVIEW_STATE.with_borrow(|state| state.debug_hook_overrides.clone());
        (*overrides).borrow_mut().clear();
        install_debug_hook_callback(&reloaded, overrides.clone());
        reseed_mask(&reloaded);
        let color_override = PREVIEW_STATE
            .with_borrow(|state| state.text_edit_session.as_ref().unwrap().color_override.clone());
        assert_eq!(
            (*overrides).borrow().get(&color_override).unwrap().as_ref().get(),
            Some(slint_interpreter::Value::Brush(slint::Brush::default()))
        );
        PREVIEW_STATE.with_borrow(|state| {
            state.handle.replace(Some(reloaded.clone_strong()));
            state.document_cache.replace(Some(Rc::new(
                i_slint_editor_preview::DocumentCache::new_from_raw_parts(
                    reloaded.definition().raw_type_loader().unwrap(),
                    None,
                    Rc::new(RefCell::new(HashMap::from([(path.clone(), Some(25))]))),
                    i_slint_editor_preview::ByteFormat::Utf8,
                ),
            )));
        });
        refresh();
        assert_eq!(begin().text, "Source");
        assert_eq!(begin().color, original_color);
        let item = reloaded.component_item(&path, offset, 0, i_slint_core::InternalToken).unwrap();
        assert_eq!(
            runtime_property(&item, "color"),
            Some(slint_interpreter::Value::Brush(slint::Brush::default()))
        );
        assert_eq!(
            runtime_property(&item, "stroke"),
            Some(slint_interpreter::Value::Brush(slint::Brush::default()))
        );
        assert!(item.is_visible());
        assert!(commit("Draft".into()));
        assert_eq!(runtime_property(&item, "stroke"), Some(original_stroke.clone()));
        let reloaded_color = slint::Brush::from(slint::Color::from_rgb_u8(0x12, 0x68, 0xab));
        assert_eq!(
            runtime_property(&item, "color"),
            Some(slint_interpreter::Value::Brush(reloaded_color.clone()))
        );
        assert!(!editor.global::<ui::InlineTextEdit>().get_active());
        assert!(to_lsp.0.borrow().is_empty());
        assert_eq!(
            runtime_property(&item, "text"),
            Some(slint_interpreter::Value::String("Reloaded".into()))
        );
        let started = begin();
        assert_eq!(started.text, "Draft");
        assert_eq!(started.color, reloaded_color);
        assert!(commit("Edited".into()));
        assert!(editor.global::<ui::InlineTextEdit>().get_active());
        assert!(editor.global::<ui::InlineTextEdit>().get_commit_pending());
        let edit = PREVIEW_STATE.with_borrow(|state| {
            state.text_edit_session.as_ref().unwrap().pending_edit.clone().unwrap().0
        });
        assert!(workspace_edit_finished(&edit, false));
        assert!(editor.global::<ui::InlineTextEdit>().get_active());
        assert!(!editor.global::<ui::InlineTextEdit>().get_commit_pending());
        assert_eq!(
            runtime_property(&item, "text"),
            Some(slint_interpreter::Value::String("Reloaded".into()))
        );
        assert_eq!(
            runtime_property(&item, "color"),
            Some(slint_interpreter::Value::Brush(slint::Brush::default()))
        );
        assert_eq!(
            runtime_property(&item, "stroke"),
            Some(slint_interpreter::Value::Brush(slint::Brush::default()))
        );
        assert!(item.is_visible());
        assert!(commit("Edited".into()));
        let edit = PREVIEW_STATE.with_borrow(|state| {
            state.text_edit_session.as_ref().unwrap().pending_edit.clone().unwrap().0
        });
        assert!(commit("Edited".into()));
        assert_eq!(to_lsp.0.borrow().len(), 2);
        assert!(workspace_edit_finished(&edit, true));
        assert_eq!(runtime_property(&item, "stroke"), Some(original_stroke.clone()));
        assert_eq!(
            runtime_property(&item, "color"),
            Some(slint_interpreter::Value::Brush(reloaded_color.clone()))
        );
        assert_eq!(
            runtime_property(&item, "text"),
            Some(slint_interpreter::Value::String("Reloaded".into()))
        );
        assert!(!editor.global::<ui::InlineTextEdit>().get_active());
        assert!(item.is_visible());
        PREVIEW_STATE.with_borrow_mut(|state| state.workspace_edit_sent = false);
        let cache = document_cache().unwrap();
        let source_file = &cache
            .get_document(&path.to_url().unwrap())
            .unwrap()
            .node
            .as_ref()
            .unwrap()
            .source_file;
        let plain_offset = (source_file.source().unwrap().find("plain := Text").unwrap()
            + "plain := ".len()) as u32;
        PREVIEW_STATE.with_borrow_mut(|state| {
            state.selected = Some(ElementSelection {
                path: path.clone(),
                offset: plain_offset.into(),
                instance_index: 0,
                editing_text: false,
            });
        });
        let plain_item =
            reloaded.component_item(&path, plain_offset, 0, i_slint_core::InternalToken).unwrap();
        assert_eq!(begin().text, "Plain");
        for applied in [false, true] {
            assert!(commit("Accepted plain".into()));
            assert_eq!(
                runtime_property(&plain_item, "text"),
                Some(slint_interpreter::Value::String("Plain".into()))
            );
            let edit = PREVIEW_STATE.with_borrow(|state| {
                state.text_edit_session.as_ref().unwrap().pending_edit.clone().unwrap().0
            });
            assert!(workspace_edit_finished(&edit, applied));
            let expected = if applied { "Accepted plain" } else { "Plain" };
            assert_eq!(
                runtime_property(&plain_item, "text"),
                Some(slint_interpreter::Value::String(expected.into()))
            );
        }
        assert!(plain_item.is_visible());
        PREVIEW_STATE.with_borrow_mut(|state| {
            state.workspace_edit_sent = false;
            state.selected = Some(ElementSelection {
                path: path.clone(),
                offset: offset.into(),
                instance_index: 0,
                editing_text: false,
            });
        });
        assert!(begin().success);
        PREVIEW_STATE
            .with_borrow_mut(|state| state.selected.as_mut().unwrap().offset = u32::MAX.into());
        refresh();
        assert!(!selected_element().unwrap().editing_text);
        assert!(!editor.global::<ui::InlineTextEdit>().get_active());
        assert!(item.is_visible());
        assert_eq!(
            runtime_property(&item, "color"),
            Some(slint_interpreter::Value::Brush(reloaded_color.clone()))
        );
        PREVIEW_STATE.with_borrow_mut(|state| {
            state.selected = Some(ElementSelection {
                path: path.clone(),
                offset: offset.into(),
                instance_index: 0,
                editing_text: false,
            });
        });
        refresh();
        assert!(!editor.global::<ui::InlineTextEdit>().get_active());
        assert!(begin().success);
        super::super::reset_project_state(path.to_url());
        let global = editor.global::<ui::InlineTextEdit>();
        assert!(!global.get_active());
        assert!(!global.get_commit_pending());
        assert!(!global.get_can_edit());
        assert_eq!(global.get_scale(), 1.0);
        assert!(!global.get_grow_width());
    }
}
