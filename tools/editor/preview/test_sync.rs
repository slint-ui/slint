// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Private lifecycle observation for system tests.
//! Causal leases cross the UI/LSP queues and outlive deferred compilation and publication.

use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};
use std::future::Future;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex, OnceLock, Weak};

use lsp_types::Url;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub(crate) const PROTOCOL_VERSION: u32 = 4;
const MAX_EVENTS: usize = 512;
const MAX_COMPLETED: usize = 128;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
struct Content {
    hash: u64,
    len: usize,
}
fn identity(content: Option<&str>) -> Option<Content> {
    content.map(|text| {
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        text.hash(&mut hash);
        Content { hash: hash.finish(), len: text.len() }
    })
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct Input {
    observation: u64,
    version: Option<i32>,
    content: Option<Content>,
}
#[derive(Clone, Debug, Serialize)]
struct Attempt {
    id: u64,
    kind: String,
    root: Url,
    component: Option<String>,
    started_cursor: u64,
    inputs: BTreeMap<Url, Input>,
    outcome: Option<String>,
    diagnostics: Vec<String>,
    processed_cursor: u64,
}
#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Outcome {
    #[default]
    Noop,
    Canceled,
    Rejected,
    Completed,
    Failed,
}
#[derive(Default, Serialize)]
struct Operation {
    sealed: bool,
    pending: BTreeMap<u64, String>,
    accepted_edits: u64,
    writes: u64,
    mutations: u64,
    outcome: Outcome,
}
impl Operation {
    fn outcome(&self) -> &str {
        match self.outcome {
            Outcome::Noop => "noop",
            Outcome::Canceled => "canceled",
            Outcome::Rejected => "rejected",
            Outcome::Completed => "completed",
            Outcome::Failed => "failed",
        }
    }

    fn settled(&self) -> bool {
        self.sealed && self.pending.is_empty()
    }
}
#[derive(Serialize)]
struct Gate {
    kind: String,
    url: Url,
    after: u64,
    reached: Option<u64>,
    attempt: Option<u64>,
    edit: Option<u64>,
    released: bool,
}
#[derive(Default)]
struct Observer {
    session: String,
    ui_thread: Option<std::thread::ThreadId>,
    cursor: u64,
    discarded_through: u64,
    serial: u64,
    events: VecDeque<Value>,
    inputs: BTreeMap<Url, Input>,
    attempts: BTreeMap<u64, Attempt>,
    installed: Option<(u64, u64)>,
    mounted: bool,
    operations: BTreeMap<u64, Operation>,
    capturing: Option<u64>,
    gates: BTreeMap<u64, Gate>,
    write_faults: BTreeMap<Url, crate::source_write::WriteFault>,
    request: Option<(u64, Value, Option<Value>)>,
    writes: u64,
    mutations: u64,
    accepted_edits: u64,
}
impl Observer {
    fn prune(&mut self) {
        let settled: Vec<_> =
            self.operations.iter().filter(|(_, op)| op.settled()).map(|(id, _)| *id).collect();
        for id in settled.iter().take(settled.len().saturating_sub(MAX_COMPLETED)) {
            self.operations.remove(id);
        }
        let released: Vec<_> =
            self.gates.iter().filter(|(_, g)| g.released).map(|(id, _)| *id).collect();
        for id in released.iter().take(released.len().saturating_sub(MAX_COMPLETED)) {
            self.gates.remove(id);
        }
        let finished: Vec<_> = self
            .attempts
            .iter()
            .filter(|(id, a)| {
                a.outcome.is_some()
                    && self.installed.is_none_or(|(installed, _)| installed != **id)
                    && !self.gates.values().any(|g| !g.released && g.attempt == Some(**id))
            })
            .map(|(id, _)| *id)
            .collect();
        for id in finished.iter().take(finished.len().saturating_sub(MAX_COMPLETED)) {
            self.attempts.remove(id);
        }
    }

    fn id(&mut self) -> u64 {
        self.serial += 1;
        self.serial
    }
    fn event(&mut self, mut value: Value) -> u64 {
        self.cursor += 1;
        value["cursor"] = self.cursor.into();
        self.events.push_back(value);
        if self.events.len() > MAX_EVENTS {
            self.discarded_through = self.events.pop_front().unwrap()["cursor"].as_u64().unwrap();
        }
        self.cursor
    }
}
static STATE: OnceLock<Arc<Mutex<Observer>>> = OnceLock::new();
static GATE_CHANGED: tokio::sync::Notify = tokio::sync::Notify::const_new();
fn with_state<R>(f: impl FnOnce(&mut Observer) -> R) -> Option<R> {
    STATE.get().map(|s| f(&mut s.lock().unwrap()))
}
thread_local! {
    static CACHE_INPUTS: RefCell<BTreeMap<Url, Input>> = const { RefCell::new(BTreeMap::new()) };
    static CONTEXT: RefCell<Option<Vec<u64>>> = const { RefCell::new(None) };
    static PUBLICATIONS: RefCell<Vec<(u64, Work, Box<dyn FnOnce()>)>> = const { RefCell::new(Vec::new()) };
}

