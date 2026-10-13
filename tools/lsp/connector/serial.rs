// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore JTAG

//! The transport to a microcontroller connected to a serial port, such as an ESP32-S3-BOX-3
//! running `examples/mcu-viewer`. The device can't compile, so the LSP compiles the design
//! with images and fonts rendered ahead of time, and sends the serialized compilation unit.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::rc::{Rc, Weak};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use i_slint_compiler::llr::serialize;
use i_slint_compiler::source_path::SourcePath;
use i_slint_core::InternalToken;
use i_slint_live_preview::protocol::{
    LspToPreviewMessage, PreviewComponent, PreviewConfig, PreviewToLspMessage,
    RemoteConnectionState,
};
use i_slint_live_preview::serial::{
    DeviceMessage, FrameDecoder, HostMessage, PROTOCOL_VERSION, Received, SLINT_VERSION, blob_key,
    encode_frame, store_frames,
};
use lsp_types::Url;
use tokio::sync::mpsc as tokio_mpsc;

use super::remote::RemoteLspToPreview;
use crate::editor_preview::LspToPreviews;

/// How long a device may take to answer the handshake, and to show a design.
const READY_TIMEOUT: Duration = Duration::from_secs(5);
const LOAD_TIMEOUT: Duration = Duration::from_secs(60);
/// How long a write may wait for the device to read, which it does between its other work.
const WRITE_TIMEOUT: Duration = Duration::from_secs(10);
/// How long the design must stay unchanged before it's sent, since the editor reports
/// every keystroke.
const REBUILD_DEBOUNCE: Duration = Duration::from_millis(200);

/// What a session's thread works on, in the order it arrived.
enum Input {
    Lsp(LspToPreviewMessage),
    Device(DeviceMessage),
    /// A line of the device's log output.
    DeviceOutput(String),
    PortClosed(String),
    Stop,
}

/// What a session's thread reports to the preview.
enum Status {
    /// With the problem showing the last design, if there was one.
    Connected {
        problem: Option<String>,
    },
    Failed(String),
}

/// The serial side of the remote preview. Lives on the LSP's thread, next to the
/// WebSocket transport in [`super::remote::RemoteLspToPreview`].
pub struct SerialLspToPreview {
    preview_to_lsp_sender: tokio_mpsc::UnboundedSender<PreviewToLspMessage>,
    to_previews: Weak<LspToPreviews>,
    session: RefCell<Option<mpsc::Sender<Input>>>,
    /// Bumped on every connect or disconnect, so that a replaced session's status is ignored.
    generation: Rc<Cell<u64>>,
}

impl SerialLspToPreview {
    pub fn new(
        preview_to_lsp_sender: tokio_mpsc::UnboundedSender<PreviewToLspMessage>,
        to_previews: Weak<LspToPreviews>,
    ) -> Self {
        Self {
            preview_to_lsp_sender,
            to_previews,
            session: RefCell::default(),
            generation: Rc::default(),
        }
    }

    pub fn is_connected(&self) -> bool {
        self.session.borrow().is_some()
    }

    pub fn send(&self, message: &LspToPreviewMessage) {
        if let Some(session) = self.session.borrow().as_ref() {
            let _ = session.send(Input::Lsp(message.clone()));
        }
    }

