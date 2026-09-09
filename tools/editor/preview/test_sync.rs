// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Opt-in synchronization for editor system tests.
//!
//! This observer is process shared because source writes happen on the LSP
//! thread while requests are served on the preview UI thread. Events contain
//! compact identities; source text is never retained in the event ring.

use std::collections::{BTreeMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex, OnceLock};

const PROTOCOL_VERSION: u32 = 2;
const MAX_EVENTS: usize = 512;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ContentIdentity {
    hash: u64,
    len: u64,
}

fn content_identity(content: Option<&str>) -> Option<ContentIdentity> {
    content.map(|content| {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        content.hash(&mut hasher);
        ContentIdentity { hash: hasher.finish(), len: content.len() as u64 }
    })
}

#[derive(Clone, Debug, serde::Serialize)]
struct Event {
    cursor: u64,
    kind: &'static str,
    url: String,
    content_hash: Option<u64>,
    content_len: Option<u64>,
    outcome: Option<&'static str>,
    operation: Option<u64>,
    attempt: Option<u64>,
    inputs: Vec<(String, Option<i32>)>,
}

#[derive(Clone, Debug)]
struct Installed {
    url: String,
    content: Option<ContentIdentity>,
    cursor: u64,
    attempt: u64,
}

#[derive(Default)]
struct Observer {
    session: String,
    next_cursor: u64,
    discarded_through: u64,
    next_attempt: u64,
    next_operation: u64,
    events: VecDeque<Event>,
    installed: Option<Installed>,
    writes: u64,
    accepted_edits: u64,
    pending_operations: u64,
    active_operation: Option<u64>,
    gate_source: bool,
    gate_publication: bool,
    held_sources: Vec<(lsp_types::Url, String)>,
    held_publications: Vec<(lsp_types::Url, String)>,
}

static OBSERVER: OnceLock<Arc<Mutex<Observer>>> = OnceLock::new();

fn observer() -> Option<&'static Arc<Mutex<Observer>>> {
    OBSERVER.get()
}

fn enabled_observer() -> Option<&'static Arc<Mutex<Observer>>> {
    observer().filter(|_| cfg!(test) || std::env::var_os("SLINT_EDITOR_TEST_SYNC").is_some())
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
    observer: &mut Observer,
    kind: &'static str,
    url: &lsp_types::Url,
    content: Option<&str>,
    outcome: Option<&'static str>,
    operation: Option<u64>,
    attempt: Option<u64>,
) -> u64 {
    observer.next_cursor += 1;
    let cursor = observer.next_cursor;
    observer.events.push_back(Event {
        cursor,
        kind,
        url: url.to_string(),
        content_hash: content_identity(content).map(|value| value.hash),
        content_len: content_identity(content).map(|value| value.len),
        outcome,
        operation,
        attempt,
        inputs: Vec::new(),
    });
    while observer.events.len() > MAX_EVENTS {
        if let Some(event) = observer.events.pop_front() {
            observer.discarded_through = event.cursor;
        }
    }
    cursor
}

fn with_observer(mut callback: impl FnMut(&mut Observer)) {
    let Some(observer) = enabled_observer() else { return };
    callback(&mut observer.lock().unwrap());
}

pub(crate) fn record_observed(url: &lsp_types::Url, content: Option<&str>) {
    with_observer(|observer| {
        if observer.session.is_empty() {
            observer.session = session_id();
        }
        push(observer, "observed", url, content, None, None, None);
    });
}

pub(crate) fn record_processed_with_inputs(
    url: &lsp_types::Url,
    content: Option<&str>,
    outcome: &'static str,
    inputs: &std::collections::HashMap<std::path::PathBuf, Option<i32>>,
) -> u64 {
    let mut result = 0;
    with_observer(|observer| {
        let attempt = {
            observer.next_attempt += 1;
            observer.next_attempt
        };
        result = push(observer, "processed", url, content, Some(outcome), None, Some(attempt));
        if let Some(event) = observer.events.back_mut() {
            event.inputs = inputs
                .iter()
                .map(|(path, version)| (path.display().to_string(), *version))
                .collect();
        }
    });
    result
}

