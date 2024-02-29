// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

#![cfg(any(feature = "preview-external", feature = "preview-engine"))]

use crate::Context;

use crate::common::{self, Result};
use crate::language;
use crate::util;

#[cfg(target_arch = "wasm32")]
use crate::wasm_prelude::*;

use i_slint_compiler::object_tree::ElementRc;
use lsp_types::{Url, WorkspaceEdit};

pub fn element_at_source_code_position(
    dc: &mut language::DocumentCache,
    position: &common::VersionedPosition,
) -> Result<ElementRc> {
    let file = Url::to_file_path(position.url())
        .map_err(|_| "Failed to convert URL to file path".to_string())?;

    if &dc.document_version(position.url()) != position.version() {
        return Err("Document version mismatch.".into());
    }

    let doc = dc.documents.get_document(&file).ok_or_else(|| "Document not found".to_string())?;

    let source_file = doc
        .node
        .as_ref()
        .map(|n| n.source_file.clone())
        .ok_or_else(|| "Document had no node".to_string())?;
    let element_position = util::map_position(&source_file, position.offset().into());

    Ok(language::element_at_position(dc, &position.url(), &element_position).ok_or_else(|| {
        format!("No element found at the given start position {:?}", &element_position)
    })?)
}

pub fn update_element(
    ctx: &Context,
    position: common::VersionedPosition,
    properties: Vec<common::PropertyChange>,
) -> Result<WorkspaceEdit> {
    let element = element_at_source_code_position(&mut ctx.document_cache.borrow_mut(), &position)?;

    let (_, e) = language::properties::set_bindings(
        &mut ctx.document_cache.borrow_mut(),
        position.url(),
        &element,
        &properties,
    )?;
    Ok(e.ok_or_else(|| "Failed to create workspace edit".to_string())?)
}
