// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Editor side of the remote preview, without an editor: `slint-viewer --connect`.
//!
//! Pairs with a viewer started with `--remote` (or the Slint Viewer app), then
//! serves it the `.slint` file given on the command line, and whatever that
//! file pulls in, from the local disk. Nothing here touches the Slint
//! platform, so it runs where no window can be opened, such as over SSH or
//! in Termux.

use std::collections::{HashMap, HashSet};
use std::io::IsTerminal as _;
use std::path::{Path, PathBuf};
use std::time::Instant;

use futures_util::stream::{SplitSink, StreamExt as _};
use futures_util::{FutureExt as _, SinkExt as _};
use i_slint_compiler::parser::{SyntaxKind, syntax_nodes};
use i_slint_compiler::source_path::{SourcePath, clean_path};
use i_slint_editor_preview::preview_log_message_to_string;
use i_slint_editor_preview::remote_authentication::{
    self, PairingAnswer, PairingInputFuture, PairingPrompt,
};
use i_slint_editor_preview::remote_client::{self, CONNECT_TIMEOUT, PING_INTERVAL, PONG_TIMEOUT};
use i_slint_live_preview::file_watcher::{FileChangeKind, FileWatcher, WatchEvent};
use i_slint_live_preview::protocol::lsp_types::{self, Url};
use i_slint_live_preview::protocol::{
    LspToPreviewMessage, PROTOCOL_SUBPROTOCOL, PreviewComponent, PreviewConfig,
    PreviewToLspMessage, VersionedUrl, session,
};
use tokio::sync::mpsc;
use tokio_tungstenite_wasm::{Message, WebSocketStream};

type Error = Box<dyn std::error::Error>;
type Result<T> = std::result::Result<T, Error>;

/// The images and fonts a `.slint` can reference. Mirrors the list the LSP
/// applies in `tools/lsp/language/preview_files.rs`.
const ASSET_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "svg", "svgz", "gif", "webp", "bmp", "ico", "avif", "ttf", "ttc", "otf",
];

pub struct Options {
    /// `host:port` of the viewer.
    pub target: String,
    pub path: PathBuf,
    pub component: Option<String>,
    /// Tried first; the user is prompted if the viewer rejects it.
    pub pairing_code: Option<String>,
    /// Connect even if the viewer has pairing disabled, so the session is
    /// not encrypted.
    pub accept_unpaired: bool,
    pub auto_reload: bool,
    pub config: PreviewConfig,
}

pub fn run(options: Options) -> Result<()> {
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
    runtime.block_on(run_async(options))
}

