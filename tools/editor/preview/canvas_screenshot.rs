// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::{PREVIEW_STATE, PreviewFutureState, PreviewState, ui};
use i_slint_compiler::diagnostics::{Diagnostic, DiagnosticLevel};
use lsp_types::Url;
use slint::ComponentHandle;
use slint_editor_mcp::EditorResponse;
use std::cell::Cell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant, SystemTime};

pub(super) struct PreviewAttempt {
    inputs: HashMap<Url, Option<String>>,
    resources: HashMap<PathBuf, Option<(SystemTime, u64)>>,
    error: Option<String>,
    pub installed: Rc<Cell<bool>>,
}

impl PreviewAttempt {
    pub fn new(
        inputs: HashMap<Url, Option<String>>,
        diagnostics: &[Diagnostic],
        compiled: Option<&slint_interpreter::ComponentDefinition>,
    ) -> Self {
        let errors = diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.level() == DiagnosticLevel::Error)
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        let error = if !errors.is_empty() {
            Some(format!("The latest preview failed to compile:\n{}", errors.join("\n")))
        } else if compiled.is_none() {
            Some("The latest compilation produced no preview component.".into())
        } else {
            None
        };
        let mut resources = PREVIEW_STATE.with_borrow(|state| {
            state
                .resources
                .iter()
                .filter_map(|url| url.to_file_path().ok())
                .map(|path| {
                    let stamp = resource_stamp(&path);
                    (path, stamp)
                })
                .collect::<HashMap<_, _>>()
        });
        if let Some(compiled) = compiled {
            resources.clear();
            let type_loader = compiled.type_loader();
            for url in inputs.keys() {
                let Some(document) = type_loader.get_document(&super::SourcePath::from_url(url))
                else {
                    continue;
                };
                for resource in document.embedded_file_resources.borrow().iter() {
                    let Some(path) = resource.path.as_ref().and_then(|path| path.as_native_path())
                    else {
                        continue;
                    };
                    resources.insert(path.into(), resource_stamp(path));
                }
            }
        }
        Self { inputs, resources, error, installed: Rc::new(Cell::new(false)) }
    }

    fn sources_are_current(&self, state: &PreviewState) -> bool {
        self.inputs.iter().all(|(url, compiled_source)| {
            let current = state.source_code.get(url);
            if current.map(|entry| &entry.code) != compiled_source.as_ref() {
                return false;
            }
            if current.is_some_and(|entry| entry.version.is_some()) {
                return true;
            }
            let Ok(path) = url.to_file_path() else { return true };
            disk_source(&path).as_ref() == compiled_source.as_ref()
        })
    }

    fn resources_are_current(&self) -> bool {
        self.resources.iter().all(|(path, compiled_stamp)| resource_stamp(path) == *compiled_stamp)
    }
}

fn disk_source(path: &Path) -> Option<String> {
    let source = std::fs::read_to_string(path).ok()?;
    if path.extension().is_some_and(|extension| extension == "rs") {
        i_slint_compiler::lexer::extract_rust_macro(source)
    } else {
        Some(source)
    }
}

fn resource_stamp(path: &Path) -> Option<(SystemTime, u64)> {
    let metadata = std::fs::metadata(path).ok()?;
    Some((metadata.modified().ok()?, metadata.len()))
}

struct Target {
    project_root: PathBuf,
    project_generation: u64,
    target_generation: u64,
}

impl Target {
    fn ready_editor(&self, state: &PreviewState) -> Result<Option<ui::EditorUi>, String> {
        if state.annotations.project_root != self.project_root
            || state.project_generation != self.project_generation
        {
            return Err("The editor changed projects while waiting for the screenshot.".into());
        }
        if state.screenshot_target_generation != self.target_generation {
            return Err("The preview target changed while waiting for the screenshot.".into());
        }
        if state.workspace_edit_sent
            || !state.pending_history.is_empty()
            || state.loading_state != PreviewFutureState::Pending
        {
            return Ok(None);
        }
        let Some(attempt) = &state.screenshot_preview else { return Ok(None) };
        if !attempt.sources_are_current(state) || !attempt.resources_are_current() {
            return Ok(None);
        }
        if let Some(error) = &attempt.error {
            return Err(error.clone());
        }
        if !attempt.installed.get() {
            return Ok(None);
        }
        state
            .editor_ui
            .as_ref()
            .map(|editor| Some(editor.clone_strong()))
            .ok_or_else(|| "The editor window is unavailable.".into())
    }
}

pub(super) fn request(
    project_root: PathBuf,
    sender: std::sync::mpsc::SyncSender<Result<EditorResponse, String>>,
) {
    let target = PREVIEW_STATE.with_borrow(|state| {
        if state.annotations.project_root != project_root {
            return Err("The editor changed projects. Discover editors again.".into());
        }
        if state.current_component().is_none() {
            return Err("The editor has no preview target.".into());
        }
        Ok(Target {
            project_root,
            project_generation: state.project_generation,
            target_generation: state.screenshot_target_generation,
        })
    });
    let result_sender = sender.clone();
    if let Err(error) = slint::spawn_local(async move {
        let result = match target {
            Ok(target) => capture_when_ready(target).await,
            Err(error) => Err(error),
        };
        let _ = result_sender.send(result);
    }) {
        let _ = sender.send(Err(error.to_string()));
    }
}

