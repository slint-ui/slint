// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::EndpointIdentity;

/// File the springboard stores its saved endpoints and last selection in. The springboard itself
/// has no filesystem access (it must also work when it is not hosted by the
/// editor process), so the setting travels over the preview protocol and the
/// editor persists it, exactly like the preview UI's own settings.
pub(super) const SPRINGBOARD_SETTINGS_FILE: &str = "springboard-settings.json";

const SPRINGBOARD_SETTINGS_VERSION: u32 = 2;

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub(super) struct SpringboardSettings {
    version: u32,
    /// Without Local, which is always available.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) saved: Vec<EndpointIdentity>,
    #[serde(default)]
    pub(super) selected: EndpointIdentity,
}

impl SpringboardSettings {
    pub(super) fn new(saved: Vec<EndpointIdentity>, selected: EndpointIdentity) -> Self {
        Self { version: SPRINGBOARD_SETTINGS_VERSION, saved, selected }
    }

    pub(super) fn serialize(&self) -> String {
        let mut json =
            serde_json::to_string_pretty(self).expect("serializing springboard settings");
        json.push('\n');
        json
    }

    /// Parse the blob the editor stored, or `None` when it is malformed.
    ///
    /// A blob written by another version is used as far as it parses, so a
    /// setting that version does not know about is the only thing lost.
    pub(super) fn deserialize(contents: &str) -> Option<Self> {
        match serde_json::from_str::<Self>(contents) {
            Ok(settings) => {
                if settings.version != SPRINGBOARD_SETTINGS_VERSION {
                    tracing::info!(
                        "Read springboard settings of version {}; this build writes version {}",
                        settings.version,
                        SPRINGBOARD_SETTINGS_VERSION
                    );
                }
                Some(settings)
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
    fn settings_of_another_version_are_still_used() {
        let contents = format!(
            r#"{{"version":{},"selected":{{"remote":{{"host":"fw12"}}}}}}"#,
            SPRINGBOARD_SETTINGS_VERSION + 1
        );
        assert_eq!(
            SpringboardSettings::deserialize(&contents).unwrap().selected,
            EndpointIdentity::Remote { host: "fw12".into() }
        );
    }

    #[test]
    fn unknown_fields_are_ignored() {
        let contents = format!(
            r#"{{"version":{SPRINGBOARD_SETTINGS_VERSION},"selected":"local","added_later":42}}"#
        );
        assert_eq!(
            SpringboardSettings::deserialize(&contents).unwrap().selected,
            EndpointIdentity::Local
        );
    }

    #[test]
    fn a_missing_selection_is_local() {
        let contents = format!(r#"{{"version":{SPRINGBOARD_SETTINGS_VERSION}}}"#);
        assert_eq!(
            SpringboardSettings::deserialize(&contents).unwrap().selected,
            EndpointIdentity::Local
        );
    }

    #[test]
    fn saved_endpoints_round_trip() {
        let saved = vec![
            EndpointIdentity::Remote { host: "Pixel 8".into() },
            EndpointIdentity::Manual { host: "192.168.1.5".into(), port: 9000 },
        ];
        let selected = saved[1].clone();
        let settings = SpringboardSettings::deserialize(
            &SpringboardSettings::new(saved.clone(), selected.clone()).serialize(),
        )
        .unwrap();
        assert_eq!(settings.saved, saved);
        assert_eq!(settings.selected, selected);
    }

    #[test]
    fn malformed_settings_are_rejected() {
        assert!(SpringboardSettings::deserialize("not json").is_none());
        // A missing version is malformed, not version 0.
        assert!(SpringboardSettings::deserialize("{}").is_none());
    }
}
