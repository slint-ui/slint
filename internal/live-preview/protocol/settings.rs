// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/// LSP-owned preview UI state persisted for the local user.
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
    fn preview_user_settings_requires_version() {
        let err = serde_json::from_value::<PreviewUserSettings>(json!({ "show_console": true }))
            .unwrap_err()
            .to_string();
        assert!(err.contains("missing field `version`"));
    }

    #[test]
    fn preview_user_settings_rejects_mismatched_version() {
        let err = serde_json::from_value::<PreviewUserSettings>(json!({
            "version": 2,
            "always_on_top": false,
            "show_library": false,
            "show_properties": false,
            "show_outline": false,
            "show_simulation_data": false,
            "show_console": false,
        }))
        .unwrap_err()
        .to_string();
        assert!(err.contains("unsupported PreviewUserSettings version 2, expected 1"));
    }
}