    pub fn connect(&self, port_name: String) {
        self.stop();
        let generation = self.generation.get();
        let (inputs, input_receiver) = mpsc::channel();
        *self.session.borrow_mut() = Some(inputs.clone());
        RemoteLspToPreview::emit_state(
            &self.to_previews,
            RemoteConnectionState::Connecting,
            port_name.clone(),
            None,
        );

        let (status_sender, mut status_receiver) = tokio_mpsc::unbounded_channel();
        let preview_to_lsp = self.preview_to_lsp_sender.clone();
        let thread_port_name = port_name.clone();
        std::thread::spawn(move || {
            let report = |status| {
                let _ = status_sender.send(status);
            };
            let log_sender = preview_to_lsp.clone();
            let log = move |message: String| {
                let _ =
                    log_sender.send(PreviewToLspMessage::DebugMessage { location: None, message });
            };
            let session = open_port(&thread_port_name).and_then(|port| {
                let reader = port.try_clone().map_err(|err| err.to_string())?;
                spawn_reader(reader, inputs);
                run_session(port, &input_receiver, &report, &log, &|| {
                    // Ask the LSP for the documents, the configuration and the component to show
                    let _ = preview_to_lsp.send(PreviewToLspMessage::RequestState {
                        files: Vec::new(),
                        settings: Vec::new(),
                    });
                })
            });
            if let Err(err) = session {
                report(Status::Failed(err));
            }
        });

        let to_previews = self.to_previews.clone();
        let current_generation = self.generation.clone();
        crate::editor_preview::spawn_local(async move {
            while let Some(status) = status_receiver.recv().await {
                if current_generation.get() != generation {
                    return;
                }
                match status {
                    Status::Connected { problem } => RemoteLspToPreview::emit_state(
                        &to_previews,
                        RemoteConnectionState::Connected,
                        port_name.clone(),
                        problem,
                    ),
                    Status::Failed(err) => RemoteLspToPreview::emit_state(
                        &to_previews,
                        RemoteConnectionState::Failed,
                        port_name.clone(),
                        Some(err),
                    ),
                }
            }
        });
    }

    pub fn disconnect(&self) {
        if self.is_connected() {
            self.stop();
            RemoteLspToPreview::emit_state(
                &self.to_previews,
                RemoteConnectionState::Disconnected,
                String::new(),
                None,
            );
        }
    }

    fn stop(&self) {
        self.generation.set(self.generation.get() + 1);
        if let Some(session) = self.session.borrow_mut().take() {
            let _ = session.send(Input::Stop);
        }
    }
}

impl Drop for SerialLspToPreview {
    fn drop(&mut self) {
        self.stop();
    }
}

fn open_port(name: &str) -> Result<Box<dyn serialport::SerialPort>, String> {
    let mut port = serialport::new(name, 115_200)
        .timeout(Duration::from_millis(100))
        .open()
        .map_err(|err| format!("Cannot open {name}: {err}"))?;
    // On USB-Serial-JTAG, toggling these can reset the chip
    let _ = port.write_data_terminal_ready(false);
    let _ = port.write_request_to_send(false);
    Ok(port)
}

/// Reads from the port on a thread of its own, into `inputs`.
fn spawn_reader(mut port: impl Read + Send + 'static, inputs: mpsc::Sender<Input>) {
    std::thread::spawn(move || {
        let mut decoder = FrameDecoder::<DeviceMessage>::new(1 << 20);
        let mut buffer = [0u8; 4096];
        loop {
            let closed = match port.read(&mut buffer) {
                Ok(0) => "the port was closed".to_string(),
                Ok(len) => {
                    decoder.push(&buffer[..len], |received| match received {
                        Received::Message(message) => {
                            let _ = inputs.send(Input::Device(message));
                        }
                        Received::Text(text) => {
                            let line = String::from_utf8_lossy(&text).trim_end().to_string();
                            let _ = inputs.send(Input::DeviceOutput(line));
                        }
                    });
                    continue;
                }
                Err(err) if err.kind() == std::io::ErrorKind::TimedOut => continue,
                Err(err) => err.to_string(),
            };
            let _ = inputs.send(Input::PortClosed(closed));
            return;
        }
    });
}

/// The design the LSP asked to preview, as the messages it sent describe it.
#[derive(Default)]
struct Design {
    /// The contents of the documents open in the editor. Other files are read from disk.
    files: HashMap<Url, Vec<u8>>,
    config: PreviewConfig,
    component: Option<PreviewComponent>,
}

impl Design {
    /// Returns whether the design to show changed.
    fn apply(&mut self, message: LspToPreviewMessage) -> bool {
        let changed = match message {
            LspToPreviewMessage::SetContents { url, contents } => {
                let changed = self.files.get(url.url()) != Some(&contents);
                self.files.insert(url.url().clone(), contents);
                changed
            }
            LspToPreviewMessage::InvalidateContents { url }
            | LspToPreviewMessage::ForgetFile { url } => self.files.remove(&url).is_some(),
            LspToPreviewMessage::SetConfiguration { config } => {
                core::mem::replace(&mut self.config, config) != self.config
            }
            LspToPreviewMessage::ShowPreview(component) => {
                self.component.replace(component.clone()) != Some(component)
            }
            _ => false,
        };
        changed && self.component.is_some()
    }

