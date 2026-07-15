// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#[cfg(all(target_arch = "wasm32", feature = "preview-external"))]
mod wasm;
#[cfg(all(target_arch = "wasm32", feature = "preview-external"))]
pub use wasm::*;

#[cfg(all(not(target_arch = "wasm32"), feature = "preview-builtin"))]
pub mod native;
#[cfg(all(not(target_arch = "wasm32"), feature = "preview-builtin"))]
pub use native::*;

#[cfg(all(not(target_arch = "wasm32"), feature = "preview-remote"))]
pub mod remote;

use crate::preview;
use i_slint_live_preview::protocol::LspToPreviewMessage;

/// The persisted preview UI settings, an opaque JSON blob every host stores and
/// restores verbatim so new settings can be added without touching the hosts or
/// the protocol. Today it only carries panel visibility, e.g.
/// `{"panels":{"library":true,"properties":false,...}}`.
#[derive(Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
struct UiSettings {
    #[serde(default)]
    panels: PanelSettings,
}

#[derive(Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
struct PanelSettings {
    #[serde(default)]
    library: bool,
    #[serde(default)]
    properties: bool,
    #[serde(default)]
    outline: bool,
    #[serde(default)]
    data: bool,
    #[serde(default)]
    console: bool,
}

/// Serialize the current panel visibility into the opaque settings blob the
/// host persists. Callers must read the getters (and drop any `PREVIEW_STATE`
/// borrow) before calling this, so we take plain values here.
#[allow(dead_code)]
pub fn serialize_ui_settings(
    library: bool,
    properties: bool,
    outline: bool,
    data: bool,
    console: bool,
) -> String {
    let settings =
        UiSettings { panels: PanelSettings { library, properties, outline, data, console } };
    serde_json::to_string(&settings).unwrap_or_default()
}

/// Apply a settings blob the host restored from an earlier session onto the UI.
///
/// Setting the panel properties fires `panels-layout-changed`, whose handler
/// persists the same values straight back, which is harmless. The caller must
/// have dropped any `PREVIEW_STATE` borrow before calling this: that handler
/// borrows `PREVIEW_STATE` again and would otherwise panic.
#[allow(dead_code)]
pub fn apply_ui_settings_to_api(api: &preview::ui::Api<'static>, blob: &str) {
    let Ok(settings) = serde_json::from_str::<UiSettings>(blob) else {
        return;
    };
    let panels = settings.panels;
    api.set_panel_library_open(panels.library);
    api.set_panel_properties_open(panels.properties);
    api.set_panel_outline_open(panels.outline);
    api.set_panel_data_open(panels.data);
    api.set_panel_console_open(panels.console);
}

pub fn lsp_to_preview(message: LspToPreviewMessage) {
    use LspToPreviewMessage as M;
    match message {
        M::InvalidateContents { url } => preview::invalidate_contents(&url),
        M::ForgetFile { url } => preview::delete_document(&url),
        M::SetContents { url, contents } => {
            if let Ok(contents) = String::from_utf8(contents) {
                preview::set_contents(&url, contents);
            }
        }
        M::SetConfiguration { config } => {
            preview::config_changed(config);
        }
        M::ShowPreview(pc) => {
            tracing::debug!(
                "Preview: ShowPreview for url={}, component={:?}",
                pc.url,
                pc.component
            );
            preview::load_preview(pc, preview::LoadBehavior::BringWindowToFront);
        }
        M::HighlightFromEditor { url, offset } => {
            preview::highlight(url, offset.into());
        }
        M::RemoteConnectionState { state, target, error } => {
            preview::set_remote_connection_state(state, target, error);
        }
        M::RestoreUiSettings { settings } => {
            // Upgrade and drop the PREVIEW_STATE borrow before applying: setting
            // the panel properties fires panels-layout-changed, whose handler
            // borrows PREVIEW_STATE again and would otherwise panic.
            let api =
                preview::PREVIEW_STATE.with_borrow(|preview_state| preview_state.api.upgrade());
            if let Some(api) = api {
                apply_ui_settings_to_api(&api, &settings);
            }
        }
        M::Quit => {
            tracing::debug!("Preview: Quit requested");
            #[cfg(not(target_arch = "wasm32"))]
            let _ = slint::quit_event_loop();
        }
        M::Ping => {
            // Keepalive for the remote-preview WebSocket; local previews never see it.
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn panels_of(blob: &str) -> PanelSettings {
        serde_json::from_str::<UiSettings>(blob).unwrap().panels
    }

    #[test]
    fn serialize_round_trips_panel_visibility() {
        let blob = serialize_ui_settings(true, false, true, false, true);
        let panels = panels_of(&blob);
        assert!(panels.library && panels.outline && panels.console);
        assert!(!panels.properties && !panels.data);
    }

    #[test]
    fn missing_fields_default_to_false() {
        // Hosts must tolerate blobs written before a field existed.
        let panels = panels_of(r#"{"panels":{"library":true}}"#);
        assert!(panels.library);
        assert!(!panels.properties && !panels.outline && !panels.data && !panels.console);
        assert!(panels_of("{}") == PanelSettings::default());
    }

    #[test]
    fn unknown_keys_are_ignored() {
        // New settings can be added without breaking hosts on the old schema.
        let panels = panels_of(r#"{"panels":{"library":true,"future_panel":true},"future":1}"#);
        assert!(panels.library);
    }

    #[test]
    fn malformed_blob_is_not_fatal() {
        // apply_ui_settings_to_api relies on this parse failing to a no-op.
        assert!(serde_json::from_str::<UiSettings>("not json").is_err());
    }
}
