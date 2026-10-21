// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::EndpointIdentity;

/// File the springboard stores its last selection in. The springboard itself
/// has no filesystem access (it must also work when it is not hosted by the
/// editor process), so the setting travels over the preview protocol and the
/// editor persists it, exactly like the preview UI's own settings.
pub(super) const SPRINGBOARD_SETTINGS_FILE: &str = "springboard-settings.json";

const SPRINGBOARD_SETTINGS_VERSION: u32 = 1;

/// The endpoint a session last connected to, mirroring [`EndpointIdentity`] but
/// additionally remembering the display name it had at the time.
///
/// The name cannot be recovered from the identity alone: it comes from the
/// discovery report (the mDNS fullname without the service type, or the host as
/// a fallback, see `remote::normalize`). At startup there is no discovery report
/// yet, so without the stored name the springboard could not say which endpoint
/// it is waiting for.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum NamedEndpointIdentity {
    Local,
    Remote { fullname: String, name: String },
}

impl From<NamedEndpointIdentity> for EndpointIdentity {
    fn from(named: NamedEndpointIdentity) -> Self {
        match named {
            NamedEndpointIdentity::Local => Self::Local,
            NamedEndpointIdentity::Remote { fullname, .. } => Self::Remote(fullname),
        }
    }
}

/// Persisted springboard state: the endpoint the user last connected to, so it
/// can be preselected once it shows up on the network again.
#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub(super) struct SpringboardSettings {
    version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) last_selected: Option<NamedEndpointIdentity>,
    #[serde(
        default,
        deserialize_with = "deserialize_devices",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub(super) devices: Vec<SavedDevice>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) struct SavedDevice {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub addresses: Vec<String>,
    #[serde(default)]
    pub port: u16,
}

fn deserialize_devices<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<SavedDevice>, D::Error> {
    use serde::Deserialize;
    let entries = Vec::<serde_json::Value>::deserialize(deserializer)?;
    Ok(entries
        .into_iter()
        .filter_map(|entry| match serde_json::from_value::<SavedDevice>(entry) {
            Ok(mut device) if !device.id.is_empty() => {
                if device.name.is_empty() {
                    device.name = device.id.clone();
                }
                Some(device)
            }
            _ => {
                tracing::warn!("Ignoring malformed saved device");
                None
            }
        })
        .collect())
}

impl SpringboardSettings {
    pub(super) fn new(last_selected: Option<NamedEndpointIdentity>) -> Self {
        Self { version: SPRINGBOARD_SETTINGS_VERSION, last_selected, devices: Vec::new() }
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
        if contents.is_empty() {
            return Some(Self::new(None));
        }
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
    fn incomplete_devices_do_not_discard_other_settings() {
        let settings = SpringboardSettings::deserialize(r#"{
            "version": 1,
            "last_selected": {"remote": {"fullname": "phone", "name": "Phone"}},
            "devices": [
                {"id": "phone", "name": "Phone", "addresses": ["127.0.0.1"], "port": 9000, "manual": false},
                {"id": "tablet"},
                {"name": "No identity"},
                {"id": "broken", "port": "invalid"}
            ]
        }"#).unwrap();
        assert_eq!(settings.devices.len(), 2);
        assert_eq!(settings.devices[0].name, "Phone");
        assert_eq!(settings.devices[1].name, "tablet");
        let roundtrip = SpringboardSettings::deserialize(&settings.serialize()).unwrap();
        assert_eq!(roundtrip.devices, settings.devices);
        assert_eq!(roundtrip.last_selected, settings.last_selected);
        assert!(!settings.serialize().contains("manual"));
    }

    #[test]
    fn absent_settings_use_defaults() {
        let settings = SpringboardSettings::deserialize("").unwrap();
        assert!(settings.devices.is_empty());
        assert!(settings.last_selected.is_none());
    }

    #[test]
    fn settings_of_another_version_are_still_used() {
        let contents = format!(
            r#"{{"version":{},"last_selected":"local"}}"#,
            SPRINGBOARD_SETTINGS_VERSION + 1
        );
        assert_eq!(
            SpringboardSettings::deserialize(&contents).unwrap().last_selected,
            Some(NamedEndpointIdentity::Local)
        );
    }

    #[test]
    fn unknown_fields_are_ignored() {
        let contents = format!(
            r#"{{"version":{SPRINGBOARD_SETTINGS_VERSION},"last_selected":"local","added_later":42}}"#
        );
        assert_eq!(
            SpringboardSettings::deserialize(&contents).unwrap().last_selected,
            Some(NamedEndpointIdentity::Local)
        );
    }

    #[test]
    fn malformed_settings_are_rejected() {
        assert!(SpringboardSettings::deserialize("not json").is_none());
        // A missing version is malformed, not version 0.
        assert!(SpringboardSettings::deserialize("{}").is_none());
    }
}