    /// Compiles the design for a device with this scale factor.
    fn compile(&self, scale_factor: f32) -> Result<Compiled, String> {
        let component = self.component.as_ref().ok_or("No component to show")?;
        let mut compiler = slint_interpreter::Compiler::default();
        let mut overrides = self.config.overrides.clone();
        // "native" would pick the style of this machine, possibly Qt
        if overrides.style.as_deref() == Some("native") {
            overrides.style = Some(i_slint_common::get_native_style(false, "").into());
        }
        overrides.embed_resources = Some(slint_interpreter::EmbedResourcesKind::EmbedTextures);
        overrides.const_scale_factor = Some(scale_factor);
        compiler.set_overrides(overrides, InternalToken);
        let files = self.files.clone();
        let open_file: i_slint_compiler::OpenImportCallback =
            std::rc::Rc::new(move |path: SourcePath| {
                let contents = path
                    .to_url()
                    .and_then(|url| files.get(&url))
                    .map(|contents| Ok(String::from_utf8_lossy(contents).into_owned()));
                Box::pin(std::future::ready(contents))
            });
        compiler.set_project_file_loader(open_file.clone(), InternalToken);
        compiler.compiler_configuration(InternalToken).open_import_callback = Some(open_file);

        let path = SourcePath::from_url(&component.url);
        let source = match self.files.get(&component.url) {
            Some(contents) => String::from_utf8_lossy(contents).into_owned(),
            None => path.read_to_string().map_err(|err| format!("Cannot read {path}: {err}"))?,
        };
        let result = spin_on::spin_on(compiler.build_from_source_path(source, path, InternalToken));
        if result.has_errors() {
            let errors = result
                .diagnostics()
                .filter(|d| d.level() == slint_interpreter::DiagnosticLevel::Error)
                .map(|d| d.to_string())
                .collect::<Vec<_>>();
            return Err(format!(
                "The design has errors, the device keeps showing the previous one:\n{}",
                errors.join("\n")
            ));
        }
        let name = component
            .component
            .as_deref()
            .or_else(|| result.component_names().next())
            .filter(|name| result.component_names().any(|known| known == *name))
            .map(String::from)
            .ok_or("No component to show")?;
        let unit = result.into_compilation_unit(InternalToken).ok_or("No component to show")?;
        let (unit, resources) = serialize::to_bytes_and_resources(unit)
            .map_err(|err| format!("Cannot serialize the design: {err}"))?;
        Ok(Compiled { component: name, unit, resources })
    }
}

/// A design, serialized for the device.
struct Compiled {
    component: String,
    unit: Vec<u8>,
    resources: Vec<Vec<u8>>,
}

/// What the device said it is, in its [`DeviceMessage::Ready`], and what it keeps.
struct Device {
    scale_factor: f32,
    /// The keys of the units that the device keeps, see [`HostMessage::Show`].
    units: HashSet<u64>,
    /// The keys of the embedded resources that the device keeps, see
    /// [`HostMessage::StoreBegin`] and [`HostMessage::Show`].
    resources: HashSet<u64>,
    /// The design of the last [`HostMessage::Show`], unless the device failed to show it.
    shown: Option<Shown>,
}

/// What a [`HostMessage::Show`] asks the device to show.
#[derive(PartialEq)]
struct Shown {
    component: String,
    unit: u64,
    resources: Vec<u64>,
}

impl Device {
    fn new(scale_factor: f32) -> Self {
        Self { scale_factor, units: HashSet::new(), resources: HashSet::new(), shown: None }
    }

    /// Forgets what the device keeps, so that it's all sent again.
    fn forget(&mut self) {
        self.units.clear();
        self.resources.clear();
        self.shown = None;
    }
}

/// A [`HostMessage::Show`] that the device didn't confirm yet.
struct Showing {
    id: u32,
    /// How many bytes of blobs were sent for it.
    sent: usize,
    started: Instant,
}

/// Whether a [`Session`] goes on.
enum Flow {
    Continue,
    Stop,
}

