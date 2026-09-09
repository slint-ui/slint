// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Opt-in synchronization for tests whose next action requires an installed source revision.
//! The system-testing transport exposes accessibility, not the preview's document cache.

use std::collections::BTreeMap;

#[derive(serde::Deserialize)]
struct Request {
    id: u64,
    sources: BTreeMap<lsp_types::Url, String>,
}

fn response(request: &Request) -> serde_json::Value {
    super::PREVIEW_STATE.with_borrow(|state| {
        let busy = state.workspace_edit_sent
            || !state.pending_history.is_empty()
            || !matches!(state.loading_state, super::PreviewFutureState::Pending);
        let cache = super::document_cache_from(state);
        let mismatches: Vec<_> = request
            .sources
            .iter()
            .filter_map(|(url, expected)| {
                let actual = cache
                    .as_ref()
                    .and_then(|cache| cache.get_document(url))
                    .and_then(|document| document.node.as_ref())
                    .and_then(|node| node.source_file.source());
                (actual != Some(expected.as_str())).then_some(url)
            })
            .collect();
        serde_json::json!({
            "id": request.id,
            "ready": !busy && mismatches.is_empty(),
            "busy": busy,
            "mismatches": mismatches,
        })
    })
}

pub(super) fn initialize() {
    let Some(directory) = std::env::var_os("SLINT_EDITOR_TEST_SYNC") else { return };
    let directory = std::path::PathBuf::from(directory);
    thread_local! {
        static TIMER: slint::Timer = slint::Timer::default();
    }
    TIMER.with(|timer| {
        timer.start(slint::TimerMode::Repeated, std::time::Duration::from_millis(20), move || {
            let Ok(bytes) = std::fs::read(directory.join("request.json")) else { return };
            let Ok(request) = serde_json::from_slice::<Request>(&bytes) else { return };
            let response = response(&request);
            let temporary = directory.join("response.tmp");
            if std::fs::write(&temporary, response.to_string()).is_ok() {
                let _ = std::fs::rename(temporary, directory.join("response.json"));
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waits_for_installed_source_and_pending_work() {
        use super::super::{PREVIEW_STATE, PreviewFutureState, PreviewState};
        use i_slint_editor_preview::test::{
            compile_test_with_sources, main_test_file_name, recompile_test_with_sources,
        };
        let url = lsp_types::Url::from_file_path(main_test_file_name()).unwrap();
        let old = "export component Test inherits Window { width: 100px; }";
        let new = "export component Test inherits Window { width: 200px; }";
        let old_cache =
            compile_test_with_sources("fluent", [(url.clone(), old.into())].into(), false);
        let new_cache =
            recompile_test_with_sources("fluent", [(url.clone(), new.into())].into(), false, false);
        let request = Request { id: 42, sources: [(url, new.into())].into() };
        PREVIEW_STATE.with_borrow_mut(|state| {
            *state = PreviewState::default();
            state.document_cache.replace(Some(std::rc::Rc::new(old_cache)));
        });
        assert_eq!(response(&request)["ready"], false);
        PREVIEW_STATE.with_borrow_mut(|state| {
            state.document_cache.replace(Some(std::rc::Rc::new(new_cache)));
            state.loading_state = PreviewFutureState::Loading;
        });
        assert_eq!(response(&request)["ready"], false);
        PREVIEW_STATE.with_borrow_mut(|state| {
            state.loading_state = PreviewFutureState::Pending;
            state.workspace_edit_sent = true;
        });
        assert_eq!(response(&request)["ready"], false);
        PREVIEW_STATE.with_borrow_mut(|state| {
            state.workspace_edit_sent = false;
            state.pending_history.push_back(false);
        });
        assert_eq!(response(&request)["ready"], false);
        PREVIEW_STATE.with_borrow_mut(|state| state.pending_history.clear());
        assert_eq!(response(&request)["ready"], true);
        assert_eq!(response(&request)["id"], 42);
        PREVIEW_STATE.with_borrow_mut(|state| *state = PreviewState::default());
    }
}
