// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore JTAG

//! The transport to a microcontroller connected to a serial port, such as an ESP32-S3-BOX-3
//! running `examples/mcu-viewer`. The device can't compile, so the host compiles the design
//! with images and fonts rendered ahead of time, and sends the serialized compilation unit.

use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use i_slint_compiler::diagnostics::{BuildDiagnostics, DiagnosticLevel};
use i_slint_compiler::generator::OutputFormat;
use i_slint_compiler::llr::serialize;
use i_slint_compiler::source_path::SourcePath;
use i_slint_compiler::{CompilerConfiguration, project_file};
use i_slint_live_preview::protocol::{
    LspToPreviewMessage, PreviewComponent, PreviewConfig, PreviewToLspMessage,
};
use i_slint_live_preview::serial::{
    DeviceMessage, FrameDecoder, HostMessage, MAX_FRAME_LEN, PROTOCOL_VERSION, Received, blob_key,
    encode_frame, store_frames,
};
use i_slint_live_preview::{REBUILD_DEBOUNCE, SLINT_VERSION};
use lsp_types::Url;
use tokio::sync::mpsc as tokio_mpsc;

/// How long a device may take to answer the handshake: the viewer firmware answers between
/// two frames, so a port that stays silent longer doesn't run it.
const READY_TIMEOUT: Duration = Duration::from_secs(1);
/// How long a device may take to show a design.
const LOAD_TIMEOUT: Duration = Duration::from_secs(60);
/// How long a write may wait for the device to read, which it does between its other work.
const WRITE_TIMEOUT: Duration = Duration::from_secs(10);

/// What a session's thread works on, in the order it arrived.
enum Input {
    Lsp(Box<LspToPreviewMessage>),
    Device(DeviceMessage),
    /// A line of the device's log output.
    DeviceOutput(String),
    PortClosed(String),
    Stop,
}

/// What a [`SerialConnection`] reports.
#[derive(Debug)]
pub enum SerialEvent {
    /// The device shows the design, or `problem` tells why it doesn't show the current one.
    Connected { problem: Option<String> },
    /// The connection is lost, or was never made.
    Failed(String),
    /// For the editor, such as a log line or a request for the documents to show.
    Message(PreviewToLspMessage),
}

/// A session showing the design on a microcontroller connected to a serial port.
/// It runs on a thread of its own, and stops when dropped.
pub struct SerialConnection {
    inputs: mpsc::Sender<Input>,
    session: Option<std::thread::JoinHandle<()>>,
}

impl SerialConnection {
    /// Connects to the serial port `port_name` once the `previous` session ended,
    /// and returns the events of the session.
    pub fn connect(
        port_name: String,
        previous: Option<Self>,
    ) -> (Self, tokio_mpsc::UnboundedReceiver<SerialEvent>) {
        let (inputs, input_receiver) = mpsc::channel();
        let (events, event_receiver) = tokio_mpsc::unbounded_channel();
        let reader_inputs = inputs.clone();
        let previous = previous.and_then(|mut previous| previous.session.take());
        let session = std::thread::spawn(move || {
            // It may hold the same port, which only opens once
            if let Some(previous) = previous {
                let _ = previous.join();
            }
            let report = |event| {
                let _ = events.send(event);
            };
            let session = open_port(&port_name).and_then(|port| {
                let reader = port.try_clone().map_err(|err| err.to_string())?;
                let stop_reading = Arc::new(AtomicBool::new(false));
                let reader = spawn_reader(reader, reader_inputs, stop_reading.clone());
                let session = run_session(port, &input_receiver, &report);
                // The reader's handle keeps the port open
                stop_reading.store(true, Ordering::Relaxed);
                let _ = reader.join();
                session
            });
            if let Err(err) = session {
                report(SerialEvent::Failed(err));
            }
        });
        (Self { inputs, session: Some(session) }, event_receiver)
    }

    pub fn send(&self, message: &LspToPreviewMessage) {
        let _ = self.inputs.send(Input::Lsp(Box::new(message.clone())));
    }
}