/// The state of [`run_session()`].
struct Session<'a, P> {
    port: P,
    inputs: &'a mpsc::Receiver<Input>,
    report: &'a dyn Fn(Status),
    log: &'a dyn Fn(String),
    design: Design,
    device: Device,
    /// When to compile and send the design, once it stopped changing for
    /// [`REBUILD_DEBOUNCE`].
    send_at: Option<Instant>,
    showing: Option<Showing>,
    last_id: u32,
    /// The device's last line of output, which tells why it crashed.
    last_output: String,
    /// The design the device crashed loading, which isn't sent again.
    crashed: Option<Shown>,
}

/// Shows the design on the device at the other end of `port` until the LSP stops the
/// session, and returns why the connection was lost otherwise.
fn run_session(
    mut port: impl Write,
    inputs: &mpsc::Receiver<Input>,
    report: &dyn Fn(Status),
    log: &dyn Fn(String),
    request_state: &dyn Fn(),
) -> Result<(), String> {
    let mut design = Design::default();
    let Some(device) = handshake(&mut port, inputs, &mut design, log)? else { return Ok(()) };
    report(Status::Connected { problem: None });
    request_state();

    let mut session = Session {
        port,
        inputs,
        report,
        log,
        design,
        device,
        send_at: None,
        showing: None,
        last_id: 0,
        last_output: String::new(),
        crashed: None,
    };
    loop {
        let load_deadline = session.showing.as_ref().map(|showing| showing.started + LOAD_TIMEOUT);
        let deadline = session.send_at.into_iter().chain(load_deadline).min();
        let input = match deadline {
            Some(deadline) => {
                inputs.recv_timeout(deadline.saturating_duration_since(Instant::now()))
            }
            None => inputs.recv().map_err(|_| mpsc::RecvTimeoutError::Disconnected),
        };
        let flow = match input {
            Ok(input) => session.handle(input)?,
            Err(mpsc::RecvTimeoutError::Disconnected) => session.handle(Input::Stop)?,
            Err(mpsc::RecvTimeoutError::Timeout) => session.on_deadline()?,
        };
        if let Flow::Stop = flow {
            return Ok(());
        }
    }
}

