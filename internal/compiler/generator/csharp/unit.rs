// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::CompilerConfiguration;
use crate::EmbedResourcesKind;
use crate::diagnostics::BuildDiagnostics;
use crate::source_path::SourcePath;
use base64::Engine;
use std::collections::BTreeMap;
use std::rc::Rc;

/// Compiles the root file at `path` like the interpreter does, for [`crate::llr::bundle::from_bytes()`],
/// with the images it shows as `data:` URLs and the fonts it imports, so that it runs on another machine.
/// Returns no bytes if the compilation fails.
pub async fn compile(
    path: &SourcePath,
    source_code: String,
    mut config: CompilerConfiguration,
) -> (BuildDiagnostics, Option<Vec<u8>>) {
    if std::env::var_os("SLINT_INLINING").is_none() {
        config.inline_all_elements = false;
    }
    config.embed_resources = EmbedResourcesKind::Nothing;
    config.resource_url_mapper = Some(Rc::new(|url| {
        let mapped = data_url(url);
        Box::pin(async move { mapped })
    }));
    let (mut diagnostics, loader) =
        crate::load_root_file(path, source_code, Default::default(), config).await;
    let Some(document) = loader.get_document(path).filter(|_| !diagnostics.has_errors()) else {
        return (diagnostics, None);
    };
    let unit =
        crate::llr::lower_to_item_tree::lower_to_item_tree(document, &loader.compiler_config);
    let resources = crate::embedded_resources::pre_rendered_resources(document);

    let mut fonts = BTreeMap::new();
    for doc in loader.all_documents() {
        for (font, token) in &doc.custom_fonts {
            // The interpreter has the fonts of the styles built in.
            let Some(native) = font.as_native_path() else { continue };
            match std::fs::read(native) {
                Ok(data) => {
                    fonts.insert(font.to_string(), data);
                }
                Err(e) => diagnostics.push_error(format!("Cannot load the font: {e}"), token),
            }
        }
    }
    if diagnostics.has_errors() {
        return (diagnostics, None);
    }
    match crate::llr::bundle::to_bytes(&unit, &resources, fonts.into_iter().collect()) {
        Ok(bytes) => (diagnostics, Some(bytes)),
        Err(e) => {
            diagnostics
                .push_error_with_span(format!("Cannot write the unit: {e}"), Default::default());
            (diagnostics, None)
        }
    }
}

fn data_url(url: &url::Url) -> Option<url::Url> {
    let path = url.to_file_path().ok()?;
    let data = std::fs::read(&path).ok()?;
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    let subtype = match extension.as_str() {
        "jpg" => "jpeg",
        "svg" | "svgz" => "svg+xml",
        other => other,
    };
    let encoded = base64::engine::general_purpose::STANDARD.encode(&data);
    url::Url::parse(&format!("data:image/{subtype};base64,{encoded}")).ok()
}
