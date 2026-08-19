// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use serde::{Deserialize, Serialize};

/// Current schema version for local mobile viewer artifact manifests.
pub const MOBILE_VIEWER_ARTIFACT_SCHEMA_VERSION: u32 = 1;

/// Environment variable that selects Springboard's local simulator artifact directory.
pub const SPRINGBOARD_ARTIFACT_DIR_ENVIRONMENT_VARIABLE: &str = "SLINT_SPRINGBOARD_ARTIFACT_DIR";

/// File name of the local mobile viewer artifact manifest.
pub const MOBILE_VIEWER_ARTIFACT_MANIFEST_FILE: &str = "slint-viewer-mobile-artifacts.json";

/// One local set of installable mobile viewer artifacts.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MobileViewerArtifactManifest {
    pub schema_version: u32,
    pub release_tag: String,
    pub slint_version: String,
    pub protocol: String,
    pub artifacts: Vec<MobileViewerArtifact>,
}

/// An installable viewer artifact built locally for Springboard.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MobileViewerArtifact {
    pub kind: MobileViewerArtifactKind,
    pub file_name: String,
    pub sha256: String,
    pub bundle_id: String,
    pub architectures: Vec<String>,
}

/// The platform package format of a mobile viewer artifact.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum MobileViewerArtifactKind {
    AndroidApk,
    IosSimulatorApp,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_manifests_round_trip_without_losing_install_metadata() {
        let manifest = MobileViewerArtifactManifest {
            schema_version: MOBILE_VIEWER_ARTIFACT_SCHEMA_VERSION,
            release_tag: "local".into(),
            slint_version: "1.18.0".into(),
            protocol: "slint-preview.1.18".into(),
            artifacts: vec![
                MobileViewerArtifact {
                    kind: MobileViewerArtifactKind::AndroidApk,
                    file_name: "slint-viewer.apk".into(),
                    sha256: "ab".repeat(32),
                    bundle_id: "dev.slint.viewer".into(),
                    architectures: vec!["arm64-v8a".into(), "x86_64".into()],
                },
                MobileViewerArtifact {
                    kind: MobileViewerArtifactKind::IosSimulatorApp,
                    file_name: "slint-viewer-ios-simulator.zip".into(),
                    sha256: "cd".repeat(32),
                    bundle_id: "dev.slint.slint-viewer".into(),
                    architectures: vec!["arm64".into(), "x86_64".into()],
                },
            ],
        };

        let json = serde_json::to_string(&manifest).unwrap();
        assert_eq!(serde_json::from_str::<MobileViewerArtifactManifest>(&json).unwrap(), manifest);
    }

    #[test]
    fn unknown_manifest_fields_are_rejected() {
        let json = r#"{
            "schema_version": 1,
            "release_tag": "local",
            "slint_version": "1.18.0",
            "protocol": "slint-preview.1.18",
            "artifacts": [],
            "download": "untrusted"
        }"#;

        assert!(serde_json::from_str::<MobileViewerArtifactManifest>(json).is_err());
    }
}
