use std::{collections::HashSet, path::PathBuf};

use i_slint_compiler::diagnostics::BuildDiagnostics;
use lsp_protocol::PreviewConfig;
use lsp_types::{InitializeParams, Url};

pub struct SetContextEvent(pub InitializeParams, pub Option<PreviewConfig>);

pub struct SendDiagnosticsEvent {
    pub extra_files: HashSet<PathBuf>,
    pub diag: BuildDiagnostics,
}

pub struct ConfigurePreviewEvent {
    pub config: PreviewConfig,
    pub doc_count: usize,
}

pub struct LoadDocumentEvent {
    pub content: String,
    pub url: Url,
    pub version: Option<i32>,
}

pub struct RecompileTimerEvent;

pub struct AddRecompile(pub HashSet<Url>);
