// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore callsite

//! Forwards the runtime's `tracing` spans to the recorder of the platform, which stores them in
//! a capture and sends it afterwards. On ESP-IDF that is `slint-esp-trace.cpp`, which also
//! describes the capture. The recorder needs the platform's headers, so it isn't written here.

use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id, Record};
use tracing::subscriber::Interest;
use tracing::{Event, Metadata, Subscriber};

unsafe extern "C" {
    safe fn slint_esp_trace_start() -> bool;
    safe fn slint_esp_trace_cancel();
    safe fn slint_esp_trace_probe_begin() -> u32;
    safe fn slint_esp_trace_probe_end(start: u32, probes: u32);
    fn slint_esp_trace_name(id: u32, name: *const u8, len: usize);
    safe fn slint_esp_trace_enter(id: u32);
    safe fn slint_esp_trace_exit(id: u32);
    safe fn slint_esp_trace_event(id: u32);
    safe fn slint_esp_trace_field(name: u32, value: u64);
}

fn register_name(id: u32, name: &str) {
    // Safety: the pointer and length are those of a valid string.
    unsafe { slint_esp_trace_name(id, name.as_ptr(), name.len()) };
}

/// The ID of a span or event is the address of its call site's metadata, and that of a field the
/// address of its name.
fn metadata_id(metadata: &'static Metadata<'static>) -> u32 {
    metadata as *const Metadata<'static> as usize as u32
}

struct FieldRecorder;

impl FieldRecorder {
    fn push(&self, field: &Field, value: u64) {
        slint_esp_trace_field(field.name().as_ptr() as u32, value);
    }
}

impl Visit for FieldRecorder {
    fn record_i64(&mut self, field: &Field, value: i64) {
        self.push(field, value as u64);
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.push(field, value);
    }

    fn record_f64(&mut self, field: &Field, value: f64) {
        self.push(field, value as i64 as u64);
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.push(field, value as u64);
    }

    fn record_debug(&mut self, _: &Field, _: &dyn core::fmt::Debug) {}
}

struct ForwardingSubscriber;

impl Subscriber for ForwardingSubscriber {
    fn register_callsite(&self, metadata: &'static Metadata<'static>) -> Interest {
        register_name(metadata_id(metadata), metadata.name());
        for field in metadata.fields() {
            register_name(field.name().as_ptr() as u32, field.name());
        }
        Interest::always()
    }

    fn enabled(&self, _: &Metadata<'_>) -> bool {
        true
    }

    fn new_span(&self, span: &Attributes<'_>) -> Id {
        span.record(&mut FieldRecorder);
        Id::from_u64(metadata_id(span.metadata()) as u64)
    }

    fn record(&self, _: &Id, _: &Record<'_>) {}

    fn record_follows_from(&self, _: &Id, _: &Id) {}

    fn event(&self, event: &Event<'_>) {
        event.record(&mut FieldRecorder);
        slint_esp_trace_event(metadata_id(event.metadata()));
    }

    fn enter(&self, span: &Id) {
        slint_esp_trace_enter(span.into_u64() as u32);
    }

    fn exit(&self, span: &Id) {
        slint_esp_trace_exit(span.into_u64() as u32);
    }
}

/// Starts the capture. Only the first call has an effect.
pub fn install() {
    if !slint_esp_trace_start() {
        return;
    }
    if tracing::dispatcher::set_global_default(tracing::Dispatch::new(ForwardingSubscriber))
        .is_err()
    {
        slint_esp_trace_cancel();
        return;
    }
    // Measures the cost of one span through the whole recording path.
    const PROBES: u32 = 10_000;
    let start = slint_esp_trace_probe_begin();
    for _ in 0..PROBES {
        let _span = tracing::info_span!("trace_probe").entered();
    }
    slint_esp_trace_probe_end(start, PROBES);
}
