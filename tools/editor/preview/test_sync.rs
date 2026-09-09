// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR
// LicenseRef-Slint-Software-3.0

//! Opt-in, revision-aware synchronization for editor system tests.
//!
//! The observer records lifecycle events at the preview boundaries. It never
//! starts a reload in response to a test request.

use std::collections::{BTreeMap, VecDeque};

const PROTOCOL_VERSION: u32 = 2;
const MAX_EVENTS: usize = 512;

#[derive(Clone, Debug, serde::Serialize)]
struct Event {
    cursor: u64,
    kind: &'static str,
    url: String,
    content: Option<String>,
    outcome: Option<&'static str>,
}

#[derive(Default)]
struct State {
    session: String,
    next_cursor: u64,
    events: VecDeque<Event>,
    writes: u64,
}

thread_local! {
    static STATE: std::cell::RefCell<State> = Default::default();
}

#[derive(serde::Deserialize)]
struct Request {
    id: u64,
    #[serde(default)]
    mode: Mode,
    #[serde(default)]
    after: u64,
    #[serde(default)]
    outcome: Option<String>,
    #[serde(default)]
    sources: BTreeMap<lsp_types::Url, String>,
}

#[derive(Clone, Copy, Default, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum Mode {
    #[default]
    Applied,
    Checkpoint,
    Processed,
    Settled,
}

fn session_id() -> String {
    format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    )
}

fn push(
    kind: &'static str,
    url: &lsp_types::Url,
    content: Option<&str>,
    outcome: Option<&'static str>,
) {
    STATE.with_borrow_mut(|state| {
        state.next_cursor += 1;
        if state.session.is_empty() {
            state.session = session_id();
        }
        state.events.push_back(Event {
            cursor: state.next_cursor,
            kind,
            url: url.to_string(),
            content: content.map(str::to_owned),
            outcome,
        });
        while state.events.len() > MAX_EVENTS {
            state.events.pop_front();
        }
    });
}

pub(crate) fn record_observed(url: &lsp_types::Url, content: Option<&str>) {
    push("observed", url, content, None);
}

pub(crate) fn record_processed(url: &lsp_types::Url, content: &str, outcome: &'static str) {
    push("processed", url, Some(content), Some(outcome));
}

pub(crate) fn record_applied(url: &lsp_types::Url, content: &str) {
    push("applied", url, Some(content), Some("compiled"));
}

pub(crate) fn record_write() {
    STATE.with_borrow_mut(|state| state.writes += 1);
}

fn response(request: &Request) -> serde_json::Value {
    STATE.with_borrow(|state| {
        let session = state.session.clone();
        let cursor = state.next_cursor;
        if matches!(request.mode, Mode::Checkpoint) {
            return serde_json::json!({
                "id": request.id,
                "protocol": PROTOCOL_VERSION,
                "session": session,
                "cursor": cursor,
                "writes": state.writes,
                "ready": true,
            });
        }

        let matching = |kind: &str| {
            request.sources.iter().all(|(url, expected)| {
                state.events.iter().any(|event| {
                    event.cursor > request.after
                        && event.kind == kind
                        && event.url == url.as_str()
                        && event.content.as_deref() == Some(expected.as_str())
                        && request.outcome.as_deref().is_none_or(|outcome| {
                            event.outcome == Some(outcome)
                        })
                })
            })
        };
        let ready = match request.mode {
            Mode::Processed => matching("processed"),
            Mode::Applied => matching("applied"),
            Mode::Settled => matching("applied"),
            Mode::Checkpoint => true,
        };
        serde_json::json!({
            "id": request.id,
            "protocol": PROTOCOL_VERSION,
            "session": session,
            "cursor": cursor,
            "writes": state.writes,
            "ready": ready,
            "overflow": request.after != 0
                && state.events.front().is_some_and(|event| request.after < event.cursor),
            "events": state.events.iter().filter(|event| event.cursor > request.after).collect::<Vec<_>>(),
        })
    })
}

pub(super) fn initialize() {
    let Some(directory) = std::env::var_os("SLINT_EDITOR_TEST_SYNC") else { return };
    let directory = std::path::PathBuf::from(directory);
    STATE.with_borrow_mut(|state| state.session = session_id());
    thread_local! {
        static TIMER: slint::Timer = slint::Timer::default();
    }
    TIMER.with(|timer| {
        timer.start(
            slint::TimerMode::Repeated,
            std::time::Duration::from_millis(20),
            move || {
                let Ok(bytes) = std::fs::read(directory.join("request.json")) else { return };
                let Ok(request) = serde_json::from_slice::<Request>(&bytes) else { return };
                let response = response(&request);
                let temporary = directory.join("response.tmp");
                if std::fs::write(&temporary, response.to_string()).is_ok() {
                    let _ = std::fs::rename(temporary, directory.join("response.json"));
                }
            },
        );
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle_events_are_revision_specific() {
        let url = lsp_types::Url::parse("file:///tmp/Main.slint").unwrap();
        STATE.with_borrow_mut(|state| *state = State::default());
        record_observed(&url, Some("old"));
        record_processed(&url, "old", "compile_error");
        record_observed(&url, Some("new"));
        record_processed(&url, "new", "compiled");
        record_applied(&url, "new");
        let request = Request {
            id: 1,
            mode: Mode::Processed,
            after: 1,
            outcome: Some("compiled".into()),
            sources: [(url.clone(), "new".into())].into(),
        };
        assert_eq!(response(&request)["ready"], true);
        let old_request = Request {
            id: 2,
            mode: Mode::Processed,
            after: 0,
            outcome: Some("compile_error".into()),
            sources: [(url, "old".into())].into(),
        };
        assert_eq!(response(&old_request)["ready"], true);
    }
}