impl Drop for SerialConnection {
    fn drop(&mut self) {
        let _ = self.inputs.send(Input::Stop);
    }
}

/// A USB serial port, which may be a microcontroller running the Slint viewer firmware:
/// which one is only known once connected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UsbSerialPort {
    pub name: String,
    /// The product, or the manufacturer, that the device reports.
    pub product: String,
}

/// How often a UI lists the USB serial ports again while it shows them.
pub const SERIAL_SCAN_INTERVAL: Duration = Duration::from_secs(2);

/// The USB serial ports of this computer.
pub fn usb_serial_ports() -> Vec<UsbSerialPort> {
    serialport::available_ports()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|port| match port.port_type {
            serialport::SerialPortType::UsbPort(usb) => Some(UsbSerialPort {
                name: port.port_name,
                product: usb
                    .product
                    .or(usb.manufacturer)
                    .unwrap_or_else(|| "USB serial device".into()),
            }),
            _ => None,
        })
        .collect()
}

fn open_port(name: &str) -> Result<Box<dyn serialport::SerialPort>, String> {
    let mut port = serialport::new(name, 115_200)
        .timeout(Duration::from_millis(100))
        .open()
        .map_err(|err| format!("Cannot open {name}: {err}"))?;
    // On USB-Serial-JTAG, RTS without DTR resets the chip, and opening the port sets both
    let _ = port.write_request_to_send(false);
    let _ = port.write_data_terminal_ready(false);
    Ok(port)
}

/// Reads from the port on a thread of its own, into `inputs`, until `stop` is set.
fn spawn_reader(
    mut port: impl Read + Send + 'static,
    inputs: mpsc::Sender<Input>,
    stop: Arc<AtomicBool>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut decoder = FrameDecoder::<DeviceMessage>::default();
        let mut buffer = [0u8; 4096];
        while !stop.load(Ordering::Relaxed) {
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
    })
}

/// The design the editor asked to preview, as the messages it sent describe it.
#[derive(Default)]
struct Design {
    /// The contents of the documents open in the editor. Other files are read from disk.
    files: HashMap<Url, Vec<u8>>,
    config: PreviewConfig,
    project_root: Option<Url>,
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
            // Also a file on disk, such as an image, that the design may use
            LspToPreviewMessage::InvalidateContents { url }
            | LspToPreviewMessage::ForgetFile { url } => {
                self.files.remove(&url);
                true
            }
            LspToPreviewMessage::SetConfiguration { config } => {
                core::mem::replace(&mut self.config, config) != self.config
            }
            LspToPreviewMessage::OpenProject { root } => {
                core::mem::replace(&mut self.project_root, root) != self.project_root
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
        let mut overrides = self.config.overrides.clone();
        // "native" would pick the style of this machine, possibly Qt
        if overrides.style.as_deref() == Some("native") {
            overrides.style = Some(i_slint_common::get_native_style(false, "").into());
        }
        overrides.embed_resources = Some(i_slint_compiler::EmbedResourcesKind::EmbedTextures);
        overrides.const_scale_factor = Some(scale_factor);
        let files = self.files.clone();
        let open_file: i_slint_compiler::OpenImportCallback =
            std::rc::Rc::new(move |path: SourcePath| {
                let contents = path
                    .to_url()
                    .and_then(|url| files.get(&url))
                    .map(|contents| Ok(String::from_utf8_lossy(contents).into_owned()));
                Box::pin(std::future::ready(contents))
            });

        let path = SourcePath::from_url(&component.url);
        let source = match self.files.get(&component.url) {
            Some(contents) => String::from_utf8_lossy(contents).into_owned(),
            None => path.read_to_string().map_err(|err| format!("Cannot read {path}: {err}"))?,
        };
        let mut diagnostics = BuildDiagnostics::default();
        let (input, _) = spin_on::spin_on(project_file::load_input(
            &path,
            Some(source),
            self.project_root.as_ref().map(SourcePath::from_url),
            Some(open_file.clone()),
            &mut diagnostics,
        ));
        let compiled = input.and_then(|input| {
            let mut config = CompilerConfiguration::new(OutputFormat::Interpreter);
            overrides.apply(input.project_file.as_ref(), &mut config);
            config.open_import_callback = Some(open_file);
            crate::document_cache::configure_preview(&mut config, component.component.clone());
            let compiled;
            (diagnostics, compiled) =
                spin_on::spin_on(serialize::compile(&input.source, input.source_code, config));
            compiled
        });
        let Some((unit, resources)) = compiled else {
            let errors = diagnostics
                .iter()
                .filter(|d| d.level() == DiagnosticLevel::Error)
                .map(|d| d.to_string())
                .collect::<Vec<_>>();
            return Err(format!(
                "The design has errors, the device keeps showing the previous one:\n{}",
                errors.join("\n")
            ));
        };
        let name = unit.public_components.first().ok_or("No component to show")?.name.to_string();
        let cannot_serialize = |err| format!("Cannot serialize the design: {err}");
        let unit = serialize::to_bytes(&unit).map_err(cannot_serialize)?;
        let resources = resources
            .iter()
            .map(serialize::resource_to_bytes)
            .collect::<Result<_, _>>()
            .map_err(cannot_serialize)?;
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
    /// Changes when the device drops the blob being stored.
    epoch: u32,
    /// The keys of the embedded resources that the device keeps, see
    /// [`HostMessage::StoreBegin`] and [`HostMessage::Show`].
    resources: HashSet<u64>,
    /// The design of the last [`HostMessage::Show`], unless the device failed to show it.
    shown: Option<Shown>,
}

/// What a [`HostMessage::Show`] asks the device to show.
#[derive(Clone, PartialEq)]
struct Shown {
    component: String,
    unit: u64,
    resources: Vec<u64>,
}

impl Device {
    fn new(scale_factor: f32) -> Self {
        Self { scale_factor, epoch: 0, resources: HashSet::new(), shown: None }
    }