async fn run_async(options: Options) -> Result<()> {
    let main_file = clean_path(&std::path::absolute(&options.path)?);
    if !main_file.is_file() {
        return Err(format!("{} is not a file", main_file.display()).into());
    }
    let main_url = Url::from_file_path(&main_file)
        .map_err(|()| format!("Cannot convert {} to a URL", main_file.display()))?;

    let mut stream = dial(&options.target).await?;
    let session = authenticate(&mut stream, &options).await?;
    eprintln!("Connected to {}, previewing {}", options.target, main_file.display());

    let (sender, mut receiver) = stream.split();
    let (watch_sender, mut watch_events) = mpsc::unbounded_channel();
    let watcher = if options.auto_reload {
        Some(FileWatcher::start(
            move |event| {
                let _ = watch_sender.send(event);
            },
            |error| tracing::warn!("File watcher error: {error}"),
        )?)
    } else {
        None
    };

    let mut client = Client {
        sender,
        sealing: session.sealing,
        access: FileAccess::new(&main_file, &options.config),
        preview: PreviewComponent { url: main_url, component: options.component.clone() },
        config: options.config,
        main_file,
        sent: HashSet::new(),
        fonts_sent: HashSet::new(),
        diagnostics: HashMap::new(),
        watcher,
    };
    let mut opening = session.opening;

    if let Some(request) = session.initial_request {
        client.handle(request).await?;
    }

    let mut ping = tokio::time::interval(PING_INTERVAL);
    let mut last_pong = Instant::now();
    loop {
        tokio::select! {
            frame = receiver.next() => {
                let bytes = match frame {
                    Some(Ok(Message::Binary(bytes))) => bytes,
                    Some(Ok(Message::Text(text))) => {
                        tracing::warn!("Ignoring text message from the viewer: {text}");
                        continue;
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        eprintln!("The viewer closed the connection");
                        return Ok(());
                    }
                    Some(Err(error)) => return Err(format!("Connection to the viewer lost: {error}").into()),
                };
                // A frame that fails to open was tampered with or reordered,
                // and the counters can't recover from that.
                let plain = opening
                    .open(&bytes)
                    .map_err(|_| "A frame from the viewer failed to open; closing the session")?;
                match postcard::from_bytes::<PreviewToLspMessage>(&plain) {
                    Ok(PreviewToLspMessage::Pong) => last_pong = Instant::now(),
                    Ok(message) if remote_client::is_allowed_message(&message) => {
                        client.handle(message).await?
                    }
                    Ok(message) => tracing::debug!("Ignoring {message:?} from the viewer"),
                    Err(error) => tracing::error!("Failed decoding a message from the viewer: {error}"),
                }
            }
            _ = ping.tick() => {
                if last_pong.elapsed() > PONG_TIMEOUT {
                    return Err("The viewer stopped responding".into());
                }
                client.send(&LspToPreviewMessage::Ping).await?;
            }
            Some(event) = watch_events.recv() => client.file_changed(event).await?,
        }
    }
}

async fn dial(target: &str) -> Result<WebSocketStream> {
    let url = format!("ws://{target}");
    let connect = tokio_tungstenite_wasm::connect_with_protocols(&url, &[PROTOCOL_SUBPROTOCOL]);
    match tokio::time::timeout(CONNECT_TIMEOUT, connect).await {
        Ok(Ok(stream)) => Ok(stream),
        Ok(Err(error)) => {
            Err(remote_authentication::describe_version_mismatch(&error, "slint-viewer")
                .unwrap_or_else(|| format!("Cannot connect to the viewer at {target}: {error}"))
                .into())
        }
        Err(_) => Err(format!("Connecting to the viewer at {target} timed out").into()),
    }
}

async fn authenticate(
    stream: &mut WebSocketStream,
    options: &Options,
) -> Result<remote_authentication::AuthenticatedSession> {
    let mut given_code = options.pairing_code.clone();
    let accept_unpaired = options.accept_unpaired;
    let prompt = move |prompt: PairingPrompt| -> PairingInputFuture {
        match prompt {
            PairingPrompt::Code { attempts_left, expires_in_seconds, code_digits } => {
                if let Some(code) = given_code.take() {
                    return Box::pin(std::future::ready(PairingAnswer::Code(code)));
                }
                if !std::io::stdin().is_terminal() {
                    eprintln!("The viewer asks for a pairing code: pass it with --pairing-code");
                    return Box::pin(std::future::ready(PairingAnswer::Cancel));
                }
                eprint!(
                    "Enter the {code_digits}-digit code shown on the viewer \
                     ({attempts_left} attempts, {expires_in_seconds}s left): "
                );
                Box::pin(read_code().map(|code| match code {
                    Some(code) => PairingAnswer::Code(code),
                    None => PairingAnswer::Cancel,
                }))
            }
            PairingPrompt::Unpaired if accept_unpaired => {
                eprintln!("Warning: the viewer has pairing disabled, the session is not encrypted");
                Box::pin(std::future::ready(PairingAnswer::AcceptUnpaired))
            }
            PairingPrompt::Unpaired => {
                eprintln!(
                    "The viewer has pairing disabled, so the session would not be encrypted. \
                     Pass --no-pairing to connect anyway."
                );
                Box::pin(std::future::ready(PairingAnswer::Cancel))
            }
        }
    };
    // Each run pairs afresh, so there is no reconnect token worth keeping.
    remote_authentication::authenticate(stream, None, prompt, |_| {})
        .await
        .map_err(|error| format!("Pairing with the viewer failed: {error}").into())
}