#[derive(Default, Clone)]
pub(crate) struct Work(Vec<Arc<Lease>>);
struct Lease {
    observer: Weak<Mutex<Observer>>,
    operation: u64,
    id: u64,
}
impl Drop for Lease {
    fn drop(&mut self) {
        if let Some(observer) = self.observer.upgrade()
            && let Some(op) = observer.lock().unwrap().operations.get_mut(&self.operation)
        {
            op.pending.remove(&self.id);
        }
    }
}
struct Scope(Option<Vec<u64>>);
impl Drop for Scope {
    fn drop(&mut self) {
        CONTEXT.with_borrow_mut(|c| *c = self.0.take());
    }
}
fn operations() -> Vec<u64> {
    CONTEXT.with_borrow(|c| c.clone()).unwrap_or_else(|| {
        with_state(|s| {
            if s.ui_thread == Some(std::thread::current().id()) {
                s.capturing.into_iter().collect()
            } else {
                Vec::new()
            }
        })
        .unwrap_or_default()
    })
}
impl Work {
    pub(crate) fn capture(stage: &str) -> Self {
        STATE.get().map(|state| Self::capture_in(state, operations(), stage)).unwrap_or_default()
    }
    fn capture_in(observer: &Arc<Mutex<Observer>>, ids: Vec<u64>, stage: &str) -> Self {
        let mut s = observer.lock().unwrap();
        Self(
            ids.into_iter()
                .map(|operation| {
                    let id = s.id();
                    s.operations.get_mut(&operation).unwrap().pending.insert(id, stage.into());
                    Arc::new(Lease { observer: Arc::downgrade(observer), operation, id })
                })
                .collect(),
        )
    }
    pub(crate) fn combine(work: Vec<Self>) -> Self {
        Self(work.into_iter().flat_map(|w| w.0).collect())
    }
    fn enter(&self) -> Scope {
        let mut ids: Vec<_> = self.0.iter().map(|l| l.operation).collect();
        ids.sort_unstable();
        ids.dedup();
        Scope(CONTEXT.with_borrow_mut(|c| c.replace(ids)))
    }
    pub(crate) fn run<R>(&self, f: impl FnOnce() -> R) -> R {
        let _scope = self.enter();
        f()
    }
    pub(crate) async fn during<F: Future>(&self, future: F) -> F::Output {
        let mut future = std::pin::pin!(future);
        std::future::poll_fn(|cx| {
            let _scope = self.enter();
            future.as_mut().poll(cx)
        })
        .await
    }
}

pub(crate) fn effect(outcome: Outcome) {
    let ids = operations();
    with_state(|s| {
        for id in ids {
            let op = s.operations.get_mut(&id).unwrap();
            op.outcome = op.outcome.max(outcome);
            s.event(json!({"kind":"effect", "operation":id, "outcome":outcome}));
        }
    });
}
pub(crate) fn accepted_edit() {
    let ids = operations();
    with_state(|s| {
        s.accepted_edits += 1;
        for id in ids {
            let op = s.operations.get_mut(&id).unwrap();
            op.accepted_edits += 1;
            op.outcome = op.outcome.max(Outcome::Completed);
        }
        s.event(json!({"kind":"accepted_edit"}));
    });
}
pub(crate) fn take_write_fault(url: &Url) -> Option<crate::source_write::WriteFault> {
    with_state(|s| s.write_faults.remove(url)).flatten()
}

pub(crate) fn mutated(url: &Url) {
    let ids = operations();
    with_state(|s| {
        s.mutations += 1;
        for id in &ids {
            s.operations.get_mut(id).unwrap().mutations += 1;
        }
        s.event(json!({"kind":"mutation", "url":url, "operations":ids}));
    });
}

pub(crate) fn written(url: &Url) {
    let ids = operations();
    with_state(|s| {
        s.writes += 1;
        for id in &ids {
            s.operations.get_mut(id).unwrap().writes += 1;
        }
        s.event(json!({"kind":"write", "url":url, "operations":ids}));
    });
}

