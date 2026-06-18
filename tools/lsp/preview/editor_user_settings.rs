// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Editor-embedded-preview user settings.
//!
//! Analogous to [`super::user_settings`] for the native preview window, but for
//! the editor-embedded preview (`AppWindow::Editor`). The LSP stores this as an
//! opaque blob under [`EDITOR_SETTINGS_FILE`] and never interprets it.

/// Name of the file the editor preview stores its settings in.
pub const EDITOR_SETTINGS_FILE: &str = "editor-user-settings.json";

/// Editor-embedded-preview UI state persisted for the local user.
///
/// No version field — serde is permissive: unknown keys are ignored and missing
/// keys fall back to [`Default`]. Add fields freely; old files remain valid.
#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct EditorUserSettings {
    pub always_on_top: bool,
}

impl EditorUserSettings {
    /// Serialize to the string stored on disk by the LSP.
    pub fn serialize(&self) -> String {
        let mut s = serde_json::to_string_pretty(self).expect("serializing editor settings");
        s.push('\n');
        s
    }

    /// Parse settings received from the LSP, returning `None` (and warning) if
    /// the stored blob is not valid JSON.
    pub fn deserialize(contents: &str) -> Option<Self> {
        match serde_json::from_str(contents) {
            Ok(v) => Some(v),
            Err(e) => {
                tracing::warn!("Ignoring malformed editor user settings: {e}");
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize_round_trips_through_deserialize() {
        let settings = EditorUserSettings { always_on_top: true };
        assert_eq!(EditorUserSettings::deserialize(&settings.serialize()), Some(settings));
    }

    #[test]
    fn deserialize_ignores_unknown_keys() {
        let json = r#"{"always_on_top": true, "unknown_future_key": 42}"#;
        assert_eq!(
            EditorUserSettings::deserialize(json),
            Some(EditorUserSettings { always_on_top: true })
        );
    }

    #[test]
    fn deserialize_uses_defaults_for_missing_keys() {
        assert_eq!(EditorUserSettings::deserialize("{}"), Some(EditorUserSettings::default()));
    }

    #[test]
    fn deserialize_rejects_malformed_json() {
        assert_eq!(EditorUserSettings::deserialize("{ not json"), None);
    }
}