    /// Forgets what the device keeps, so that it's all sent again.
    fn forget(&mut self) {
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
    report: &'a dyn Fn(SerialEvent),
    design: Design,
    device: Device,
    /// When to compile and send the design, once it stopped changing for
    /// [`REBUILD_DEBOUNCE`].
    send_at: Option<Instant>,
    showing: Option<Showing>,
    last_id: u32,
    /// The design sent last since the device started, which crashed it if it restarts.
    sent: Option<Shown>,
    /// The last panic message in the device's output.
    last_panic: Option<String>,
    /// The design the device crashed showing, which isn't sent again.
    crashed: Option<Shown>,
}

/// Shows the design on the device at the other end of `port` until the editor stops the
/// session, and returns why the connection was lost otherwise.
fn run_session(
    mut port: impl Write,
    inputs: &mpsc::Receiver<Input>,
    report: &dyn Fn(SerialEvent),
) -> Result<(), String> {
    let mut design = Design::default();
    let Some(device) = handshake(&mut port, inputs, &mut design, report)? else { return Ok(()) };
    report(SerialEvent::Connected { problem: None });
    // Ask for the documents, the configuration and the component to show
    report(SerialEvent::Message(PreviewToLspMessage::RequestState {
        files: Vec::new(),
        settings: Vec::new(),
    }));
    // The editor may have sent the design during the handshake
    let send_at = design.component.is_some().then(Instant::now);

    let mut session = Session {
        port,
        inputs,
        report,
        design,
        device,
        send_at,
        showing: None,
        last_id: 0,
        sent: None,
        last_panic: None,
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
        (self.report)(SerialEvent::Connected { problem: problem.clone() });
        if let Some(problem) = problem {
            log_to(self.report, problem);
        }
    }

    fn handle(&mut self, input: Input) -> Result<Flow, String> {
        let report = self.report;
        let log = |message| log_to(report, message);
        match input {
            Input::Stop => {
                let _ = self.port.write_all(&encode_frame(&HostMessage::Clear));
                return Ok(Flow::Stop);
            }
            Input::Lsp(message) => {
                if self.design.apply(*message) {
                    self.send_at = Some(Instant::now() + REBUILD_DEBOUNCE);
                }
            }
            // The answer to the handshake's `Hello`, after the device's start
            Input::Device(DeviceMessage::Ready { boot: false, .. }) => {}
            Input::Device(DeviceMessage::Ready { scale_factor, .. }) => {
                self.showing = None;
                self.device.forget();
                self.device.epoch += 1;
                self.device.scale_factor = scale_factor;
                // It only restarts on its own when it panics
                if let Some(sent) = self.sent.take() {
                    self.crashed = Some(sent);
                    let panic = self.last_panic.take().map_or(String::new(), |p| format!(": {p}"));
                    self.report_problem(Some(format!(
                        "The device restarted while showing the design{panic}"
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
                match id {
                    // A `Show` failed, which leaves the blob being stored alone
                    Some(id) => {
                        self.showing.take_if(|showing| showing.id == id);
                    }
                    None => self.device.epoch += 1,
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
                // The board's panic handler prints this before it restarts the board
                if line.starts_with("Panic") {
                    self.last_panic = Some(line);
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
        if self.device.shown.as_ref() == Some(&shown) {
            self.report_problem(None);
            return Ok(Flow::Continue);
        }
        if self.crashed.as_ref() == Some(&shown) {
            return Ok(Flow::Continue);
        }
        let show = HostMessage::Show {
            id: self.last_id + 1,
            component: shown.component.clone(),
            unit: shown.unit,
            resources: shown.resources.clone(),
        };
        let show = encode_frame(&show);
        if show.len() > MAX_FRAME_LEN {
            self.report_problem(Some(format!(
                "The design has too many images and fonts for the device: {}",
                shown.resources.len()
            )));
            return Ok(Flow::Continue);
        }
        self.sent = Some(shown.clone());
        let epoch = self.device.epoch;
        let started = Instant::now();
        let mut sent = 0;
        let shown_unit = self.device.shown.as_ref().map(|shown| shown.unit);
        let unit = (shown_unit != Some(shown.unit)).then_some((shown.unit, &compiled.unit, false));
        let resources = shown.resources.iter().zip(&compiled.resources);
        let mut listed = HashSet::new();
        let resources = resources
            .filter(|(key, _)| !self.device.resources.contains(key) && listed.insert(**key))
            .map(|(key, resource)| (*key, resource, true));
        let blobs = unit.into_iter().chain(resources).collect::<Vec<_>>();
        for (key, blob, resource) in blobs {
            for frame in store_frames(key, blob, resource) {
                write_patiently(&mut self.port, &frame)
                    .map_err(|err| format!("Cannot send the design: {err}"))?;
                while let Ok(input) = self.inputs.try_recv() {
                    if let Flow::Stop = self.handle(input)? {
                        return Ok(Flow::Stop);
                    }
                }
                // The design changed, so the next store supersedes this one,
                // or the device dropped this one
                if self.send_at.is_some() || self.device.epoch != epoch {
                    return Ok(Flow::Continue);
                }
            }
            if resource {
                self.device.resources.insert(key);
            }
            sent += blob.len();
        }
        write_patiently(&mut self.port, &show)
            .map_err(|err| format!("Cannot send the design: {err}"))?;
        self.last_id += 1;
        self.device.resources = shown.resources.iter().copied().collect();
        self.device.shown = Some(shown);
        self.showing = Some(Showing { id: self.last_id, sent, started });
        Ok(Flow::Continue)
    }
}

fn log_to(report: &dyn Fn(SerialEvent), message: String) {
    report(SerialEvent::Message(PreviewToLspMessage::DebugMessage { location: None, message }));
}

/// Asks the device what it is, and checks that it runs the same Slint as this host.
/// Returns `None` if the session was stopped meanwhile.
fn handshake(
    port: &mut impl Write,
    inputs: &mpsc::Receiver<Input>,
    design: &mut Design,
    report: &dyn Fn(SerialEvent),
) -> Result<Option<Device>, String> {
    let log = |message| log_to(report, message);
    port.write_all(&encode_frame(&HostMessage::Hello))
        .map_err(|err| format!("Cannot write to the port: {err}"))?;
    let deadline = Instant::now() + READY_TIMEOUT;
    loop {
        let input = inputs.recv_timeout(deadline.saturating_duration_since(Instant::now()));
        match input {
            Ok(Input::Device(DeviceMessage::Ready {
                protocol,
                slint_version,
                board,
                scale_factor,
                ..
            })) => {
                // A unit only reads back with an interpreter of the same Slint version
                if slint_version != SLINT_VERSION {
                    return Err(format!(
                        "The device runs Slint {slint_version}, not {SLINT_VERSION}: flash the firmware of Slint {SLINT_VERSION}"
                    ));
                }
                if protocol != PROTOCOL_VERSION {
                    return Err(format!(
                        "The device speaks protocol {protocol}, not {PROTOCOL_VERSION}: flash the viewer firmware of this exact Slint version"
                    ));
                }
                log(format!("Connected to {board}"));
                return Ok(Some(Device::new(scale_factor)));
            }
            Ok(Input::Device(DeviceMessage::Error { message, .. })) => return Err(message),
            Ok(Input::Device(DeviceMessage::Loaded { .. })) => {}
            Ok(Input::Lsp(message)) => {
                design.apply(*message);
            }
            Ok(Input::DeviceOutput(line)) => log(format!("[device] {line}")),
            Ok(Input::PortClosed(err)) => return Err(format!("Lost the connection: {err}")),
            Ok(Input::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(None),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                return Err(
                    "The device didn't answer: does it run the viewer firmware of this exact Slint version?".into()
                );
            }
        }
    }
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
    use i_slint_core::InternalToken;
    use i_slint_live_preview::protocol::VersionedUrl;
    use i_slint_live_preview::serial::{DeviceEvent, DeviceSession};
    use slint_interpreter::LoadedResource;

    /// Plays the device: starts, answers the handshake with `slint_version`, and reads back
    /// each unit it receives. Returns the components it showed, once the host cleared the screen.
    fn fake_device(
        mut from_host: impl Read,
        mut to_host: impl Write,
        slint_version: &str,
    ) -> Vec<String> {
        let mut session = DeviceSession::new(1 << 24);
        let mut loaded = HashMap::new();
        let mut shown = Vec::new();
        let ready = |boot| {
            encode_frame(&DeviceMessage::Ready {
                boot,
                protocol: PROTOCOL_VERSION,
                slint_version: slint_version.into(),
                board: "fake".into(),
                scale_factor: 1.,
            })
        };
        to_host.write_all(&ready(true)).unwrap();
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
                        to_host.write_all(&ready(false)).unwrap();
                    }
                    DeviceEvent::Resource { key, data } => {
                        let resource = serialize::resource_from_bytes(&data).unwrap();
                        loaded.insert(key, LoadedResource::new(resource, InternalToken));
                    }
                    DeviceEvent::Show { id, component, unit, resources } => {
                        loaded.retain(|key, _| resources.contains(key));
                        let unit = serialize::from_bytes(session.blob(unit).unwrap()).unwrap();
                        let resources = resources.iter().map(|key| loaded[key].clone());
                        let result = slint_interpreter::CompilationResult::from_compilation_unit(
                            unit,
                            resources,
                            InternalToken,
                        );
                        assert!(result.component(&component).is_some());
                        to_host
                            .write_all(&encode_frame(&DeviceMessage::Loaded { id, heap_free: 0 }))
                            .unwrap();
                        if component == "Crash" {
                            // Like the board's panic handler, which then restarts it
                            to_host.write_all(b"Panic at main.rs:1: out of memory\n").unwrap();
                            to_host.write_all(b"Peripherals initialized\n").unwrap();
                            to_host.write_all(&ready(true)).unwrap();
                            shown.push("crashed".into());
                            continue;
                        }
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

    /// Starts a session against a fake device, which the editor feeds `messages` once connected.
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
        spawn_reader(host_reader, inputs.clone(), Arc::default());
        let (problems, problem_receiver) = mpsc::channel();
        let feeder = inputs.clone();
        let session = std::thread::spawn(move || {
            let report = |event| match event {
                SerialEvent::Connected { problem } => {
                    let _ = problems.send(problem);
                }
                SerialEvent::Message(PreviewToLspMessage::DebugMessage { message, .. }) => {
                    let _ = logs.send(message);
                }
                SerialEvent::Message(PreviewToLspMessage::RequestState { .. }) => {
                    for message in &messages {
                        let _ = feeder.send(Input::Lsp(Box::new(message.clone())));
                    }
                }
                _ => {}
            };
            run_session(host_writer, &input_receiver, &report)
        });
        FakeSession { inputs, logs: log_receiver, problems: problem_receiver, session, device }
    }

    /// Runs a session against a fake device, feeding it `messages` from the editor once
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
        let set_contents = |path: &str, contents: &str| LspToPreviewMessage::SetContents {
            url: VersionedUrl::new(Url::from_file_path(dir.path().join(path)).unwrap(), None),
            contents: contents.as_bytes().to_vec(),
        };
        let mut messages = vec![
            LspToPreviewMessage::OpenProject {
                root: Some(Url::from_directory_path(dir.path()).unwrap()),
            },
            set_contents(
                "slint-project.json",
                r#"{ "slint-edition": "1.19", "include-paths": ["include"] }"#,
            ),
        ];
        // The compiler looks for imports in include paths on disk only
        std::fs::create_dir(dir.path().join("include")).unwrap();
        std::fs::write(
            dir.path().join("include/red.slint"),
            "export component Red inherits Rectangle { background: red; }",
        )
        .unwrap();
        messages.extend(show(
            &url,
            r#"import { Red } from "red.slint"; export component Main inherits Window { width: 32px; height: 32px; Red { } }"#,
        ));
        let (ended, logs, shown) =
            run_against_fake_device(SLINT_VERSION, messages, Some("The device shows "));
        assert_eq!(ended, Ok(()));
        assert_eq!(shown, ["Main"], "{logs:?}");
        assert!(logs.iter().any(|line| line == "[device] a log line from the device"), "{logs:?}");
        // The answer to `Hello` after the device's start isn't a restart
        assert!(!logs.iter().any(|line| line.starts_with("The device restarted")), "{logs:?}");
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
            Input::Lsp(Box::new(LspToPreviewMessage::SetContents {
                url: VersionedUrl::new(url.clone(), None),
                contents: contents.into_bytes(),
            }))
        };
        let fake = start_against_fake_device(SLINT_VERSION, show(&url, source));
        fake.wait_for("The device shows ");
        // The same unit, compiled again
        fake.inputs.send(edit(format!("// A comment\n{source}"))).unwrap();
        std::thread::sleep(REBUILD_DEBOUNCE * 3);
        fake.inputs.send(edit(source.replace("red", "blue"))).unwrap();
        let seen = fake.wait_for("The device shows ");
        // An error, undone: the device already shows the design, and the error goes away
        fake.inputs.send(edit(format!("{source} syntax error"))).unwrap();
        fake.wait_for("The design has errors");
        fake.inputs.send(edit(source.replace("red", "blue"))).unwrap();
        let problems =
            std::iter::from_fn(|| fake.problems.recv_timeout(Duration::from_secs(10)).ok());
        assert!(problems.skip_while(Option::is_none).any(|problem| problem.is_none()));
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
        let seen = fake.wait_for("The device restarted while showing the design");
        assert!(seen.last().unwrap().ends_with(": Panic at main.rs:1: out of memory"), "{seen:?}");
        // Reported in the preview, after the connection itself and the design it showed
        let problems: Vec<_> = fake.problems.try_iter().collect();
        assert!(
            matches!(problems.as_slice(), [None, None, Some(p)] if p.contains("Panic")),
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
}
