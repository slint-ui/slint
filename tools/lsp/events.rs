// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::{collections::HashSet, path::PathBuf};

use i_slint_compiler::diagnostics::BuildDiagnostics;
use lsp_protocol::PreviewConfig;
use lsp_types::Url;

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