pub(crate) fn record_applied_with_attempt(
    url: &lsp_types::Url,
    content: &str,
    attempt: Option<u64>,
) {
    with_observer(|observer| {
        if observer.gate_publication {
            observer.held_publications.push((url.clone(), content.to_owned()));
            push(observer, "gate", url, Some(content), Some("publication_reached"), None, None);
            return;
        }
        record_applied_now(observer, url, content, attempt);
    });
}

fn record_applied_now(
    observer: &mut Observer,
    url: &lsp_types::Url,
    content: &str,
    attempt: Option<u64>,
) {
    let attempt = attempt.unwrap_or_else(|| {
        observer.next_attempt += 1;
        observer.next_attempt
    });
    let cursor =
        push(observer, "applied", url, Some(content), Some("compiled"), None, Some(attempt));
    observer.installed = Some(Installed {
        url: url.to_string(),
        content: content_identity(Some(content)),
        cursor,
        attempt,
    });
}

pub(crate) fn record_write() {
    with_observer(|observer| {
        observer.writes += 1;
        push(
            observer,
            "write",
            &lsp_types::Url::parse("about:workspace").unwrap(),
            None,
            Some("written"),
            observer.active_operation,
            None,
        );
    });
}

pub(crate) fn record_accepted_edit() -> u64 {
    let mut operation = 0;
    with_observer(|observer| {
        operation = observer.active_operation.unwrap_or_else(|| {
            observer.next_operation = observer.next_operation.saturating_add(1);
            observer.pending_operations += 1;
            observer.next_operation
        });
        observer.active_operation = Some(operation);
        observer.accepted_edits += 1;
        push(
            observer,
            "edit",
            &lsp_types::Url::parse("about:workspace").unwrap(),
            None,
            Some("accepted"),
            Some(operation),
            None,
        );
    });
    operation
}

pub(crate) fn record_edit_completed(operation: u64, outcome: &'static str) {
    with_observer(|observer| {
        observer.pending_operations = observer.pending_operations.saturating_sub(1);
        if observer.active_operation == Some(operation) {
            observer.active_operation = None;
        }
        push(
            observer,
            "action",
            &lsp_types::Url::parse("about:action").unwrap(),
            None,
            Some(outcome),
            Some(operation),
            None,
        );
    });
}

pub(crate) fn should_hold_source(url: &lsp_types::Url, content: &str) -> bool {
    let Some(observer) = enabled_observer() else { return false };
    let mut observer = observer.lock().unwrap();
    if !observer.gate_source {
        return false;
    }
    observer.held_sources.push((url.clone(), content.to_owned()));
    push(&mut observer, "gate", url, Some(content), Some("source_reached"), None, None);
    true
}

fn pump() {
    let Some(observer) = enabled_observer() else { return };
    let (sources, publications) = {
        let mut observer = observer.lock().unwrap();
        if observer.gate_source || observer.gate_publication {
            return;
        }
        (
            std::mem::take(&mut observer.held_sources),
            std::mem::take(&mut observer.held_publications),
        )
    };
    for (url, content) in sources {
        super::set_contents(&i_slint_live_preview::protocol::VersionedUrl::new(url, None), content);
    }
    if !publications.is_empty() {
        with_observer(|observer| {
            for (url, content) in &publications {
                record_applied_now(observer, url, content, None);
            }
        });
    }
}

fn matching(
    event: &Event,
    url: &lsp_types::Url,
    expected: Option<&str>,
    outcome: Option<&str>,
) -> bool {
    event.url == url.as_str()
        && event.content_hash == content_identity(expected).map(|value| value.hash)
        && event.content_len == content_identity(expected).map(|value| value.len)
        && outcome.is_none_or(|outcome| event.outcome == Some(outcome))
}