pub(crate) fn begin_attempt(root: &Url, component: Option<String>) -> u64 {
    with_state(|s| {
        let id = s.id();
        let started_cursor = s.event(json!({"kind":"attempt_started", "attempt":id, "url":root}));
        s.attempts.insert(
            id,
            Attempt {
                id,
                kind: "compilation".into(),
                root: root.clone(),
                component,
                started_cursor,
                inputs: BTreeMap::new(),
                outcome: None,
                diagnostics: Vec::new(),
                processed_cursor: 0,
            },
        );
        id
    })
    .unwrap_or(0)
}
pub(crate) fn observed_read(url: &Url, content: Option<&str>) {
    with_state(|s| {
        let content = identity(content);
        let observation = s.event(json!({"kind":"observed", "url":url, "content":content}));
        let input = Input { observation, version: None, content };
        s.inputs.insert(url.clone(), input.clone());
        if input.content.is_none() {
            let id = s.id();
            let processed_cursor =
                s.event(json!({"kind":"processed", "attempt":id, "outcome":"load_error"}));
            s.attempts.insert(
                id,
                Attempt {
                    id,
                    kind: "load".into(),
                    root: url.clone(),
                    component: None,
                    started_cursor: observation,
                    inputs: BTreeMap::from([(url.clone(), input)]),
                    outcome: Some("load_error".into()),
                    diagnostics: vec![format!("Unable to load {url}")],
                    processed_cursor,
                },
            );
        }
    });
}

pub(crate) fn source_delivery(
    message: &i_slint_live_preview::protocol::LspToPreviewMessage,
) -> Option<(Url, Input)> {
    use i_slint_live_preview::protocol::LspToPreviewMessage as M;
    let (url, version, content) = match message {
        M::SetContents { url, contents } => {
            (url.url(), *url.version(), identity(std::str::from_utf8(contents).ok()))
        }
        M::ForgetFile { url } => (url, None, None),
        _ => return None,
    };
    with_state(|s| {
        s.inputs
            .get(url)
            .filter(|i| i.content == content)
            .map(|i| (url.clone(), Input { version, ..i.clone() }))
    })
    .flatten()
}

pub(crate) fn install_source(input: Option<(Url, Input)>) {
    if let Some((url, input)) = input {
        CACHE_INPUTS.with_borrow_mut(|c| {
            c.insert(url, input);
        });
    }
}

pub(crate) fn read_input(attempt: u64, url: &Url, version: Option<i32>, content: Option<&str>) {
    let content = identity(content);
    let input = CACHE_INPUTS.with_borrow(|c| c.get(url).filter(|i| i.content == content).cloned());
    with_state(|s| {
        if let Some(a) = s.attempts.get_mut(&attempt) {
            a.inputs
                .insert(url.clone(), input.unwrap_or(Input { observation: 0, version, content }));
        }
    });
}

pub(crate) fn processed(attempt: u64, outcome: &str, diagnostics: Vec<String>) {
    with_state(|s| {
        let cursor = s.event(json!({"kind":"processed", "attempt":attempt, "outcome":outcome}));
        if let Some(a) = s.attempts.get_mut(&attempt) {
            a.outcome = Some(outcome.into());
            a.processed_cursor = cursor;
            a.diagnostics = diagnostics;
        }
    });
}
pub(crate) fn unmounted(attempt: Option<u64>) {
    with_state(|s| {
        if s.mounted && attempt.is_none_or(|a| s.installed.is_none_or(|(id, _)| id <= a)) {
            s.mounted = false;
            s.event(json!({"kind":"unmounted"}));
        }
    });
}

pub(crate) fn applied(attempt: u64) {
    with_state(|s| {
        let cursor = s.event(json!({"kind":"applied", "attempt":attempt}));
        s.installed = Some((attempt, cursor));
        s.mounted = true;
    });
}

fn claim_gate(kind: &str, url: &Url, attempt: Option<u64>) -> Option<u64> {
    with_state(|s| {
        let id = s.gates.iter().find_map(|(id, g)| {
            (!g.released
                && g.kind == kind
                && &g.url == url
                && (kind == "source"
                    || (g.reached.is_none()
                        && (kind == "acknowledgment"
                            || attempt
                                .and_then(|id| s.attempts.get(&id))
                                .is_some_and(|a| a.started_cursor > g.after)))))
            .then_some(*id)
        })?;
        if s.gates[&id].reached.is_none() {
            let cursor =
                s.event(json!({"kind":"gate_reached", "gate":id, "attempt":attempt, "url":url}));
            let gate = s.gates.get_mut(&id).unwrap();
            gate.reached = Some(cursor);
            gate.attempt = attempt;
        }
        Some(id)
    })
    .flatten()
}
pub(crate) fn hold_source(url: &Url) -> Option<u64> {
    claim_gate("source", url, None)
}
pub(crate) fn gate_released(id: u64) -> bool {
    with_state(|s| s.gates.get(&id).is_none_or(|g| g.released)).unwrap_or(true)
}
pub(crate) async fn gate_changed() {
    GATE_CHANGED.notified().await;
}
pub(crate) fn acknowledge(urls: &[Url], id: u64, outcome: super::WorkspaceEditOutcome) {
    let gate = urls.iter().find_map(|url| claim_gate("acknowledgment", url, None));
    let callback = move || super::workspace_edit_result(id, outcome);
    if let Some(gate) = gate {
        with_state(|s| s.gates.get_mut(&gate).unwrap().edit = Some(id));
        PUBLICATIONS.with_borrow_mut(|p| {
            p.push((gate, Work::capture("acknowledgment gate"), Box::new(callback)))
        });
    } else {
        callback();
    }
}