async fn capture_when_ready(target: Target) -> Result<EditorResponse, String> {
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut resource_reload = None;
    loop {
        if let Some(editor) = PREVIEW_STATE.with_borrow(|state| target.ready_editor(state))? {
            let result = super::canvas_screenshot(&editor)?;
            if PREVIEW_STATE.with_borrow(|state| target.ready_editor(state))?.is_some() {
                return Ok(result);
            }
        }
        let changed_resources = PREVIEW_STATE.with_borrow(|state| {
            let attempt = state.screenshot_preview.as_ref()?;
            if state.workspace_edit_sent
                || !state.pending_history.is_empty()
                || state.loading_state != PreviewFutureState::Pending
                || !attempt.sources_are_current(state)
                || attempt.resources_are_current()
            {
                return None;
            }
            let stamps = attempt
                .resources
                .keys()
                .map(|path| (path.clone(), resource_stamp(path)))
                .collect::<HashMap<_, _>>();
            Some((state.current_component()?, stamps))
        });
        if let Some((component, stamps)) = changed_resources
            && resource_reload.as_ref() != Some(&stamps)
        {
            resource_reload = Some(stamps);
            super::load_preview(component, super::LoadBehavior::Reload);
        }
        if Instant::now() >= deadline {
            return Err("Timed out waiting for the latest source files to compile and install in the canvas.".into());
        }
        let (sender, receiver) = tokio::sync::oneshot::channel();
        slint::Timer::single_shot(Duration::from_millis(20), move || {
            let _ = sender.send(());
        });
        receiver.await.map_err(|error| error.to_string())?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preview::SourceCodeCacheEntry;

    fn attempt(
        url: &Url,
        source: Option<&str>,
        error: Option<&str>,
        installed: bool,
    ) -> PreviewAttempt {
        PreviewAttempt {
            inputs: HashMap::from([(url.clone(), source.map(str::to_owned))]),
            resources: HashMap::new(),
            error: error.map(str::to_owned),
            installed: Rc::new(Cell::new(installed)),
        }
    }

    #[test]
    fn waits_for_disk_updates_queued_loading_superseded_and_installed_preview() {
        i_slint_backend_testing::init_no_event_loop();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("main.slint");
        std::fs::write(&path, "old").unwrap();
        let url = Url::from_file_path(&path).unwrap();
        let mut state =
            PreviewState { editor_ui: Some(ui::EditorUi::new().unwrap()), ..Default::default() };
        state.annotations.project_root = directory.path().into();
        state
            .source_code
            .insert(url.clone(), SourceCodeCacheEntry { version: None, code: "old".into() });
        state.screenshot_preview = Some(attempt(&url, Some("old"), None, true));
        let target = Target {
            project_root: directory.path().into(),
            project_generation: 0,
            target_generation: 0,
        };
        assert!(target.ready_editor(&state).unwrap().is_some());
        std::fs::write(&path, "new").unwrap();
        assert!(target.ready_editor(&state).unwrap().is_none());
        state.source_code.get_mut(&url).unwrap().code = "new".into();
        assert!(target.ready_editor(&state).unwrap().is_none());
        state.screenshot_preview = Some(attempt(&url, Some("new"), None, true));
        for loading_state in [
            PreviewFutureState::PreLoading,
            PreviewFutureState::Loading,
            PreviewFutureState::NeedsReload,
        ] {
            state.loading_state = loading_state;
            assert!(target.ready_editor(&state).unwrap().is_none());
        }
        state.loading_state = PreviewFutureState::Pending;
        state.screenshot_preview.as_ref().unwrap().installed.set(false);
        assert!(target.ready_editor(&state).unwrap().is_none());
        state.screenshot_preview.as_ref().unwrap().installed.set(true);
        assert!(target.ready_editor(&state).unwrap().is_some());
        state.screenshot_target_generation += 1;
        assert!(target.ready_editor(&state).err().unwrap().contains("target changed"));
        state.screenshot_target_generation = 0;
        state.project_generation += 1;
        assert!(target.ready_editor(&state).err().unwrap().contains("changed projects"));
    }

    #[test]
    fn reports_only_current_failures_and_preserves_unsaved_buffers() {
        i_slint_backend_testing::init_no_event_loop();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("main.slint");
        let url = Url::from_file_path(&path).unwrap();
        std::fs::write(&path, "broken").unwrap();
        let mut state =
            PreviewState { editor_ui: Some(ui::EditorUi::new().unwrap()), ..Default::default() };
        state.annotations.project_root = directory.path().into();
        state
            .source_code
            .insert(url.clone(), SourceCodeCacheEntry { version: None, code: "broken".into() });
        state.screenshot_preview =
            Some(attempt(&url, Some("broken"), Some("main.slint:1: Current error"), false));
        let target = Target {
            project_root: directory.path().into(),
            project_generation: 0,
            target_generation: 0,
        };
        assert!(target.ready_editor(&state).err().unwrap().contains("Current error"));
        std::fs::write(&path, "corrected").unwrap();
        assert!(target.ready_editor(&state).unwrap().is_none());
        state
            .source_code
            .insert(url.clone(), SourceCodeCacheEntry { version: Some(2), code: "unsaved".into() });
        state.screenshot_preview = Some(attempt(&url, Some("unsaved"), None, true));
        assert!(target.ready_editor(&state).unwrap().is_some());
        let missing = directory.path().join("missing.slint");
        let missing_url = Url::from_file_path(&missing).unwrap();
        state.screenshot_preview = Some(attempt(&missing_url, None, Some("Missing import"), false));
        assert!(target.ready_editor(&state).err().unwrap().contains("Missing import"));
        std::fs::write(missing, "New import").unwrap();
        assert!(target.ready_editor(&state).unwrap().is_none());
    }
}