#[derive(serde::Deserialize)]
struct Request {
    id: u64,
    protocol: u32,
    session: Option<String>,
    mode: Mode,
    after: u64,
    operation: Option<u64>,
    outcome: Option<String>,
    sources: BTreeMap<lsp_types::Url, Option<String>>,
    gate: Option<String>,
    release: bool,
    gate_control: bool,
    begin_action: bool,
}

#[derive(Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum Mode {
    Handshake,
    Checkpoint,
    Processed,
    Applied,
    Settled,
    Gate,
    Finish,
}

fn response(request: &Request) -> serde_json::Value {
    let Some(observer) = observer() else {
        return serde_json::json!({ "id": request.id, "error": "observer disabled" });
    };
    let mut observer = observer.lock().unwrap();
    if request.protocol != PROTOCOL_VERSION && !matches!(request.mode, Mode::Handshake) {
        return serde_json::json!({ "id": request.id, "error": "protocol mismatch" });
    }
    if !matches!(request.mode, Mode::Handshake)
        && request.session.as_deref() != Some(observer.session.as_str())
    {
        return serde_json::json!({ "id": request.id, "error": "stale session" });
    }
    if request.after > observer.next_cursor {
        return serde_json::json!({ "id": request.id, "error": "future cursor" });
    }
    if request.after < observer.discarded_through {
        return serde_json::json!({ "id": request.id, "error": "event history overflow" });
    }
    if observer.session.is_empty() {
        observer.session = session_id();
    }
    if let Some(gate) = &request.gate {
        match gate.as_str() {
            "source" => observer.gate_source = !request.release,
            "publication" => observer.gate_publication = !request.release,
            _ => return serde_json::json!({ "id": request.id, "error": "unknown gate" }),
        }
    }
    let cursor = observer.next_cursor;
    if matches!(request.mode, Mode::Handshake) {
        return serde_json::json!({ "id": request.id, "protocol": PROTOCOL_VERSION, "session": observer.session, "cursor": cursor, "writes": observer.writes, "ready": true });
    }
    if matches!(request.mode, Mode::Checkpoint) && request.begin_action {
        observer.next_operation = observer.next_operation.saturating_add(1);
        observer.pending_operations += 1;
        observer.active_operation = Some(observer.next_operation);
    }
    if matches!(request.mode, Mode::Finish) {
        let Some(operation) = request.operation else {
            return serde_json::json!({ "id": request.id, "error": "missing operation" });
        };
        observer.pending_operations = observer.pending_operations.saturating_sub(1);
        let outcome = match request.outcome.as_deref() {
            Some("canceled") => Some("canceled"),
            Some("rejected") => Some("rejected"),
            Some("noop") => Some("noop"),
            Some("completed") => Some("completed"),
            _ => None,
        };
        push(
            &mut observer,
            "action",
            &lsp_types::Url::parse("about:action").unwrap(),
            None,
            outcome,
            Some(operation),
            None,
        );
        return serde_json::json!({ "id": request.id, "protocol": PROTOCOL_VERSION, "session": observer.session, "cursor": observer.next_cursor, "writes": observer.writes, "accepted_edits": observer.accepted_edits, "ready": true, "operation": operation });
    }
    let source_matches = |kind: &str| {
        request.sources.iter().all(|(url, expected)| {
            observer.events.iter().any(|event| {
                event.cursor > request.after
                    && event.kind == kind
                    && matching(event, url, expected.as_deref(), request.outcome.as_deref())
            })
        })
    };
    let current_matches = request.sources.iter().all(|(url, expected)| {
        observer.installed.as_ref().is_some_and(|installed| {
            installed.url == url.as_str()
                && installed.content == content_identity(expected.as_deref())
        })
    });
    let ready = match request.mode {
        Mode::Checkpoint => true,
        Mode::Processed => source_matches("processed"),
        Mode::Applied => current_matches && (request.after == 0 || source_matches("applied")),
        Mode::Settled => {
            current_matches
                && request.operation.is_some_and(|operation| {
                    observer.pending_operations == 0
                        && observer.events.iter().any(|event| {
                            event.operation == Some(operation)
                                && event.outcome == request.outcome.as_deref()
                        })
                })
        }
        Mode::Gate if request.gate_control => true,
        Mode::Gate => request.gate.as_deref().is_some_and(|gate| {
            observer.events.iter().any(|event| {
                event.kind == "gate"
                    && event.outcome
                        == Some(match gate {
                            "source" => "source_reached",
                            "publication" => "publication_reached",
                            _ => "",
                        })
            })
        }),
        Mode::Handshake | Mode::Finish => true,
    };
    serde_json::json!({
        "id": request.id,
        "protocol": PROTOCOL_VERSION,
        "session": observer.session,
        "cursor": cursor,
        "writes": observer.writes,
        "accepted_edits": observer.accepted_edits,
        "ready": ready,
        "events": observer.events.iter().filter(|event| event.cursor > request.after).take(32).collect::<Vec<_>>(),
        "installed_attempt": observer.installed.as_ref().map(|installed| installed.attempt),
        "installed_cursor": observer.installed.as_ref().map(|installed| installed.cursor),
        "operation": observer.active_operation,
    })
}

