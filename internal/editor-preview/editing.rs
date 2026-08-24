// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::DocumentCache;
use i_slint_compiler::parser::TextSize;
use i_slint_live_preview::protocol::{SourceFileVersion, VersionedUrl};
use lsp_types::{TextEdit, Url, WorkspaceEdit};
use std::{collections::HashMap, path::Path};

#[cfg(target_arch = "wasm32")]
use crate::wasm_prelude::*;

pub mod import_edit;
pub mod rename_component;
pub mod rename_element_id;
#[cfg(any(test, feature = "preview-engine"))]
pub mod text_edit;

pub struct SingleTextEdit {
    pub url: Url,
    pub version: SourceFileVersion,
    pub edit: TextEdit,
}

impl SingleTextEdit {
    pub fn from_path(document_cache: &DocumentCache, path: &Path, edit: TextEdit) -> Option<Self> {
        let url = Url::from_file_path(path).ok()?;
        let version = document_cache.document_version_by_path(path);
        Some(Self { url, version, edit })
    }
}

pub fn create_text_document_edit(
    uri: Url,
    version: SourceFileVersion,
    edits: Vec<TextEdit>,
) -> lsp_types::TextDocumentEdit {
    let edits = edits
        .into_iter()
        .map(lsp_types::OneOf::Left::<TextEdit, lsp_types::AnnotatedTextEdit>)
        .collect();
    lsp_types::TextDocumentEdit {
        text_document: lsp_types::OptionalVersionedTextDocumentIdentifier { uri, version },
        edits,
    }
}

pub fn create_workspace_edit_from_path(
    document_cache: &DocumentCache,
    path: &Path,
    edits: Vec<TextEdit>,
) -> Option<WorkspaceEdit> {
    let url = Url::from_file_path(path).ok()?;
    let version = document_cache.document_version_by_path(path);
    Some(create_workspace_edit(url, version, edits))
}

pub fn create_workspace_edit(
    url: Url,
    version: SourceFileVersion,
    edits: Vec<TextEdit>,
) -> WorkspaceEdit {
    create_workspace_edit_from_text_document_edits(vec![create_text_document_edit(
        url, version, edits,
    )])
}

pub fn create_workspace_edit_from_text_document_edits(
    edits: Vec<lsp_types::TextDocumentEdit>,
) -> WorkspaceEdit {
    let document_changes = Some(lsp_types::DocumentChanges::Edits(edits));
    WorkspaceEdit { document_changes, ..Default::default() }
}

#[cfg(any(test, feature = "testing"))]
/// Merges the document-change edits from `additional` into `base`.
pub fn merge_workspace_edits(base: &mut WorkspaceEdit, additional: WorkspaceEdit) {
    debug_assert!(
        additional.changes.is_none() && additional.change_annotations.is_none(),
        "merge_workspace_edits only merges document_changes; got changes/change_annotations on additional",
    );
    let Some(lsp_types::DocumentChanges::Edits(more)) = additional.document_changes else {
        return;
    };
    match &mut base.document_changes {
        Some(lsp_types::DocumentChanges::Edits(existing)) => existing.extend(more),
        None => {
            base.document_changes = Some(lsp_types::DocumentChanges::Edits(more));
        }
        Some(lsp_types::DocumentChanges::Operations(_)) => {}
    }
}

pub fn create_workspace_edit_from_single_text_edits(inputs: Vec<SingleTextEdit>) -> WorkspaceEdit {
    let mut files: HashMap<
        (Url, SourceFileVersion),
        Vec<lsp_types::OneOf<TextEdit, lsp_types::AnnotatedTextEdit>>,
    > = HashMap::new();
    inputs.into_iter().for_each(|single_edit| {
        let edit = lsp_types::OneOf::Left(single_edit.edit);
        files
            .entry((single_edit.url, single_edit.version))
            .and_modify(|edits| edits.push(edit.clone()))
            .or_insert_with(|| vec![edit]);
    });

    let changes = lsp_types::DocumentChanges::Edits(
        files
            .into_iter()
            .map(|((uri, version), edits)| lsp_types::TextDocumentEdit {
                text_document: lsp_types::OptionalVersionedTextDocumentIdentifier { uri, version },
                edits,
            })
            .collect::<Vec<_>>(),
    );

    WorkspaceEdit { document_changes: Some(changes), ..Default::default() }
}

/// A versioned file position.
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
pub struct VersionedPosition {
    /// The file URL and version.
    url: VersionedUrl,
    /// The offset in the file.
    offset: u32,
}

#[allow(unused)]
impl VersionedPosition {
    pub fn new(url: VersionedUrl, offset: TextSize) -> Self {
        Self { url, offset: offset.into() }
    }

    pub fn url(&self) -> &Url {
        self.url.url()
    }

    pub fn version(&self) -> &SourceFileVersion {
        self.url.version()
    }

    pub fn offset(&self) -> TextSize {
        self.offset.into()
    }
}

#[allow(unused)]
#[derive(Clone, Eq, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct PropertyChange {
    pub name: String,
    pub value: String,
}

impl PropertyChange {
    #[allow(unused)]
    pub fn new(name: &str, value: String) -> Self {
        PropertyChange { name: name.to_string(), value }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edit(uri: &str, new_text: &str) -> SingleTextEdit {
        SingleTextEdit {
            url: Url::parse(uri).unwrap(),
            version: None,
            edit: TextEdit {
                range: lsp_types::Range::new(
                    lsp_types::Position::new(0, 0),
                    lsp_types::Position::new(0, 0),
                ),
                new_text: new_text.into(),
            },
        }
    }

    fn document_edits(edit: &WorkspaceEdit) -> &Vec<lsp_types::TextDocumentEdit> {
        match edit.document_changes.as_ref().unwrap() {
            lsp_types::DocumentChanges::Edits(document_edits) => document_edits,
            _ => panic!("expected DocumentChanges::Edits"),
        }
    }

    #[test]
    fn merge_workspace_edits_combines_edits_lists() {
        let mut base =
            create_workspace_edit_from_single_text_edits(vec![edit("file:///a.slint", "a")]);
        let additional =
            create_workspace_edit_from_single_text_edits(vec![edit("file:///b.rs", "b")]);
        merge_workspace_edits(&mut base, additional);
        assert_eq!(document_edits(&base).len(), 2);
    }

    #[test]
    fn merge_workspace_edits_into_empty_base() {
        let mut base = WorkspaceEdit::default();
        let additional =
            create_workspace_edit_from_single_text_edits(vec![edit("file:///b.rs", "b")]);
        merge_workspace_edits(&mut base, additional);
        assert_eq!(document_edits(&base).len(), 1);
    }

    #[test]
    fn merge_workspace_edits_noop_for_empty_additional() {
        let mut base =
            create_workspace_edit_from_single_text_edits(vec![edit("file:///a.slint", "a")]);
        let before_len = document_edits(&base).len();
        merge_workspace_edits(&mut base, WorkspaceEdit::default());
        assert_eq!(document_edits(&base).len(), before_len);
    }
}