pub(crate) fn acknowledgment_received(id: u64, accepted: bool) {
    with_state(|s| {
        s.event(json!({"kind":"acknowledgment", "edit":id, "accepted":accepted}));
    });
}

pub(crate) fn hold_factory(attempt: u64, callback: impl FnOnce() + 'static) -> bool {
    let root = with_state(|s| s.attempts.get(&attempt).map(|a| a.root.clone())).flatten();
    let Some(gate) = root.and_then(|url| claim_gate("factory", &url, Some(attempt))) else {
        return false;
    };
    PUBLICATIONS.with_borrow_mut(|p| p.push((gate, Work::default(), Box::new(callback))));
    true
}

pub(crate) fn publish(url: &Url, attempt: u64, callback: impl FnOnce() + 'static) {
    if let Some(gate) = claim_gate("publication", url, Some(attempt)) {
        PUBLICATIONS.with_borrow_mut(|p| {
            p.push((gate, Work::capture("publication gate"), Box::new(callback)))
        });
    } else {
        callback();
    }
}
fn pump_publications() {
    let ready = PUBLICATIONS.with_borrow_mut(|p| {
        let mut ready = Vec::new();
        let mut held = Vec::new();
        for item in p.drain(..) {
            if gate_released(item.0) {
                ready.push(item);
            } else {
                held.push(item);
            }
        }
        *p = held;
        ready
    });
    for (_, work, callback) in ready {
        work.run(callback);
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    id: u64,
    protocol: u32,
    session: Option<String>,
    mode: String,
    #[serde(default)]
    after: u64,
    #[serde(default)]
    sources: BTreeMap<Url, Option<String>>,
    outcome: Option<String>,
    operation: Option<u64>,
    gate: Option<u64>,
    edit: Option<u64>,
    fault: Option<crate::source_write::WriteFault>,
    kind: Option<String>,
    url: Option<Url>,
}
fn answer(s: &mut Observer, r: &Request) -> Result<Value, String> {
    if r.protocol != PROTOCOL_VERSION {
        return Err("protocol mismatch".into());
    }
    if r.mode != "handshake" && r.session.as_deref() != Some(&s.session) {
        return Err("stale session".into());
    }
    if r.after > s.cursor {
        return Err("future cursor".into());
    }
    if matches!(r.mode.as_str(), "observed" | "processed" | "events" | "acknowledgment")
        && r.after < s.discarded_through
    {
        return Err("event history overflow".into());
    }
    let mut ready = true;
    let mut result = json!({});
    match r.mode.as_str() {
        "handshake" => {
            result["build"] = json!({"revision": env!("SLINT_EDITOR_BUILD_REVISION"), "features": env!("SLINT_EDITOR_BUILD_FEATURES")});
        }
        "checkpoint" => {}
        "begin" => {
            if s.capturing.is_some() {
                return Err("an input action is already open".into());
            }
            let id = s.id();
            s.operations.insert(id, Operation::default());
            s.capturing = Some(id);
            result["operation"] = id.into();
        }
        "seal" => {
            let id = r.operation.ok_or("missing operation")?;
            let op = s.operations.get_mut(&id).ok_or("unknown or expired operation")?;
            op.sealed = true;
            if s.capturing == Some(id) {
                s.capturing = None;
            }
        }
        "settled" | "operation" => {
            let id = r.operation.ok_or("missing operation")?;
            let op = s.operations.get(&id).ok_or("unknown or expired operation")?;
            ready = r.mode == "operation" || op.settled();
            result["settled"] = op.settled().into();
            result["operation"] = id.into();
            result["operation_state"] = json!(op);
            result["outcome"] = op.outcome().into();
            if op.settled() && r.outcome.as_deref().is_some_and(|o| o != op.outcome()) {
                return Err(format!(
                    "operation {id}: expected {:?}, got {}",
                    r.outcome,
                    op.outcome()
                ));
            }
        }
        "acknowledgment" => {
            let id = r.edit.ok_or("missing edit ID")?;
            let found = s.events.iter().rev().find(|event| {
                event["kind"] == "acknowledgment"
                    && event["edit"] == id
                    && event["cursor"].as_u64().is_some_and(|cursor| cursor > r.after)
            });
            ready = found.is_some();
            result["acknowledgment"] = json!(found);
        }
        "observed" => {
            let found = s.events.iter().rev().find(|e| {
                e["kind"] == "observed"
                    && e["cursor"].as_u64().is_some_and(|c| c > r.after)
                    && r.sources.iter().all(|(url, c)| {
                        e["url"] == url.as_str() && e["content"] == json!(identity(c.as_deref()))
                    })
            });
            ready = found.is_some();
            result["observation"] = json!(found);
        }
        "processed" => {
            let found = s.attempts.values().rev().find(|a| {
                a.processed_cursor > r.after
                    && a.outcome.is_some()
                    && r.sources.iter().all(|(url, c)| {
                        a.inputs.get(url).is_some_and(|i| {
                            i.content == identity(c.as_deref())
                                && (r.after == 0 || i.observation > r.after)
                        })
                    })
                    && r.outcome.as_deref().is_none_or(|o| a.outcome.as_deref() == Some(o))
            });
            ready = found.is_some();
            result["attempt"] = json!(found);
        }
        "applied" => {
            let found = s.installed.and_then(|(id, cursor)| {
                (cursor > r.after || r.after == 0).then(|| s.attempts.get(&id)).flatten()
            });
            ready = s.mounted
                && found.is_some_and(|a| {
                    r.sources.iter().all(|(url, c)| {
                        a.inputs.get(url).is_some_and(|i| {
                            i.content == identity(c.as_deref())
                                && (r.after == 0 || i.observation > r.after)
                        })
                    })
                });
            ready &= super::PREVIEW_STATE.with_borrow(|p| {
                p.pending_edit.is_none()
                    && p.pending_history.is_empty()
                    && matches!(p.loading_state, super::PreviewFutureState::Pending)
            });
            result["attempt"] = json!(found);
        }
        "write_fault" => {
            let url = r.url.clone().ok_or("missing fault URL")?;
            let fault = r.fault.ok_or("missing write fault")?;
            if s.write_faults.contains_key(&url) {
                return Err("overlapping write fault".into());
            }
            s.write_faults.insert(url, fault);
        }
        "clear_write_fault" => {
            s.write_faults.remove(&r.url.clone().ok_or("missing fault URL")?);
        }
        "gate_open" => {
            let kind = r.kind.clone().ok_or("missing gate kind")?;
            if !matches!(kind.as_str(), "source" | "publication" | "acknowledgment" | "factory") {
                return Err("unknown gate kind".into());
            }
            let url = r.url.clone().ok_or("missing gate URL")?;
            if s.gates.values().any(|g| !g.released && g.kind == kind && g.url == url) {
                return Err("overlapping gate".into());
            }
            let id = s.id();
            s.gates.insert(
                id,
                Gate {
                    kind,
                    url,
                    after: s.cursor,
                    reached: None,
                    attempt: None,
                    edit: None,
                    released: false,
                },
            );
            result["gate"] = id.into();
        }
        "gate_release" | "gate_wait" => {
            let id = r.gate.ok_or("missing gate")?;
            let g = s.gates.get_mut(&id).ok_or("unknown or expired gate")?;
            if r.mode == "gate_release" {
                g.released = true;
                GATE_CHANGED.notify_one();
            } else {
                ready = g.reached.is_some_and(|c| c > g.after && c > r.after);
            }
            result["gate"] = id.into();
            result["gate_state"] = json!(g);
        }
        "events" => {}
        _ => return Err("unknown mode".into()),
    }
    result["ready"] = ready.into();
    Ok(result)
}
fn respond_in(s: &mut Observer, raw: Value) -> Value {
    let id = raw.get("id").cloned().unwrap_or(Value::Null);
    let Ok(request) = serde_json::from_value::<Request>(raw.clone()) else {
        return json!({"id":id,"error":"malformed request"});
    };
    let immutable = matches!(
        request.mode.as_str(),
        "handshake"
            | "checkpoint"
            | "begin"
            | "seal"
            | "gate_open"
            | "gate_release"
            | "write_fault"
            | "clear_write_fault"
    );
    if let Some((previous_id, previous, response)) = &s.request {
        if request.id < *previous_id {
            return json!({"id":id,"error":"stale request ID"});
        }
        if request.id == *previous_id {
            if previous != &raw {
                return json!({"id":id,"error":"request ID reused"});
            }
            if let Some(response) = response {
                return response.clone();
            }
        }
    }
    let mut result = match answer(s, &request) {
        Ok(v) => v,
        Err(e) => json!({"error":e}),
    };
    result["id"] = id;
    result["protocol"] = PROTOCOL_VERSION.into();
    result["session"] = s.session.clone().into();
    result["cursor"] = s.cursor.into();
    result["writes"] = s.writes.into();
    result["accepted_edits"] = s.accepted_edits.into();
    result["mutations"] = s.mutations.into();
    result["mounted"] = json!(s.mounted);
    result["installed"] = json!(s.installed.and_then(|(id, _)| s.attempts.get(&id)));
    result["operations"] =
        json!(s.operations.iter().filter(|(_, op)| !op.settled()).collect::<BTreeMap<_, _>>());
    result["attempts"] = json!(s.attempts.values().rev().take(16).collect::<Vec<_>>());
    result["events"] = json!(
        s.events
            .iter()
            .filter(|e| e["cursor"].as_u64().unwrap() > request.after)
            .collect::<Vec<_>>()
    );
    s.request = Some((request.id, raw, immutable.then(|| result.clone())));
    s.prune();
    result
}

fn respond(raw: Value) -> Value {
    with_state(|s| respond_in(s, raw.clone()))
        .unwrap_or_else(|| json!({"id":raw["id"],"error":"observer disabled"}))
}

pub(crate) fn initialize() {
    let Some(directory) = std::env::var_os("SLINT_EDITOR_TEST_SYNC") else { return };
    let session = format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    );
    let _ = STATE.set(Arc::new(Mutex::new(Observer {
        session,
        ui_thread: Some(std::thread::current().id()),
        ..Default::default()
    })));
    let directory = std::path::PathBuf::from(directory);
    thread_local! {static TIMER:slint::Timer=slint::Timer::default();}
    TIMER.with(|timer| {
        timer.start(slint::TimerMode::Repeated, std::time::Duration::from_millis(20), move || {
            pump_publications();
            let Ok(bytes) = std::fs::read(directory.join("request.json")) else { return };
            let raw = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
            let response = respond(raw);
            let temporary = directory.join("response.tmp");
            if std::fs::write(&temporary, response.to_string()).is_ok() {
                let _ = std::fs::rename(temporary, directory.join("response.json"));
            }
        })
    });
}