impl<P: Write> Session<'_, P> {
    /// Shows `problem` in the preview, or that there's none.
    fn report_problem(&self, problem: Option<String>) {
        if let Some(problem) = &problem {
            (self.log)(problem.clone());
        }
        (self.report)(Status::Connected { problem });
    }

    fn handle(&mut self, input: Input) -> Result<Flow, String> {
        let log = self.log;
        match input {
            Input::Stop => {
                let _ = self.port.write_all(&encode_frame(&HostMessage::Clear));
                return Ok(Flow::Stop);
            }
            Input::Lsp(message) => {
                if self.design.apply(message) {
                    self.send_at = Some(Instant::now() + REBUILD_DEBOUNCE);
                }
            }
            Input::Device(DeviceMessage::Ready { scale_factor, .. }) => {
                let crashed = self.showing.take().is_some();
                let shown = core::mem::replace(&mut self.device, Device::new(scale_factor)).shown;
                if crashed {
                    self.crashed = shown;
                    self.report_problem(Some(format!(
                        "The device restarted while loading the design. Its last output: {}",
                        self.last_output
                    )));
                } else {
                    log("The device restarted".into());
                    if self.design.component.is_some() {
                        self.send_at = Some(Instant::now());
                    }
                }
            }
            Input::Device(DeviceMessage::Error { id, message }) => {
                self.report_problem(Some(format!("The device reports: {message}")));
                self.device.forget();
                if id.is_some() && self.showing.as_ref().map(|showing| showing.id) == id {
                    self.showing = None;
                }
            }
            Input::Device(DeviceMessage::Loaded { id, heap_free }) => {
                if let Some(showing) = self.showing.take_if(|showing| showing.id == id) {
                    self.crashed = None;
                    self.report_problem(None);
                    log(format!(
                        "The device shows {}: {} KiB sent in {} ms, {} KiB free",
                        self.device.shown.as_ref().map_or("", |shown| shown.component.as_str()),
                        showing.sent / 1024,
                        showing.started.elapsed().as_millis(),
                        heap_free / 1024
                    ));
                }
            }
            Input::DeviceOutput(line) => {
                log(format!("[device] {line}"));
                if !line.is_empty() {
                    self.last_output = line;
                }
            }
            Input::PortClosed(err) => return Err(format!("Lost the connection: {err}")),
        }
        Ok(Flow::Continue)
    }

    fn on_deadline(&mut self) -> Result<Flow, String> {
        let now = Instant::now();
        if self.showing.as_ref().is_some_and(|showing| showing.started + LOAD_TIMEOUT <= now) {
            self.report_problem(Some("The device didn't confirm showing the design".into()));
            self.showing = None;
            self.device.forget();
        }
        if self.send_at.is_some_and(|send_at| send_at <= now) {
            self.send_at = None;
            return self.send();
        }
        Ok(Flow::Continue)
    }

    /// Compiles the design, and sends what the device doesn't keep of it, unless the design
    /// changes again meanwhile. The device confirms showing it later, with
    /// [`DeviceMessage::Loaded`].
    fn send(&mut self) -> Result<Flow, String> {
        let compiled = match self.design.compile(self.device.scale_factor) {
            Ok(compiled) => compiled,
            Err(err) => {
                self.report_problem(Some(err));
                return Ok(Flow::Continue);
            }
        };
        let shown = Shown {
            component: compiled.component.clone(),
            unit: blob_key(&compiled.unit),
            resources: compiled.resources.iter().map(|r| blob_key(r)).collect(),
        };
        if self.device.shown.as_ref() == Some(&shown) || self.crashed.as_ref() == Some(&shown) {
            return Ok(Flow::Continue);
        }
        let started = Instant::now();
        let mut sent = 0;
        let blobs = core::iter::once((shown.unit, &compiled.unit, false))
            .chain(shown.resources.iter().zip(&compiled.resources).map(|(k, r)| (*k, r, true)));
        for (key, blob, resource) in blobs {
            let stored = if resource { &self.device.resources } else { &self.device.units };
            if stored.contains(&key) {
                continue;
            }
            for frame in store_frames(key, blob, resource) {
                write_patiently(&mut self.port, &frame)
                    .map_err(|err| format!("Cannot send the design: {err}"))?;
                while let Ok(input) = self.inputs.try_recv() {
                    if let Flow::Stop = self.handle(input)? {
                        return Ok(Flow::Stop);
                    }
                }
                if self.send_at.is_some() {
                    // The design changed: the next store supersedes this one
                    return Ok(Flow::Continue);
                }
            }
            if resource { &mut self.device.resources } else { &mut self.device.units }.insert(key);
            sent += blob.len();
        }
        self.last_id += 1;
        let show = HostMessage::Show {
            id: self.last_id,
            component: shown.component.clone(),
            unit: shown.unit,
            resources: shown.resources.clone(),
        };
        write_patiently(&mut self.port, &encode_frame(&show))
            .map_err(|err| format!("Cannot send the design: {err}"))?;
        self.device.units = HashSet::from([shown.unit]);
        self.device.resources = shown.resources.iter().copied().collect();
        self.device.shown = Some(shown);
        self.showing = Some(Showing { id: self.last_id, sent, started });
        Ok(Flow::Continue)
    }
}

/// Asks the device what it is, and checks that it runs the same Slint as this LSP.
/// Returns `None` if the session was stopped meanwhile.
fn handshake(
    port: &mut impl Write,
    inputs: &mpsc::Receiver<Input>,
    design: &mut Design,
    log: &dyn Fn(String),
) -> Result<Option<Device>, String> {
    port.write_all(&encode_frame(&HostMessage::Hello { protocol: PROTOCOL_VERSION }))
        .map_err(|err| format!("Cannot write to the port: {err}"))?;
    let deadline = Instant::now() + READY_TIMEOUT;
    loop {
        let input = inputs.recv_timeout(deadline.saturating_duration_since(Instant::now()));
        match input {
            Ok(Input::Device(DeviceMessage::Ready {
                protocol,
                slint_version,
                format,
                scale_factor,
                ..
            })) => {
                return check_device(protocol, &slint_version, format)
                    .map(|()| Some(Device::new(scale_factor)));
            }
            Ok(Input::Device(DeviceMessage::Error { message, .. })) => return Err(message),
            Ok(Input::Device(DeviceMessage::Loaded { .. })) => {}
            Ok(Input::Lsp(message)) => {
                design.apply(message);
            }
            Ok(Input::DeviceOutput(line)) => log(format!("[device] {line}")),
            Ok(Input::PortClosed(err)) => return Err(format!("Lost the connection: {err}")),
            Ok(Input::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(None),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                return Err(
                    "The device didn't answer: is it running the Slint viewer firmware?".into()
                );
            }
        }
    }
}