/// Read a line from the terminal without blocking the event loop, which has
/// to keep answering the viewer while the user types.
async fn read_code() -> Option<String> {
    tokio::task::spawn_blocking(|| {
        let mut line = String::new();
        std::io::stdin().read_line(&mut line).ok().filter(|read| *read > 0)?;
        Some(line.trim().to_owned())
    })
    .await
    .ok()
    .flatten()
}

struct Client {
    sender: SplitSink<WebSocketStream, Message>,
    sealing: session::Sealing,
    access: FileAccess,
    preview: PreviewComponent,
    config: PreviewConfig,
    main_file: PathBuf,
    /// Files the viewer holds a copy of, which a change on disk must reach.
    sent: HashSet<PathBuf>,
    fonts_sent: HashSet<PathBuf>,
    /// Last diagnostics printed per file, so a rebuild that changes nothing
    /// doesn't print them again.
    diagnostics: HashMap<Url, Vec<String>>,
    watcher: Option<FileWatcher>,
}

impl Client {
    async fn send(&mut self, message: &LspToPreviewMessage) -> Result<()> {
        tracing::debug!("Sending {message:?}");
        let bytes = postcard::to_allocvec(message)?;
        let frame = self.sealing.seal(bytes).map_err(|_| "Session counter exhausted")?;
        self.sender.send(Message::binary(frame)).await?;
        Ok(())
    }

    async fn handle(&mut self, message: PreviewToLspMessage) -> Result<()> {
        use PreviewToLspMessage as M;
        match message {
            M::RequestState { files, .. } => {
                if files.is_empty() {
                    self.send_state().await?;
                } else {
                    for url in files {
                        self.send_file(&url).await?;
                    }
                }
            }
            M::Diagnostics { uri, diagnostics, .. } => self.print_diagnostics(uri, &diagnostics),
            M::DebugMessage { location, message } => {
                eprintln!("{}", preview_log_message_to_string(&location, &message));
            }
            message => tracing::debug!("Ignoring {message:?} from the viewer"),
        }
        Ok(())
    }

    /// Everything a freshly started preview needs: the files it held before
    /// (none on the first request), the configuration, and what to show.
    async fn send_state(&mut self) -> Result<()> {
        let mut files: Vec<PathBuf> = self.sent.iter().cloned().collect();
        if !self.sent.contains(&self.main_file) {
            files.push(self.main_file.clone());
        }
        self.fonts_sent.clear();
        for path in files {
            self.send_path(path).await?;
        }
        self.send(&LspToPreviewMessage::SetConfiguration { config: self.config.clone() }).await?;
        self.send(&LspToPreviewMessage::ShowPreview(self.preview.clone())).await
    }

    async fn send_file(&mut self, url: &Url) -> Result<()> {
        let Ok(path) = url.to_file_path() else {
            tracing::warn!("Viewer asked for {url}, which is not a local file");
            return self.send(&LspToPreviewMessage::ForgetFile { url: url.clone() }).await;
        };
        let path = clean_path(&path);
        if path != self.main_file && !self.access.allows(&path) {
            tracing::warn!(
                "Refusing to send {} to the viewer: not a file of the project being previewed",
                path.display()
            );
            return self.send(&LspToPreviewMessage::ForgetFile { url: url.clone() }).await;
        }
        self.send_path(path).await
    }

