// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_compiler::source_path::SourcePath;
use std::collections::HashMap;
use std::path::PathBuf;

pub const LIBRARY_NAME: &str = "editor-controls";
pub const ENTRY_URL: &str = "slint-editor-controls:///src/editor.slint";

pub fn library_paths() -> HashMap<String, PathBuf> {
    HashMap::from([(LIBRARY_NAME.into(), ENTRY_URL.into())])
}

pub fn source(path: &SourcePath) -> Option<&'static str> {
    let url = path.to_url()?;
    if url.scheme() != "slint-editor-controls" {
        return None;
    }
    match url.path().strip_prefix("/src/")? {
        "editor.slint" => Some(include_str!("../controls/src/editor.slint")),
        "basic.slint" => Some(include_str!("../controls/src/basic.slint")),
        "headless.slint" => Some(include_str!("../controls/src/headless.slint")),
        "basic/button.slint" => Some(include_str!("../controls/src/basic/button.slint")),
        "basic/theme.slint" => Some(include_str!("../controls/src/basic/theme.slint")),
        "basic/slider.slint" => Some(include_str!("../controls/src/basic/slider.slint")),
        "basic/combo_box.slint" => Some(include_str!("../controls/src/basic/combo_box.slint")),
        "headless/button_base.slint" => {
            Some(include_str!("../controls/src/headless/button_base.slint"))
        }
        "headless/slider_base.slint" => {
            Some(include_str!("../controls/src/headless/slider_base.slint"))
        }
        "headless/linear_slider_surface.slint" => {
            Some(include_str!("../controls/src/headless/linear_slider_surface.slint"))
        }
        "headless/combo_box_base.slint" => {
            Some(include_str!("../controls/src/headless/combo_box_base.slint"))
        }
        "headless/combo_box_popup_base.slint" => {
            Some(include_str!("../controls/src/headless/combo_box_popup_base.slint"))
        }
        _ => None,
    }
}

pub fn components(
    document_cache: &mut i_slint_editor_preview::DocumentCache,
) -> Vec<i_slint_editor_preview::component_catalog::ComponentInformation> {
    use i_slint_editor_preview::{component_catalog, editing::PropertyChange};
    let url = lsp_types::Url::parse(ENTRY_URL).unwrap();
    if document_cache.get_document(&url).is_none() {
        let mut diagnostics = i_slint_compiler::diagnostics::BuildDiagnostics::default();
        i_slint_editor_preview::util::poll_once(document_cache.load_url(
            &url,
            None,
            source(&SourcePath::from_url(&url)).unwrap().into(),
            &mut diagnostics,
        ));
    }
    let mut components = Vec::new();
    component_catalog::all_exported_components(
        document_cache,
        &mut |component| {
            component.defined_at.as_ref().is_some_and(|position| {
                position.url().scheme() == "slint-editor-controls"
                    && position.url().path() == "/@editor-controls"
            })
        },
        &mut components,
    );
    for component in &mut components {
        component.default_properties = match component.name.as_str() {
            "ControlButton" => vec![PropertyChange::new("text", "\"Button\"".into())],
            "ControlComboBox" => {
                vec![PropertyChange::new("model", "[\"First\", \"Second\", \"Third\"]".into())]
            }
            "ControlSlider" => vec![PropertyChange::new("value", "42".into())],
            _ => Vec::new(),
        };
    }
    components
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;

    #[test]
    fn bundled_controls_compile_alongside_standard_widgets() {
        i_slint_backend_testing::init_no_event_loop();
        let mut compiler = slint_interpreter::Compiler::default();
        let config = compiler.compiler_configuration(i_slint_core::InternalToken);
        config.enable_experimental = true;
        config.library_paths = library_paths();
        config.open_import_callback = Some(Rc::new(|path| {
            Box::pin(async move { source(&path).map(|source| Ok(source.into())) })
        }));
        let result = spin_on::spin_on(
            compiler.build_from_source(
                r#"
            import { Button } from "std-widgets.slint";
            import { ControlButton, ControlSlider, ControlComboBox } from "@editor-controls";
            export component Probe inherits Window {
                HorizontalLayout {
                    Button { text: "Standard"; }
                    button := ControlButton { text: "Custom"; }
                    slider := ControlSlider { value: 42; }
                    combo := ControlComboBox { model: ["First", "Second"]; }
                }
                out property <bool> test: button.text == "Custom" && slider.value == 42
                    && combo.current-value == "First";
            }
        "#
                .into(),
                "/controls-probe.slint".into(),
            ),
        );
        let definition = result
            .component("Probe")
            .unwrap_or_else(|| panic!("{:?}", result.diagnostics().collect::<Vec<_>>()));
        let instance = definition.create().unwrap();
        assert_eq!(instance.get_property("test").unwrap(), slint_interpreter::Value::Bool(true));
    }
}
