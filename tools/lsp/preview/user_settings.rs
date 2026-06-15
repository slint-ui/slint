// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Preview-owned user settings.
//!
//! These settings belong to the (native) preview UI, not to the protocol or
//! the LSP. The preview serializes them to a string and hands them to the LSP
//! for storage under [`PREVIEW_SETTINGS_FILE`]; the LSP persists the blob
//! verbatim and never interprets it (see `crate::settings_store`).

/// Name of the file the preview stores its settings in. The LSP treats this as
/// an opaque key.
pub const PREVIEW_SETTINGS_FILE: &str = "preview-user-settings.json";

/// Preview UI state persisted for the local user.
///
/// This intentionally excludes editor/compile configuration (`PreviewConfig`)
/// and geometry. Version 1 is the initial frozen shape.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "PreviewUserSettingsSerde", rename_all = "snake_case")]
pub struct PreviewUserSettings {
    pub version: u32,
    pub always_on_top: bool,
    pub show_library: bool,
    pub show_properties: bool,
    pub show_outline: bool,
    pub show_simulation_data: bool,
    pub show_console: bool,
}

impl PreviewUserSettings {
    pub const CURRENT_VERSION: u32 = 1;

    /// Serialize to the string stored on disk by the LSP.
    pub fn serialize(&self) -> String {
        // Serializing this plain struct cannot fail.
        let mut json = serde_json::to_string_pretty(self).expect("serializing settings");
        json.push('\n');
        json
    }

    /// Parse settings received from the LSP, returning `None` (and warning) if
    /// the stored blob is malformed or of an unsupported version.
    pub fn deserialize(contents: &str) -> Option<Self> {
        match serde_json::from_str(contents) {
            Ok(settings) => Some(settings),
            Err(err) => {
                tracing::warn!("Ignoring malformed preview user settings: {err}");
                None
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
struct PreviewUserSettingsSerde {
    version: u32,
    #[serde(default)]
    always_on_top: bool,
    #[serde(default)]
    show_library: bool,
    #[serde(default)]
    show_properties: bool,
    #[serde(default)]
    show_outline: bool,
    #[serde(default)]
    show_simulation_data: bool,
    #[serde(default)]
    show_console: bool,
}

impl TryFrom<PreviewUserSettingsSerde> for PreviewUserSettings {
    type Error = String;

    fn try_from(value: PreviewUserSettingsSerde) -> Result<Self, Self::Error> {
        if value.version != Self::CURRENT_VERSION {
            return Err(format!(
                "unsupported PreviewUserSettings version {}, expected {}",
                value.version,
                Self::CURRENT_VERSION
            ));
        }

        Ok(Self {
            version: value.version,
            always_on_top: value.always_on_top,
            show_library: value.show_library,
            show_properties: value.show_properties,
            show_outline: value.show_outline,
            show_simulation_data: value.show_simulation_data,
            show_console: value.show_console,
        })
    }
}

impl Default for PreviewUserSettings {
    fn default() -> Self {
        Self {
            version: Self::CURRENT_VERSION,
            always_on_top: false,
            show_library: false,
            show_properties: false,
            show_outline: false,
            show_simulation_data: false,
            show_console: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn preview_user_settings_serializes_with_version_and_view_toggles() {
        assert_eq!(
            serde_json::to_value(PreviewUserSettings::default()).unwrap(),
            json!({
                "version": 1,
                "always_on_top": false,
                "show_library": false,
                "show_properties": false,
                "show_outline": false,
                "show_simulation_data": false,
                "show_console": false,
            })
        );
    }

    #[test]
    fn serialize_round_trips_through_deserialize() {
        let settings = PreviewUserSettings {
            version: PreviewUserSettings::CURRENT_VERSION,
            always_on_top: true,
            show_library: false,
            show_properties: true,
            show_outline: false,
            show_simulation_data: true,
            show_console: false,
        };
        assert_eq!(PreviewUserSettings::deserialize(&settings.serialize()), Some(settings));
    }

    #[test]
    fn deserialize_preserves_older_subset_fields() {
        assert_eq!(
            PreviewUserSettings::deserialize("{ \"version\": 1, \"always_on_top\": true }"),
            Some(PreviewUserSettings { always_on_top: true, ..PreviewUserSettings::default() })
        );
    }

    #[test]
    fn deserialize_rejects_malformed_or_mismatched_version() {
        assert_eq!(PreviewUserSettings::deserialize("{ this is not json"), None);
        assert_eq!(PreviewUserSettings::deserialize("{ \"show_console\": true }"), None);
        assert_eq!(
            PreviewUserSettings::deserialize(
                "{ \"version\": 2, \"always_on_top\": false, \"show_library\": false, \
                 \"show_properties\": false, \"show_outline\": false, \
                 \"show_simulation_data\": false, \"show_console\": false }"
            ),
            None
        );
    }
}
