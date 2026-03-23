use lsp_types::Url;

use crate::SourceFileVersion;

/// A versioned file
#[derive(Clone, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
pub struct VersionedUrl {
    /// The file url
    url: Url,
    // The file version
    version: SourceFileVersion,
}

impl VersionedUrl {
    pub fn new(url: Url, version: SourceFileVersion) -> Self {
        VersionedUrl { url, version }
    }

    pub fn url(&self) -> &Url {
        &self.url
    }

    pub fn version(&self) -> &SourceFileVersion {
        &self.version
    }
}

impl std::fmt::Debug for VersionedUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let version = self.version.map(|v| format!("v{v}")).unwrap_or_else(|| "none".to_string());
        write!(f, "{}@{}", self.url, version)
    }
}