pub(super) fn initialize() {
    let Some(directory) = std::env::var_os("SLINT_EDITOR_TEST_SYNC") else { return };
    let observer = Arc::new(Mutex::new(Observer { session: session_id(), ..Default::default() }));
    let _ = OBSERVER.set(observer);
    let directory = std::path::PathBuf::from(directory);
    thread_local! { static TIMER: slint::Timer = slint::Timer::default(); }
    TIMER.with(|timer| {
        timer.start(slint::TimerMode::Repeated, std::time::Duration::from_millis(20), move || {
            pump();
            let Ok(bytes) = std::fs::read(directory.join("request.json")) else { return };
            let Ok(request) = serde_json::from_slice::<Request>(&bytes) else {
                let id = serde_json::from_slice::<serde_json::Value>(&bytes)
                    .ok()
                    .and_then(|value| value.get("id").and_then(serde_json::Value::as_u64))
                    .unwrap_or_default();
                let result = serde_json::json!({
                    "id": id,
                    "protocol": PROTOCOL_VERSION,
                    "error": "malformed request"
                });
                let temporary = directory.join("response.tmp");
                if std::fs::write(&temporary, result.to_string()).is_ok() {
                    let _ = std::fs::rename(temporary, directory.join("response.json"));
                }
                return;
            };
            let result = response(&request);
            let temporary = directory.join("response.tmp");
            if std::fs::write(&temporary, result.to_string()).is_ok() {
                let _ = std::fs::rename(temporary, directory.join("response.json"));
            }
        })
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_installation_and_shared_writes_are_authoritative() {
        let observer =
            Arc::new(Mutex::new(Observer { session: "test".into(), ..Default::default() }));
        let _ = OBSERVER.set(observer);
        let url = lsp_types::Url::parse("file:///tmp/Main.slint").unwrap();
        record_applied_with_attempt(&url, "A", None);
        record_applied_with_attempt(&url, "B", None);
        let req = Request {
            id: 1,
            protocol: 2,
            session: Some("test".into()),
            mode: Mode::Applied,
            after: 0,
            operation: None,
            outcome: None,
            sources: [(url, Some("A".into()))].into(),
            gate: None,
            release: false,
            gate_control: false,
            begin_action: false,
        };
        assert!(!response(&req)["ready"].as_bool().unwrap());
        let thread = std::thread::spawn(record_write);
        thread.join().unwrap();
        assert_eq!(
            response(&Request {
                id: 2,
                mode: Mode::Checkpoint,
                protocol: 2,
                session: Some("test".into()),
                after: 0,
                operation: None,
                outcome: None,
                sources: BTreeMap::new(),
                gate: None,
                release: false,
                gate_control: false,
                begin_action: false
            })["writes"],
            1
        );
    }
}