static WATCH_WORK: OnceLock<Mutex<BTreeMap<Url, Vec<Work>>>> = OnceLock::new();
pub(crate) fn expect_watch(url: &Url) {
    let work = Work::capture("filesystem observation");
    if work.0.is_empty() {
        return;
    }
    WATCH_WORK
        .get_or_init(Default::default)
        .lock()
        .unwrap()
        .entry(url.clone())
        .or_default()
        .push(work);
}
pub(crate) fn observed_write(url: &Url) -> Work {
    Work::combine(WATCH_WORK.get().and_then(|w| w.lock().unwrap().remove(url)).unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(id: u64, mode: &str) -> Value {
        json!({"id":id,"protocol":PROTOCOL_VERSION,"session":"test","mode":mode})
    }
    fn observer() -> Observer {
        Observer { session: "test".into(), ..Default::default() }
    }

    #[test]
    fn completed_records_expire_without_discarding_pending_work() {
        let mut state = observer();
        state.operations.insert(1, Operation::default());
        for id in 2..=(MAX_COMPLETED as u64 + 3) {
            state.operations.insert(id, Operation { sealed: true, ..Default::default() });
        }
        state.prune();
        assert!(state.operations.contains_key(&1));
        assert!(!state.operations.contains_key(&2));
        assert_eq!(state.operations.len(), MAX_COMPLETED + 1);
        let mut query = request(1000, "operation");
        query["operation"] = 2.into();
        assert_eq!(respond_in(&mut state, query)["error"], "unknown or expired operation");
    }

    // cspell:ignore nocapture
    #[test]
    fn installation_errors_are_fatal_with_or_without_a_publication_gate() {
        const CHILD: &str = "SLINT_TEST_INSTALLATION_FAILURE";
        if let Ok(mode) = std::env::var(CHILD) {
            i_slint_backend_testing::init_no_event_loop();
            STATE.set(Arc::new(Mutex::new(observer()))).ok().unwrap();
            let url: Url = "file:///Main.slint".parse().unwrap();
            let mut open = request(1, "gate_open");
            open["kind"] = "publication".into();
            open["url"] = json!(url);
            let gate = (mode == "deferred").then(|| respond(open)["gate"].clone());
            let attempt = begin_attempt(&url, None);
            publish(&url, attempt, || {
                super::super::finish_preview_installation(Err(
                    "injected installation failure".into()
                ));
            });
            println!("publication held");
            let mut release = request(2, "gate_release");
            release["gate"] = gate.unwrap();
            respond(release);
            pump_publications();
            panic!("installation failure did not terminate the editor");
        }
        for mode in ["immediate", "deferred"] {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "preview::test_sync::tests::installation_errors_are_fatal_with_or_without_a_publication_gate", "--nocapture"])
                .env(CHILD, mode)
                .output().unwrap();
            assert_eq!(
                output.status.code(),
                Some(3),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(
                String::from_utf8_lossy(&output.stdout).contains("publication held"),
                mode == "deferred"
            );
        }
    }

    #[test]
    fn older_requests_cannot_run_again() {
        let mut state = observer();
        respond_in(&mut state, request(1, "checkpoint"));
        respond_in(&mut state, request(2, "checkpoint"));
        assert_eq!(respond_in(&mut state, request(1, "begin"))["error"], "stale request ID");
        assert!(state.operations.is_empty());
    }

    #[test]
    fn observational_requests_recheck_state_but_cannot_change_payload() {
        let mut state = observer();
        state.operations.insert(7, Operation::default());
        let mut query = request(1, "settled");
        query["operation"] = 7.into();
        assert_eq!(respond_in(&mut state, query.clone())["ready"], false);
        state.operations.get_mut(&7).unwrap().sealed = true;
        assert_eq!(respond_in(&mut state, query.clone())["ready"], true);
        query["operation"] = 8.into();
        assert_eq!(respond_in(&mut state, query)["error"], "request ID reused");
    }

    #[test]
    fn repeated_control_requests_are_idempotent() {
        let mut state = observer();
        let begin = request(1, "begin");
        let first = respond_in(&mut state, begin.clone());
        state.event(json!({"kind":"unrelated"}));
        assert_eq!(first, respond_in(&mut state, begin));
        assert_eq!(state.operations.len(), 1);
        assert_eq!(respond_in(&mut state, request(1, "checkpoint"))["error"], "request ID reused");
        let mut gate = request(2, "gate_open");
        gate["kind"] = "publication".into();
        gate["url"] = "file:///Main.slint".into();
        let opened = respond_in(&mut state, gate.clone());
        assert_eq!(opened, respond_in(&mut state, gate));
        assert_eq!(state.gates.len(), 1);
    }

    #[test]
    fn accumulated_outcomes_preserve_precedence_in_both_orders() {
        use Outcome::*;
        let outcomes = [Noop, Canceled, Rejected, Completed, Failed];
        for (index, outcome) in outcomes.iter().enumerate() {
            let expected = ["noop", "canceled", "rejected", "completed", "failed"][index];
            for previous in &outcomes[..=index] {
                for pair in [[*previous, *outcome], [*outcome, *previous]] {
                    let mut operation = Operation::default();
                    for effect in pair {
                        operation.outcome = operation.outcome.max(effect);
                    }
                    assert_eq!(operation.outcome(), expected);
                }
            }
        }
    }

    #[test]
    fn settlement_cannot_manufacture_a_canceled_result() {
        let mut state = observer();
        let id = respond_in(&mut state, request(1, "begin"))["operation"].clone();
        let mut seal = request(2, "seal");
        seal["operation"] = id.clone();
        respond_in(&mut state, seal);
        let mut wait = request(3, "settled");
        wait["operation"] = id.clone();
        wait["outcome"] = "canceled".into();
        assert!(respond_in(&mut state, wait)["error"].as_str().unwrap().contains("got noop"));
        assert_eq!(state.operations[&id.as_u64().unwrap()].outcome(), "noop");
    }

    #[test]
    fn leases_survive_queue_transfers_and_release_only_after_last_owner() {
        let state = Arc::new(Mutex::new(observer()));
        state
            .lock()
            .unwrap()
            .operations
            .insert(7, Operation { sealed: true, ..Default::default() });
        let work = Work::capture_in(&state, vec![7], "queued history");
        let queued = work.clone();
        drop(work);
        assert!(!state.lock().unwrap().operations[&7].settled());
        std::thread::spawn(move || {
            queued.run(|| assert_eq!(operations(), vec![7]));
            assert!(operations().is_empty());
            drop(queued);
        })
        .join()
        .unwrap();
        assert!(state.lock().unwrap().operations[&7].settled());
    }

    #[test]
    fn async_context_is_restored_between_polls() {
        let state = Arc::new(Mutex::new(observer()));
        state.lock().unwrap().operations.insert(7, Operation::default());
        let work = Work::capture_in(&state, vec![7], "compile");
        let mut polls = 0;
        let future = std::future::poll_fn(|_| {
            assert_eq!(operations(), vec![7]);
            polls += 1;
            if polls == 1 { std::task::Poll::Pending } else { std::task::Poll::Ready(()) }
        });
        let mut scoped = std::pin::pin!(work.during(future));
        let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
        assert!(scoped.as_mut().poll(&mut cx).is_pending());
        assert!(operations().is_empty());
        assert!(scoped.as_mut().poll(&mut cx).is_ready());
        assert!(operations().is_empty());
    }

    #[test]
    fn history_overflow_does_not_prevent_checkpoint_or_gate_cleanup() {
        let mut state = observer();
        for _ in 0..MAX_EVENTS + 2 {
            state.event(json!({"kind":"observed"}));
        }
        assert_eq!(
            respond_in(&mut state, request(1, "processed"))["error"],
            "event history overflow"
        );
        assert_eq!(respond_in(&mut state, request(2, "checkpoint"))["ready"], true);
        let mut open = request(3, "gate_open");
        open["kind"] = "source".into();
        open["url"] = "file:///Main.slint".into();
        let gate = respond_in(&mut state, open)["gate"].clone();
        let mut release = request(4, "gate_release");
        release["gate"] = gate;
        assert_eq!(respond_in(&mut state, release)["ready"], true);
    }

    fn attempt(
        id: u64,
        observation: u64,
        processed_cursor: u64,
        content: Option<&str>,
        outcome: &str,
    ) -> Attempt {
        let root = Url::parse("file:///Main.slint").unwrap();
        Attempt {
            id,
            kind: "compilation".into(),
            root: root.clone(),
            component: None,
            started_cursor: observation,
            inputs: BTreeMap::from([(
                root,
                Input { observation, version: Some(id as i32), content: identity(content) },
            )]),
            outcome: Some(outcome.into()),
            diagnostics: vec!["diagnostic".into()],
            processed_cursor,
        }
    }

    #[test]
    fn old_input_cannot_acknowledge_same_content_after_a_new_boundary() {
        let mut state = observer();
        state.cursor = 12;
        state.attempts.insert(1, attempt(1, 1, 11, Some("A"), "compiled"));
        let mut wait = request(1, "processed");
        wait["after"] = 10.into();
        wait["sources"] = json!({"file:///Main.slint":"A"});
        assert_eq!(respond_in(&mut state, wait.clone())["ready"], false);
        state.attempts.insert(2, attempt(2, 11, 12, Some("A"), "compiled"));
        assert_eq!(respond_in(&mut state, wait)["attempt"]["id"], 2);
    }

    #[test]
    fn applied_requires_a_mounted_instance_but_retains_diagnostics() {
        let mut state = observer();
        state.cursor = 3;
        state.attempts.insert(1, attempt(1, 1, 2, Some("A"), "compiled"));
        state.installed = Some((1, 3));
        let mut wait = request(1, "applied");
        wait["sources"] = json!({"file:///Main.slint":"A"});
        let result = respond_in(&mut state, wait.clone());
        assert_eq!(result["ready"], false);
        assert_eq!(result["installed"]["id"], 1);
        state.mounted = true;
        assert_eq!(respond_in(&mut state, wait)["ready"], true);
    }

    #[test]
    fn missing_and_failed_inputs_are_separate_from_installed_success() {
        let mut state = observer();
        state.cursor = 6;
        state.attempts.insert(1, attempt(1, 1, 2, Some("A"), "compiled"));
        state.installed = Some((1, 3));
        state.attempts.insert(2, attempt(2, 4, 5, None, "load_error"));
        let mut wait = request(1, "processed");
        wait["after"] = 3.into();
        wait["sources"] = json!({"file:///Main.slint":null});
        wait["outcome"] = "load_error".into();
        let result = respond_in(&mut state, wait);
        assert_eq!(result["attempt"]["id"], 2);
        assert_eq!(result["installed"]["id"], 1);
        assert_eq!(result["attempt"]["diagnostics"], json!(["diagnostic"]));
    }

    #[test]
    fn protocol_and_session_are_validated_before_mutation() {
        let mut state = observer();
        let mut wrong = request(1, "begin");
        wrong["protocol"] = 0.into();
        assert_eq!(respond_in(&mut state, wrong)["error"], "protocol mismatch");
        let mut stale = request(2, "begin");
        stale["session"] = "previous".into();
        assert_eq!(respond_in(&mut state, stale)["error"], "stale session");
        assert!(state.operations.is_empty());
    }
}
