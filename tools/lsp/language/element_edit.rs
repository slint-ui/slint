// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

#![cfg(any(feature = "preview-external", feature = "preview-engine"))]

use crate::Context;

use crate::common::{self, Result};
use crate::language::{self, completion};
use crate::util;

#[cfg(target_arch = "wasm32")]
use crate::wasm_prelude::*;

use i_slint_compiler::object_tree::ElementRc;
use lsp_types::{TextEdit, Url, WorkspaceEdit};

pub fn notify_preview_about_text_edit(
    server_notifier: &crate::ServerNotifier,
    edit: &TextEdit,
    source_file: &i_slint_compiler::diagnostics::SourceFile,
) {
    let new_length = edit.new_text.len() as u32;
    let (start_offset, end_offset) = {
        let so =
            source_file.offset(edit.range.start.line as usize, edit.range.start.character as usize);
        let eo =
            source_file.offset(edit.range.end.line as usize, edit.range.end.character as usize);
        (std::cmp::min(so, eo) as u32, std::cmp::max(so, eo) as u32)
    };

    let Ok(url) = Url::from_file_path(source_file.path()) else {
        return;
    };
    server_notifier.send_message_to_preview(common::LspToPreviewMessage::AdjustSelection {
        url: common::VersionedUrl::new(url, source_file.version()),
        start_offset,
        end_offset,
        new_length,
    });
}

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

pub fn add_component(ctx: &Context, component: common::ComponentAddition) -> Result<WorkspaceEdit> {
    let document_url = component.insert_position.url();
    let dc = ctx.document_cache.borrow();
    let file = Url::to_file_path(document_url)
        .map_err(|_| "Failed to convert URL to file path".to_string())?;

    if &dc.document_version(document_url) != component.insert_position.version() {
        return Err("Document version mismatch.".into());
    }

    let doc = dc
        .documents
        .get_document(&file)
        .ok_or_else(|| "Document URL not found in cache".to_string())?;
    let mut edits = Vec::with_capacity(2);
    if let Some(edit) =
        completion::create_import_edit(doc, &component.component_type, &component.import_path)
    {
        if let Some(sf) = doc.node.as_ref().map(|n| &n.source_file) {
            notify_preview_about_text_edit(&ctx.server_notifier, &edit, sf);
        }
        edits.push(edit);
    }

    let source_file = doc.node.as_ref().unwrap().source_file.clone();

    let ip = util::map_position(&source_file, component.insert_position.offset().into());
    edits.push(TextEdit {
        range: lsp_types::Range::new(ip.clone(), ip),
        new_text: component.component_text,
    });

    common::create_workspace_edit_from_source_file(&source_file, edits)
        .ok_or("Could not create workspace edit".into())
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

pub fn remove_element(ctx: &Context, position: common::VersionedPosition) -> Result<WorkspaceEdit> {
    let element = element_at_source_code_position(&mut ctx.document_cache.borrow_mut(), &position)?;

    let e = element.borrow();
    let Some(node) = e.debug.get(0).map(|(n, _)| n) else {
        return Err("No node found".into());
    };

    let Some(range) = util::map_node(node) else {
        return Err("Could not map element node".into());
    };
    let edits = vec![TextEdit { range, new_text: String::new() }];

    common::create_workspace_edit_from_source_file(&node.source_file, edits)
        .ok_or("Could not create workspace edit".into())
}
