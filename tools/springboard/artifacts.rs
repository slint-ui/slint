// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Release artifact locations used by managed simulator targets.

use anyhow::{Result, bail};

pub const ARTIFACT_BASE_URL_ENVIRONMENT_VARIABLE: &str = "SLINT_SPRINGBOARD_ARTIFACT_BASE_URL";
pub const ARTIFACT_CHANNEL_ENVIRONMENT_VARIABLE: &str = "SLINT_SPRINGBOARD_ARTIFACT_CHANNEL";
pub const DEFAULT_ARTIFACT_BASE_URL: &str = "https://github.com/slint-ui/slint/releases/download";
pub const DEFAULT_ARTIFACT_CHANNEL: &str = env!("SLINT_SPRINGBOARD_DEFAULT_ARTIFACT_CHANNEL");
pub const MOBILE_VIEWER_ARTIFACT_MANIFEST_FILE: &str = "slint-viewer-mobile-artifacts.json";

/// Release location used to resolve a viewer manifest and its artifacts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactSource {
    base_url: String,
    channel: String,
}

impl ArtifactSource {
    /// Resolve the configured release source, including test and private mirror overrides.
    pub fn from_environment() -> Result<Self> {
        let base_url = std::env::var(ARTIFACT_BASE_URL_ENVIRONMENT_VARIABLE)
            .unwrap_or_else(|_| DEFAULT_ARTIFACT_BASE_URL.into());
        let channel = std::env::var(ARTIFACT_CHANNEL_ENVIRONMENT_VARIABLE)
            .unwrap_or_else(|_| DEFAULT_ARTIFACT_CHANNEL.into());
        Self::new(base_url, channel)
    }

    /// Create an explicit artifact source.
    pub fn new(base_url: impl Into<String>, channel: impl Into<String>) -> Result<Self> {
        let base_url = base_url.into().trim_end_matches('/').to_string();
        let channel = channel.into().trim_matches('/').to_string();
        if base_url.is_empty() {
            bail!("The Springboard artifact base URL is empty");
        }
        if channel.is_empty() {
            bail!("The Springboard artifact channel is empty");
        }
        Ok(Self { base_url, channel })
    }

    /// URL of the versioned artifact manifest.
    pub fn manifest_url(&self) -> String {
        format!("{}/{}/{}", self.base_url, self.channel, MOBILE_VIEWER_ARTIFACT_MANIFEST_FILE)
    }

    /// URL of one artifact named by the manifest.
    pub fn artifact_url(&self, file_name: &str) -> Result<String> {
        if file_name.is_empty() || file_name.contains('/') || file_name.contains('\\') {
            bail!("The viewer artifact file name is invalid");
        }
        Ok(format!("{}/{}/{}", self.base_url, self.channel, file_name))
    }

    /// Release tag or moving channel selected for this source.
    pub fn channel(&self) -> &str {
        &self.channel
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_and_mirror_urls_use_the_same_channel_layout() {
        let source = ArtifactSource::new("https://mirror.invalid/releases/", "/nightly/").unwrap();

        assert_eq!(
            source.manifest_url(),
            "https://mirror.invalid/releases/nightly/slint-viewer-mobile-artifacts.json"
        );
        assert_eq!(
            source.artifact_url("slint-viewer.apk").unwrap(),
            "https://mirror.invalid/releases/nightly/slint-viewer.apk"
        );
    }

    #[test]
    fn artifact_names_cannot_escape_the_release_directory() {
        let source = ArtifactSource::new(DEFAULT_ARTIFACT_BASE_URL, "v1.18.0").unwrap();

        assert!(source.artifact_url("../viewer.apk").is_err());
        assert!(source.artifact_url("nested/viewer.apk").is_err());
    }

    #[test]
    fn compiled_default_artifact_channel_is_well_formed() {
        assert!(DEFAULT_ARTIFACT_CHANNEL == "nightly" || DEFAULT_ARTIFACT_CHANNEL.starts_with('v'));
    }
}