    async fn send_path(&mut self, path: PathBuf) -> Result<()> {
        let Ok(url) = Url::from_file_path(&path) else { return Ok(()) };
        let newly_sent = self.sent.insert(path.clone());
        match std::fs::read(&path) {
            Ok(contents) => {
                let fonts = if is_slint_source(&path) {
                    font_imports(&path, &contents, &self.config.include_paths)
                } else {
                    Vec::new()
                };
                self.send(&LspToPreviewMessage::SetContents {
                    url: VersionedUrl::new(url, None),
                    contents,
                })
                .await?;
                for font in fonts {
                    self.send_font(font).await?;
                }
            }
            Err(error) => {
                tracing::warn!("Failed to read {}: {error}", path.display());
                self.send(&LspToPreviewMessage::ForgetFile { url }).await?;
            }
        }
        if newly_sent {
            self.update_watches();
        }
        Ok(())
    }

    /// Fonts are pushed rather than requested: the viewer can't tell a font
    /// import resolves to a file that only exists on this machine.
    async fn send_font(&mut self, path: PathBuf) -> Result<()> {
        if !self.fonts_sent.insert(path.clone()) {
            return Ok(());
        }
        let Ok(url) = Url::from_file_path(&path) else { return Ok(()) };
        match std::fs::read(&path) {
            Ok(contents) => {
                self.send(&LspToPreviewMessage::SetContents {
                    url: VersionedUrl::new(url, None),
                    contents,
                })
                .await?;
            }
            Err(error) => tracing::warn!("Failed to read font {}: {error}", path.display()),
        }
        if self.sent.insert(path) {
            self.update_watches();
        }
        Ok(())
    }

    async fn file_changed(&mut self, event: WatchEvent) -> Result<()> {
        let path = clean_path(&event.path);
        if !self.sent.contains(&path) {
            return Ok(());
        }
        match event.kind {
            FileChangeKind::Created | FileChangeKind::Changed => {
                // A changed font has to be pushed again; the viewer would
                // otherwise keep the old one.
                self.fonts_sent.remove(&path);
                if i_slint_compiler::fileaccess::is_font_file(&path.to_string_lossy()) {
                    self.send_font(path).await
                } else {
                    self.send_path(path).await
                }
            }
            FileChangeKind::Deleted => {
                let Ok(url) = Url::from_file_path(&path) else { return Ok(()) };
                self.send(&LspToPreviewMessage::ForgetFile { url }).await
            }
        }
    }

    fn update_watches(&mut self) {
        let Some(watcher) = self.watcher.as_mut() else { return };
        if let Err(error) = watcher.update_watched_paths(self.sent.iter().cloned()) {
            tracing::warn!("Failed to watch files: {error}");
        }
    }

    fn print_diagnostics(&mut self, uri: Url, diagnostics: &[lsp_types::Diagnostic]) {
        let file =
            uri.to_file_path().map_or_else(|()| uri.to_string(), |p| p.display().to_string());
        let lines: Vec<String> = diagnostics
            .iter()
            .map(|diagnostic| {
                let severity = match diagnostic.severity {
                    Some(lsp_types::DiagnosticSeverity::ERROR) => "error",
                    Some(lsp_types::DiagnosticSeverity::WARNING) => "warning",
                    _ => "note",
                };
                let start = diagnostic.range.start;
                format!(
                    "{file}:{}:{}: {severity}: {}",
                    start.line + 1,
                    start.character + 1,
                    diagnostic.message
                )
            })
            .collect();
        let previous = self.diagnostics.insert(uri, lines.clone()).unwrap_or_default();
        if previous == lines {
            return;
        }
        if lines.is_empty() {
            eprintln!("{file}: no issues");
        }
        for line in lines {
            eprintln!("{line}");
        }
    }
}

/// Which files the viewer may be sent. The viewer is on another device and
/// names the files it wants, so this keeps it to the project: files under
/// the previewed file's directory or the include and library paths, that
/// are sources or assets. Mirrors the LSP's `PreviewFileAccess`.
struct FileAccess {
    roots: Vec<PathBuf>,
}

