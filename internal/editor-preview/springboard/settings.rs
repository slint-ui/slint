// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::NamedEndpointIdentity;

/// File the springboard stores its last selection in. The springboard itself
/// has no filesystem access (it must also work when it is not hosted by the
/// editor process), so the setting travels over the preview protocol and the
/// editor persists it, exactly like the preview UI's own settings.
pub(super) const SPRINGBOARD_SETTINGS_FILE: &str = "springboard-settings.json";

const SPRINGBOARD_SETTINGS_VERSION: u32 = 1;

/// Persisted springboard state: the endpoint the user last connected to, so it
/// can be preselected once it shows up on the network again.
#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
#[serde(deny_unknown_fields)]
pub(super) struct SpringboardSettings {
    version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) last_selected: Option<NamedEndpointIdentity>,
}

impl SpringboardSettings {
    pub(super) fn new(last_selected: Option<NamedEndpointIdentity>) -> Self {
        Self { version: SPRINGBOARD_SETTINGS_VERSION, last_selected }
    }

    pub(super) fn serialize(&self) -> String {
        let mut json =
            serde_json::to_string_pretty(self).expect("serializing springboard settings");
        json.push('\n');
        json
    }

    /// Parse the blob the editor stored, or `None` when it is malformed or was
    /// written by an unsupported version.
    pub(super) fn deserialize(contents: &str) -> Option<Self> {
        match serde_json::from_str::<Self>(contents) {
            Ok(settings) if settings.version == SPRINGBOARD_SETTINGS_VERSION => Some(settings),
            Ok(settings) => {
                tracing::warn!(
                    "Ignoring springboard settings of unsupported version {}",
                    settings.version
                );
                None
            }
            Err(error) => {
                tracing::warn!("Ignoring malformed springboard settings: {error}");
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_of_another_version_are_rejected() {
        let contents = format!(r#"{{"version":{}}}"#, SPRINGBOARD_SETTINGS_VERSION + 1);
        assert!(SpringboardSettings::deserialize(&contents).is_none());
        // A missing version is malformed, not version 0.
        assert!(SpringboardSettings::deserialize("{}").is_none());
    }
}