/// A unit only reads back with an interpreter of the same Slint version, built from the
/// same sources: the format isn't stable.
fn check_device(protocol: u16, slint_version: &str, format: u64) -> Result<(), String> {
    if protocol != PROTOCOL_VERSION || slint_version != SLINT_VERSION {
        return Err(format!(
            "The device runs Slint {slint_version}, and this LSP Slint {SLINT_VERSION}: flash the firmware of Slint {SLINT_VERSION}"
        ));
    }
    if format != serialize::FORMAT_ID {
        return Err(format!(
            "The device runs a firmware built from other sources than this LSP: build it from the same sources of Slint {SLINT_VERSION} and flash it again"
        ));
    }
    Ok(())
}

/// Like `write_all`, but waits through the port's timeouts: the device reads when it gets
/// to it, and a write only times out once it stopped reading for [`WRITE_TIMEOUT`].
fn write_patiently(port: &mut impl Write, mut bytes: &[u8]) -> std::io::Result<()> {
    let mut stalled_since = None;
    while !bytes.is_empty() {
        match port.write(bytes) {
            Ok(0) => return Err(std::io::ErrorKind::WriteZero.into()),
            Ok(written) => {
                bytes = &bytes[written..];
                stalled_since = None;
            }
            Err(err)
                if matches!(
                    err.kind(),
                    std::io::ErrorKind::TimedOut
                        | std::io::ErrorKind::WouldBlock
                        | std::io::ErrorKind::Interrupted
                ) =>
            {
                if stalled_since.get_or_insert_with(Instant::now).elapsed() > WRITE_TIMEOUT {
                    return Err(err);
                }
            }
            Err(err) => return Err(err),
        }
    }
    port.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use i_slint_live_preview::protocol::VersionedUrl;
    use i_slint_live_preview::serial::{DeviceEvent, DeviceSession};

    /// Plays the device: answers the handshake with `slint_version`, and reads back each
    /// unit it receives. Returns the components it showed, once the host cleared the screen.
    fn fake_device(
        mut from_host: impl Read,
        mut to_host: impl Write,
        slint_version: &str,
    ) -> Vec<String> {
        let mut session = DeviceSession::new(1 << 24);
        let mut shown = Vec::new();
        let ready = DeviceMessage::Ready {
            protocol: PROTOCOL_VERSION,
            slint_version: slint_version.into(),
            format: serialize::FORMAT_ID,
            board: "fake".into(),
            width: 320,
            height: 240,
            scale_factor: 1.,
            heap_free: 0,
        };
        let mut buffer = [0u8; 4096];
        loop {
            let Ok(len) = from_host.read(&mut buffer) else { return shown };
            if len == 0 {
                return shown;
            }
            let mut cleared = false;
            let mut events = Vec::new();
            session.push(&buffer[..len], |event| events.push(event));
            for event in events {
                match event {
                    DeviceEvent::Hello => {
                        to_host.write_all(b"a log line from the device\n").unwrap();
                        to_host.write_all(&encode_frame(&ready)).unwrap();
                    }
                    DeviceEvent::Resource { key, data } => {
                        let resource = serialize::resource_from_bytes(&data).unwrap();
                        slint_interpreter::register_embedded_resource(
                            key,
                            resource,
                            InternalToken,
                        );
                    }
                    DeviceEvent::Show { id, component, unit, resources } => {
                        slint_interpreter::retain_embedded_resources(&resources, InternalToken);
                        let unit = session.blob(unit).unwrap();
                        let unit = serialize::from_bytes_with_registered_resources(unit, resources)
                            .unwrap();
                        let result = slint_interpreter::CompilationResult::from_compilation_unit(
                            unit,
                            InternalToken,
                        );
                        assert!(result.component(&component).is_some());
                        if component == "Crash" {
                            // Like the board's panic handler, which then restarts it
                            to_host.write_all(b"Panic: out of memory\n").unwrap();
                            to_host.write_all(&encode_frame(&ready)).unwrap();
                            shown.push("crashed".into());
                            continue;
                        }
                        to_host
                            .write_all(&encode_frame(&DeviceMessage::Loaded { id, heap_free: 0 }))
                            .unwrap();
                        shown.push(component);
                    }
                    DeviceEvent::Clear => cleared = true,
                    other => panic!("unexpected {other:?}"),
                }
            }
            if cleared {
                return shown;
            }
        }
    }

    /// A session running against a fake device, see [`start_against_fake_device()`].
    struct FakeSession {
        inputs: mpsc::Sender<Input>,
        logs: mpsc::Receiver<String>,
        /// The problems the session reported while connected.
        problems: mpsc::Receiver<Option<String>>,
        session: std::thread::JoinHandle<Result<(), String>>,
        device: std::thread::JoinHandle<Vec<String>>,
    }

    impl FakeSession {
        /// Waits for a log line starting with `prefix`, and returns the lines until then.
        fn wait_for(&self, prefix: &str) -> Vec<String> {
            let mut seen = Vec::new();
            while let Ok(line) = self.logs.recv_timeout(Duration::from_secs(60)) {
                let done = line.starts_with(prefix);
                seen.push(line);
                if done {
                    break;
                }
            }
            seen
        }

        /// Stops the session, and returns how it ended, what it logged meanwhile, and what
        /// the device showed.
        fn stop(self) -> (Result<(), String>, Vec<String>, Vec<String>) {
            let _ = self.inputs.send(Input::Stop);
            let ended = self.session.join().unwrap();
            (ended, self.logs.try_iter().collect(), self.device.join().unwrap())
        }
    }

    /// Starts a session against a fake device, which the LSP feeds `messages` once connected.
    fn start_against_fake_device(
        slint_version: &'static str,
        messages: Vec<LspToPreviewMessage>,
    ) -> FakeSession {
        let (host_reader, device_writer) = std::io::pipe().unwrap();
        let (device_reader, host_writer) = std::io::pipe().unwrap();
        let device =
            std::thread::spawn(move || fake_device(device_reader, device_writer, slint_version));

        let (inputs, input_receiver) = mpsc::channel();
        let (logs, log_receiver) = mpsc::channel::<String>();
        spawn_reader(host_reader, inputs.clone());
        let (problems, problem_receiver) = mpsc::channel();
        let feeder = inputs.clone();
        let session = std::thread::spawn(move || {
            let log = |line| {
                let _ = logs.send(line);
            };
            let report = |status| {
                if let Status::Connected { problem } = status {
                    let _ = problems.send(problem);
                }
            };
            let request_state = || {
                for message in &messages {
                    let _ = feeder.send(Input::Lsp(message.clone()));
                }
            };
            run_session(host_writer, &input_receiver, &report, &log, &request_state)
        });
        FakeSession { inputs, logs: log_receiver, problems: problem_receiver, session, device }
    }

    /// Runs a session against a fake device, feeding it `messages` from the LSP once
    /// connected. With `stop_after`, stops it once it logged a line starting with that.
    /// Returns how the session ended, what it logged, and what the device showed.
    fn run_against_fake_device(
        slint_version: &'static str,
        messages: Vec<LspToPreviewMessage>,
        stop_after: Option<&str>,
    ) -> (Result<(), String>, Vec<String>, Vec<String>) {
        let fake = start_against_fake_device(slint_version, messages);
        let Some(stop_after) = stop_after else {
            let ended = fake.session.join().unwrap();
            return (ended, fake.logs.try_iter().collect(), fake.device.join().unwrap());
        };
        let mut seen = fake.wait_for(stop_after);
        let (ended, logs, shown) = fake.stop();
        seen.extend(logs);
        (ended, seen, shown)
    }

    fn show(url: &Url, contents: &str) -> Vec<LspToPreviewMessage> {
        vec![
            LspToPreviewMessage::SetContents {
                url: VersionedUrl::new(url.clone(), None),
                contents: contents.as_bytes().to_vec(),
            },
            LspToPreviewMessage::SetConfiguration { config: PreviewConfig::default() },
            LspToPreviewMessage::ShowPreview(PreviewComponent {
                url: url.clone(),
                component: None,
            }),
        ]
    }

    #[test]
    fn shows_the_design_on_the_device() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("main.slint");
        // Only the editor's contents exist, not the file
        let url = Url::from_file_path(&path).unwrap();
        let messages = show(
            &url,
            "export component Main inherits Window { width: 32px; height: 32px; Rectangle { background: red; } }",
        );
        let (ended, logs, shown) =
            run_against_fake_device(SLINT_VERSION, messages, Some("The device shows "));
        assert_eq!(ended, Ok(()));
        assert_eq!(shown, ["Main"], "{logs:?}");
        assert!(logs.iter().any(|line| line == "[device] a log line from the device"), "{logs:?}");
    }

    #[test]
    fn sends_nothing_that_does_not_compile() {
        let dir = tempfile::tempdir().unwrap();
        let url = Url::from_file_path(dir.path().join("main.slint")).unwrap();
        let mut messages = show(&url, "export component Main inherits Window { }");
        messages.extend(show(&url, "export component Main inherits Window { syntax error"));
        let fake = start_against_fake_device(SLINT_VERSION, messages);
        let logs = fake.wait_for("The design has errors");
        let problems: Vec<_> = fake.problems.try_iter().collect();
        let (ended, _, shown) = fake.stop();
        assert_eq!(ended, Ok(()));
        assert!(shown.is_empty(), "{shown:?}");
        assert!(logs.iter().any(|line| line.starts_with("The design has errors")), "{logs:?}");
        assert!(
            matches!(problems.last(), Some(Some(p)) if p.starts_with("The design has errors")),
            "{problems:?}"
        );
    }

    #[test]
    fn sends_only_what_changes_the_design() {
        let dir = tempfile::tempdir().unwrap();
        let url = Url::from_file_path(dir.path().join("main.slint")).unwrap();
        let source = "export component Main inherits Window { Rectangle { background: red; } }";
        let edit = |contents: String| {
            Input::Lsp(LspToPreviewMessage::SetContents {
                url: VersionedUrl::new(url.clone(), None),
                contents: contents.into_bytes(),
            })
        };
        let fake = start_against_fake_device(SLINT_VERSION, show(&url, source));
        fake.wait_for("The device shows ");
        // The same unit, compiled again
        fake.inputs.send(edit(format!("// A comment\n{source}"))).unwrap();
        std::thread::sleep(REBUILD_DEBOUNCE * 3);
        fake.inputs.send(edit(source.replace("red", "blue"))).unwrap();
        let seen = fake.wait_for("The device shows ");
        let (ended, _, shown) = fake.stop();
        assert_eq!(ended, Ok(()));
        assert_eq!(shown, ["Main", "Main"], "{seen:?}");
    }

    #[test]
    fn reports_a_crash_and_does_not_send_that_design_again() {
        let dir = tempfile::tempdir().unwrap();
        let url = Url::from_file_path(dir.path().join("main.slint")).unwrap();
        let fake = start_against_fake_device(
            SLINT_VERSION,
            show(&url, "export component Crash inherits Window { }"),
        );
        let seen = fake.wait_for("The device restarted while loading the design");
        assert!(
            seen.last().unwrap().ends_with("Its last output: Panic: out of memory"),
            "{seen:?}"
        );
        // Reported in the preview, after the connection itself
        let problems: Vec<_> = fake.problems.try_iter().collect();
        assert!(
            matches!(problems.as_slice(), [None, Some(p)] if p.contains("Panic")),
            "{problems:?}"
        );
        std::thread::sleep(REBUILD_DEBOUNCE * 3);
        let (ended, _, shown) = fake.stop();
        assert_eq!(ended, Ok(()));
        assert_eq!(shown, ["crashed"]);
    }

    #[test]
    fn refuses_a_device_of_another_slint_version() {
        let (ended, _, shown) = run_against_fake_device("0.0.1", Vec::new(), None);
        let err = ended.unwrap_err();
        assert!(err.contains("Slint 0.0.1"), "{err}");
        assert!(shown.is_empty());
    }

    #[test]
    fn refuses_a_device_built_from_other_sources() {
        let err = check_device(PROTOCOL_VERSION, SLINT_VERSION, !serialize::FORMAT_ID).unwrap_err();
        assert!(err.contains("other sources"), "{err}");
        assert!(check_device(PROTOCOL_VERSION + 1, SLINT_VERSION, serialize::FORMAT_ID).is_err());
        assert!(check_device(PROTOCOL_VERSION, SLINT_VERSION, serialize::FORMAT_ID).is_ok());
    }
}