impl FileAccess {
    fn new(main_file: &Path, config: &PreviewConfig) -> Self {
        let roots = main_file
            .parent()
            .map(Path::to_path_buf)
            .into_iter()
            .chain(config.include_paths.iter().cloned())
            .chain(config.library_paths.values().cloned())
            .filter_map(|path| {
                // Canonical, so a request can't escape with `..` or a symlink.
                let path = path.canonicalize().ok()?;
                if path.is_dir() { Some(path) } else { Some(path.parent()?.to_path_buf()) }
            })
            .collect();
        Self { roots }
    }

    fn allows(&self, path: &Path) -> bool {
        let Ok(canonical) = path.canonicalize() else { return false };
        self.roots.iter().any(|root| canonical.starts_with(root))
            && (is_slint_source(path) || is_asset(path))
    }
}

fn is_slint_source(path: &Path) -> bool {
    path.extension().is_some_and(|extension| extension == "slint")
}

fn is_asset(path: &Path) -> bool {
    path.extension().is_some_and(|extension| {
        ASSET_EXTENSIONS.iter().any(|asset| extension.eq_ignore_ascii_case(asset))
    })
}

/// The font files `source` imports with `import "font.ttf";`, resolved the
/// way the compiler does: against the importing file's directory first,
/// then the include paths.
fn font_imports(path: &Path, source: &[u8], include_paths: &[PathBuf]) -> Vec<PathBuf> {
    let Ok(source) = std::str::from_utf8(source) else { return Vec::new() };
    let mut diagnostics = i_slint_compiler::diagnostics::BuildDiagnostics::default();
    let node = i_slint_compiler::parser::parse(
        source.to_owned(),
        Some(SourcePath::new(path)),
        &mut diagnostics,
    );
    let Some(document) = syntax_nodes::Document::new(node) else { return Vec::new() };
    let directory = path.parent().unwrap_or(Path::new(""));
    document
        .ImportSpecifier()
        .filter(|import| import.ImportIdentifierList().is_none())
        .filter_map(|import| import.child_token(SyntaxKind::StringLiteral))
        .map(|token| token.text().trim_matches('"').to_owned())
        .filter(|file| i_slint_compiler::fileaccess::is_font_file(file))
        .map(|file| {
            std::iter::once(directory)
                .chain(include_paths.iter().map(PathBuf::as_path))
                .map(|base| clean_path(&base.join(&file)))
                .find(|candidate| candidate.is_file())
                .unwrap_or_else(|| clean_path(&directory.join(&file)))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_font_imports_only() {
        let dir = tempfile::tempdir().unwrap();
        let main = dir.path().join("main.slint");
        std::fs::create_dir(dir.path().join("fonts")).unwrap();
        std::fs::write(dir.path().join("fonts/a.ttf"), b"").unwrap();
        let source = br#"
            import "fonts/a.ttf";
            import { Button } from "std-widgets.slint";
            import "missing.otf";
            export component App inherits Window {}
        "#;
        assert_eq!(
            font_imports(&main, source, &[]),
            vec![dir.path().join("fonts/a.ttf"), dir.path().join("missing.otf")]
        );
    }

    #[test]
    fn file_access_stays_within_the_project() {
        let project = tempfile::tempdir().unwrap();
        let elsewhere = tempfile::tempdir().unwrap();
        let main = project.path().join("main.slint");
        for file in ["main.slint", "logo.png", ".env"] {
            std::fs::write(project.path().join(file), b"").unwrap();
        }
        std::fs::write(elsewhere.path().join("secret.slint"), b"").unwrap();
        let access = FileAccess::new(&main, &PreviewConfig::default());
        assert!(access.allows(&project.path().join("logo.png")));
        assert!(access.allows(&main));
        assert!(!access.allows(&project.path().join(".env")));
        assert!(!access.allows(&elsewhere.path().join("secret.slint")));
        assert!(!access.allows(&project.path().join("../secret.slint")));
    }
}
