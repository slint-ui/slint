// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Release artifact resolution and caching for managed simulator targets.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{Context as _, Result, anyhow, bail};
use futures_util::StreamExt as _;
use i_slint_live_preview::protocol::{PROTOCOL_SUBPROTOCOL, SLINT_VERSION};
use i_slint_springboard::{
    MOBILE_VIEWER_ARTIFACT_SCHEMA_VERSION, MobileViewerArtifact, MobileViewerArtifactKind,
    MobileViewerArtifactManifest,
};
use sha2::{Digest as _, Sha256};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

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

    fn cache_key(&self) -> String {
        hex::encode(Sha256::digest(format!("{}\n{}", self.base_url, self.channel)))
    }
}

/// Observable stages while Springboard prepares a managed viewer artifact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ArtifactCacheProgress {
    CheckingCache,
    FetchingManifest,
    Downloading { bytes_received: u64, total_bytes: Option<u64> },
    Validating,
    Ready { from_cache: bool },
    UsingPrevious { reason: String },
}

/// How an artifact was resolved.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ArtifactResolution {
    Downloaded,
    Cached,
    CachedAfterFailure { reason: String },
}

/// A validated viewer package ready for installation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CachedViewerArtifact {
    pub path: PathBuf,
    pub manifest: MobileViewerArtifactManifest,
    pub artifact: MobileViewerArtifact,
    pub resolution: ArtifactResolution,
}

/// Persistent cache for version-matched managed viewer packages.
pub struct ArtifactCache {
    root: PathBuf,
    source: ArtifactSource,
    client: reqwest::Client,
}

impl ArtifactCache {
    /// Create a cache in Springboard's operating-system cache directory.
    pub fn from_platform_cache(source: ArtifactSource) -> Result<Self> {
        let project_dirs = directories::ProjectDirs::from("dev", "Slint", "slint-springboard")
            .context("Cannot determine the Springboard cache directory")?;
        Self::new(project_dirs.cache_dir().join("viewer-artifacts"), source)
    }

    /// Create a cache rooted at an explicit directory.
    pub fn new(root: PathBuf, source: ArtifactSource) -> Result<Self> {
        // reqwest deliberately has no implicit crypto provider so Springboard's binary does not
        // pull in a second provider alongside the ring provider used elsewhere in the workspace.
        let _ = rustls::crypto::ring::default_provider().install_default();
        let client = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(15))
            .build()
            .context("Failed to initialize the viewer artifact downloader")?;
        Ok(Self { root, source, client })
    }

    /// Download or reuse an artifact that matches this Springboard build and target architecture.
    pub async fn prepare(
        &self,
        kind: MobileViewerArtifactKind,
        architecture: &str,
        mut progress: impl FnMut(ArtifactCacheProgress),
    ) -> Result<CachedViewerArtifact> {
        let architecture = normalize_architecture(architecture);
        progress(ArtifactCacheProgress::CheckingCache);
        let previous = self.find_cached(kind, architecture).await;
        if self.source.channel().starts_with('v')
            && let Some(previous) = previous.clone()
        {
            progress(ArtifactCacheProgress::Ready { from_cache: true });
            return Ok(previous);
        }

        progress(ArtifactCacheProgress::FetchingManifest);
        let fetched = self.fetch_manifest().await;
        let (manifest, manifest_bytes) = match fetched {
            Ok(fetched) => fetched,
            Err(error) => {
                return use_previous(previous, error, &mut progress);
            }
        };
        let artifact = match validate_manifest(&manifest, &self.source, kind, architecture) {
            Ok(artifact) => artifact.clone(),
            Err(error) => return use_previous(previous, error, &mut progress),
        };
        if let Err(error) = self.persist_manifest(&manifest_bytes).await {
            return use_previous(previous, error, &mut progress);
        }

        let artifact_path = self.artifact_path(&artifact);
        if validate_artifact_file(&artifact_path, &artifact.sha256).await.unwrap_or(false) {
            progress(ArtifactCacheProgress::Ready { from_cache: true });
            return Ok(CachedViewerArtifact {
                path: artifact_path,
                manifest,
                artifact,
                resolution: ArtifactResolution::Cached,
            });
        }

        let download_result = self.download_artifact(&artifact, &mut progress).await;
        match download_result {
            Ok(path) => {
                progress(ArtifactCacheProgress::Ready { from_cache: false });
                Ok(CachedViewerArtifact {
                    path,
                    manifest,
                    artifact,
                    resolution: ArtifactResolution::Downloaded,
                })
            }
            Err(error) => use_previous(previous, error, &mut progress),
        }
    }

    fn source_directory(&self) -> PathBuf {
        self.root.join(self.source.cache_key())
    }

    fn manifest_directory(&self) -> PathBuf {
        self.source_directory().join("manifests")
    }

    fn artifact_directory(&self) -> PathBuf {
        self.source_directory().join("artifacts")
    }

    fn artifact_path(&self, artifact: &MobileViewerArtifact) -> PathBuf {
        self.artifact_directory().join(format!(
            "{}-{}",
            artifact.sha256.to_ascii_lowercase(),
            artifact.file_name
        ))
    }

    async fn fetch_manifest(&self) -> Result<(MobileViewerArtifactManifest, Vec<u8>)> {
        const MAX_MANIFEST_SIZE: usize = 1024 * 1024;

        let response = self
            .client
            .get(self.source.manifest_url())
            .send()
            .await
            .context("Failed to download the viewer artifact manifest")?
            .error_for_status()
            .context("The viewer artifact manifest request failed")?;
        if response.content_length().is_some_and(|length| length > MAX_MANIFEST_SIZE as u64) {
            bail!("The viewer artifact manifest exceeds {MAX_MANIFEST_SIZE} bytes");
        }
        let mut bytes = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.context("Failed while reading the viewer artifact manifest")?;
            if bytes.len() + chunk.len() > MAX_MANIFEST_SIZE {
                bail!("The viewer artifact manifest exceeds {MAX_MANIFEST_SIZE} bytes");
            }
            bytes.extend_from_slice(&chunk);
        }
        let manifest =
            serde_json::from_slice(&bytes).context("The viewer artifact manifest is malformed")?;
        Ok((manifest, bytes))
    }

    async fn persist_manifest(&self, bytes: &[u8]) -> Result<()> {
        let directory = self.manifest_directory();
        tokio::fs::create_dir_all(&directory)
            .await
            .context("Failed to create the viewer manifest cache")?;
        let digest = hex::encode(Sha256::digest(bytes));
        persist_bytes_noclobber(&directory, &directory.join(format!("{digest}.json")), bytes).await
    }

    async fn download_artifact(
        &self,
        artifact: &MobileViewerArtifact,
        progress: &mut impl FnMut(ArtifactCacheProgress),
    ) -> Result<PathBuf> {
        let directory = self.artifact_directory();
        tokio::fs::create_dir_all(&directory)
            .await
            .context("Failed to create the viewer artifact cache")?;
        let response = self
            .client
            .get(self.source.artifact_url(&artifact.file_name)?)
            .send()
            .await
            .with_context(|| format!("Failed to download {}", artifact.file_name))?
            .error_for_status()
            .with_context(|| format!("The {} download request failed", artifact.file_name))?;
        let total_bytes = response.content_length();
        let temporary = tempfile::NamedTempFile::new_in(&directory)
            .context("Failed to create a temporary viewer artifact")?;
        let mut file = tokio::fs::File::from_std(
            temporary.reopen().context("Failed to open the temporary viewer artifact")?,
        );
        let mut digest = Sha256::new();
        let mut bytes_received = 0u64;
        let mut stream = response.bytes_stream();
        progress(ArtifactCacheProgress::Downloading { bytes_received, total_bytes });
        while let Some(chunk) = stream.next().await {
            let chunk = chunk
                .with_context(|| format!("Failed while downloading {}", artifact.file_name))?;
            file.write_all(&chunk)
                .await
                .with_context(|| format!("Failed to cache {}", artifact.file_name))?;
            digest.update(&chunk);
            bytes_received += chunk.len() as u64;
            progress(ArtifactCacheProgress::Downloading { bytes_received, total_bytes });
        }
        file.sync_all().await.context("Failed to flush the cached viewer artifact")?;
        drop(file);

        progress(ArtifactCacheProgress::Validating);
        let actual = hex::encode(digest.finalize());
        if !actual.eq_ignore_ascii_case(&artifact.sha256) {
            bail!("The {} checksum is {actual}, expected {}", artifact.file_name, artifact.sha256);
        }

        let path = self.artifact_path(artifact);
        if tokio::fs::try_exists(&path).await?
            && !validate_artifact_file(&path, &artifact.sha256).await?
        {
            preserve_invalid_cache_file(&path).await?;
        }
        persist_temporary_noclobber(temporary, &path)?;
        if !validate_artifact_file(&path, &artifact.sha256).await? {
            bail!("The atomically cached {} failed checksum validation", artifact.file_name);
        }
        Ok(path)
    }

    async fn find_cached(
        &self,
        kind: MobileViewerArtifactKind,
        architecture: &str,
    ) -> Option<CachedViewerArtifact> {
        let mut directory = tokio::fs::read_dir(self.manifest_directory()).await.ok()?;
        let mut candidates = Vec::new();
        while let Ok(Some(entry)) = directory.next_entry().await {
            if entry.path().extension().and_then(|extension| extension.to_str()) != Some("json") {
                continue;
            }
            let modified = entry
                .metadata()
                .await
                .ok()
                .and_then(|metadata| metadata.modified().ok())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            candidates.push((modified, entry.path()));
        }
        candidates.sort_by_key(|(modified, _)| std::cmp::Reverse(*modified));

        for (_, path) in candidates {
            let Ok(bytes) = tokio::fs::read(path).await else { continue };
            let Ok(manifest) = serde_json::from_slice::<MobileViewerArtifactManifest>(&bytes)
            else {
                continue;
            };
            let Ok(artifact) =
                validate_manifest(&manifest, &self.source, kind, architecture).cloned()
            else {
                continue;
            };
            let artifact_path = self.artifact_path(&artifact);
            if validate_artifact_file(&artifact_path, &artifact.sha256).await.unwrap_or(false) {
                return Some(CachedViewerArtifact {
                    path: artifact_path,
                    manifest,
                    artifact,
                    resolution: ArtifactResolution::Cached,
                });
            }
        }
        None
    }
}

fn validate_manifest<'a>(
    manifest: &'a MobileViewerArtifactManifest,
    source: &ArtifactSource,
    kind: MobileViewerArtifactKind,
    architecture: &str,
) -> Result<&'a MobileViewerArtifact> {
    if manifest.schema_version != MOBILE_VIEWER_ARTIFACT_SCHEMA_VERSION {
        bail!(
            "Viewer artifact manifest schema {} is unsupported; expected {}",
            manifest.schema_version,
            MOBILE_VIEWER_ARTIFACT_SCHEMA_VERSION
        );
    }
    if manifest.release_tag != source.channel() {
        bail!(
            "Viewer artifact manifest is for {}, expected {}",
            manifest.release_tag,
            source.channel()
        );
    }
    if manifest.slint_version != SLINT_VERSION {
        bail!(
            "Viewer artifact manifest contains Slint {}, expected {SLINT_VERSION}",
            manifest.slint_version
        );
    }
    if manifest.protocol != PROTOCOL_SUBPROTOCOL {
        bail!(
            "Viewer artifact manifest uses protocol {}, expected {PROTOCOL_SUBPROTOCOL}",
            manifest.protocol
        );
    }
    let artifact = manifest
        .artifacts
        .iter()
        .find(|artifact| artifact.kind == kind)
        .with_context(|| format!("The viewer artifact manifest does not contain {kind:?}"))?;
    if artifact.file_name.is_empty()
        || artifact.file_name.contains('/')
        || artifact.file_name.contains('\\')
    {
        bail!("The viewer artifact file name is invalid");
    }
    if artifact.bundle_id.trim().is_empty() {
        bail!("The viewer artifact bundle ID is empty");
    }
    if artifact.sha256.len() != 64 || !artifact.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        bail!("The viewer artifact SHA-256 checksum is invalid");
    }
    if !artifact.architectures.iter().any(|candidate| candidate == architecture) {
        bail!("The viewer artifact does not support the {architecture} architecture");
    }
    Ok(artifact)
}

fn normalize_architecture(architecture: &str) -> &str {
    match architecture {
        "aarch64" => "arm64",
        architecture => architecture,
    }
}

fn use_previous(
    previous: Option<CachedViewerArtifact>,
    error: anyhow::Error,
    progress: &mut impl FnMut(ArtifactCacheProgress),
) -> Result<CachedViewerArtifact> {
    let reason = error.to_string();
    let Some(mut previous) = previous else { return Err(error) };
    progress(ArtifactCacheProgress::UsingPrevious { reason: reason.clone() });
    progress(ArtifactCacheProgress::Ready { from_cache: true });
    previous.resolution = ArtifactResolution::CachedAfterFailure { reason };
    Ok(previous)
}

async fn validate_artifact_file(path: &Path, expected: &str) -> Result<bool> {
    let mut file = match tokio::fs::File::open(path).await {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    let mut digest = Sha256::new();
    let mut buffer = vec![0; 64 * 1024];
    loop {
        let count = file.read(&mut buffer).await?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(hex::encode(digest.finalize()).eq_ignore_ascii_case(expected))
}

async fn persist_bytes_noclobber(directory: &Path, path: &Path, bytes: &[u8]) -> Result<()> {
    if tokio::fs::try_exists(path).await? {
        if tokio::fs::read(path).await? == bytes {
            return Ok(());
        }
        preserve_invalid_cache_file(path).await?;
    }
    let temporary = tempfile::NamedTempFile::new_in(directory)?;
    let mut file = tokio::fs::File::from_std(temporary.reopen()?);
    file.write_all(bytes).await?;
    file.sync_all().await?;
    drop(file);
    persist_temporary_noclobber(temporary, path)
}

async fn preserve_invalid_cache_file(path: &Path) -> Result<()> {
    let suffix =
        SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default().as_nanos();
    let file_name = path.file_name().and_then(|name| name.to_str()).unwrap_or("artifact");
    let invalid = path.with_file_name(format!("{file_name}.invalid-{suffix}"));
    tokio::fs::rename(path, invalid)
        .await
        .with_context(|| format!("Failed to preserve the corrupt cache file {}", path.display()))
}

fn persist_temporary_noclobber(temporary: tempfile::NamedTempFile, path: &Path) -> Result<()> {
    match temporary.persist_noclobber(path) {
        Ok(_) => Ok(()),
        Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        Err(error) => Err(anyhow!(error.error)).with_context(|| {
            format!("Failed to atomically cache the viewer artifact at {}", path.display())
        }),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex};

    use tokio::net::TcpListener;

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

    #[tokio::test]
    async fn matching_artifacts_are_downloaded_and_reused_offline() {
        let viewer = b"universal simulator viewer";
        let server = TestServer::start().await;
        let source = ArtifactSource::new(server.base_url(), "test").unwrap();
        server.set_release(viewer_manifest("viewer.zip", viewer), "viewer.zip", viewer);
        let cache_directory = tempfile::tempdir().unwrap();
        let cache = ArtifactCache::new(cache_directory.path().into(), source).unwrap();
        let mut progress = Vec::new();

        let downloaded = cache
            .prepare(MobileViewerArtifactKind::IosSimulatorApp, "aarch64", |event| {
                progress.push(event)
            })
            .await
            .unwrap();

        assert_eq!(downloaded.resolution, ArtifactResolution::Downloaded);
        assert_eq!(tokio::fs::read(&downloaded.path).await.unwrap(), viewer);
        assert!(progress.iter().any(|event| matches!(
            event,
            ArtifactCacheProgress::Downloading {
                bytes_received,
                total_bytes: Some(total_bytes)
            } if bytes_received == total_bytes
        )));

        drop(server);
        let reused = cache
            .prepare(MobileViewerArtifactKind::IosSimulatorApp, "arm64", |_| {})
            .await
            .unwrap();
        assert_eq!(reused.path, downloaded.path);
        assert!(matches!(reused.resolution, ArtifactResolution::CachedAfterFailure { .. }));
    }

    #[tokio::test]
    async fn failed_updates_keep_the_previous_valid_artifact() {
        let first_viewer = b"first valid viewer";
        let server = TestServer::start().await;
        let source = ArtifactSource::new(server.base_url(), "test").unwrap();
        server.set_release(viewer_manifest("viewer.zip", first_viewer), "viewer.zip", first_viewer);
        let cache_directory = tempfile::tempdir().unwrap();
        let cache = ArtifactCache::new(cache_directory.path().into(), source).unwrap();
        let first = cache
            .prepare(MobileViewerArtifactKind::IosSimulatorApp, "arm64", |_| {})
            .await
            .unwrap();

        let expected_update = b"expected updated viewer";
        server.set_release(
            viewer_manifest("viewer-update.zip", expected_update),
            "viewer-update.zip",
            b"truncated download",
        );
        let update = cache
            .prepare(MobileViewerArtifactKind::IosSimulatorApp, "arm64", |_| {})
            .await
            .unwrap();

        assert_eq!(update.path, first.path);
        assert_eq!(tokio::fs::read(&update.path).await.unwrap(), first_viewer);
        assert!(matches!(
            update.resolution,
            ArtifactResolution::CachedAfterFailure { ref reason }
                if reason.contains("checksum")
        ));
    }

    #[tokio::test]
    async fn incompatible_manifests_are_rejected_before_artifact_downloads() {
        let viewer = b"viewer";
        let server = TestServer::start().await;
        let source = ArtifactSource::new(server.base_url(), "test").unwrap();
        let mut manifest = viewer_manifest("viewer.zip", viewer);
        manifest.protocol = "slint-preview.0.1".into();
        server.set_release(manifest, "viewer.zip", viewer);
        let cache_directory = tempfile::tempdir().unwrap();
        let cache = ArtifactCache::new(cache_directory.path().into(), source).unwrap();

        let error = cache
            .prepare(MobileViewerArtifactKind::IosSimulatorApp, "arm64", |_| {})
            .await
            .unwrap_err()
            .to_string();

        assert!(error.contains(PROTOCOL_SUBPROTOCOL));
        assert_eq!(server.request_count("/test/viewer.zip"), 0);
    }

    #[tokio::test]
    async fn corrupt_cache_entries_are_preserved_and_replaced() {
        let viewer = b"valid viewer";
        let manifest = viewer_manifest("viewer.zip", viewer);
        let artifact = manifest.artifacts[0].clone();
        let server = TestServer::start().await;
        let source = ArtifactSource::new(server.base_url(), "test").unwrap();
        server.set_release(manifest, "viewer.zip", viewer);
        let cache_directory = tempfile::tempdir().unwrap();
        let cache = ArtifactCache::new(cache_directory.path().into(), source).unwrap();
        let cached_path = cache.artifact_path(&artifact);
        tokio::fs::create_dir_all(cached_path.parent().unwrap()).await.unwrap();
        tokio::fs::write(&cached_path, b"corrupt").await.unwrap();

        let prepared = cache
            .prepare(MobileViewerArtifactKind::IosSimulatorApp, "arm64", |_| {})
            .await
            .unwrap();

        assert_eq!(tokio::fs::read(prepared.path).await.unwrap(), viewer);
        let mut entries = tokio::fs::read_dir(cache.artifact_directory()).await.unwrap();
        let mut preserved = false;
        while let Some(entry) = entries.next_entry().await.unwrap() {
            preserved |= entry.file_name().to_string_lossy().contains(".invalid-");
        }
        assert!(preserved);
    }

    #[test]
    fn manifest_validation_checks_schema_version_release_and_architecture() {
        let source = ArtifactSource::new("https://releases.invalid", "test").unwrap();
        let mut manifest = viewer_manifest("viewer.zip", b"viewer");
        assert!(
            validate_manifest(
                &manifest,
                &source,
                MobileViewerArtifactKind::IosSimulatorApp,
                "arm64"
            )
            .is_ok()
        );

        manifest.schema_version += 1;
        assert!(
            validate_manifest(
                &manifest,
                &source,
                MobileViewerArtifactKind::IosSimulatorApp,
                "arm64"
            )
            .unwrap_err()
            .to_string()
            .contains("schema")
        );
        manifest.schema_version = MOBILE_VIEWER_ARTIFACT_SCHEMA_VERSION;
        manifest.release_tag = "other".into();
        assert!(
            validate_manifest(
                &manifest,
                &source,
                MobileViewerArtifactKind::IosSimulatorApp,
                "arm64"
            )
            .unwrap_err()
            .to_string()
            .contains("expected test")
        );
        manifest.release_tag = "test".into();
        assert!(
            validate_manifest(
                &manifest,
                &source,
                MobileViewerArtifactKind::IosSimulatorApp,
                "riscv64"
            )
            .unwrap_err()
            .to_string()
            .contains("riscv64")
        );
    }

    fn viewer_manifest(file_name: &str, contents: &[u8]) -> MobileViewerArtifactManifest {
        MobileViewerArtifactManifest {
            schema_version: MOBILE_VIEWER_ARTIFACT_SCHEMA_VERSION,
            release_tag: "test".into(),
            slint_version: SLINT_VERSION.into(),
            protocol: PROTOCOL_SUBPROTOCOL.into(),
            artifacts: vec![MobileViewerArtifact {
                kind: MobileViewerArtifactKind::IosSimulatorApp,
                file_name: file_name.into(),
                sha256: hex::encode(Sha256::digest(contents)),
                bundle_id: "dev.slint.slint-viewer".into(),
                architectures: vec!["arm64".into(), "x86_64".into()],
            }],
        }
    }

    struct TestServer {
        address: std::net::SocketAddr,
        responses: Arc<Mutex<BTreeMap<String, Vec<u8>>>>,
        requests: Arc<Mutex<BTreeMap<String, usize>>>,
        task: tokio::task::JoinHandle<()>,
    }

    impl TestServer {
        async fn start() -> Self {
            let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await.unwrap();
            let address = listener.local_addr().unwrap();
            let responses = Arc::new(Mutex::new(BTreeMap::<String, Vec<u8>>::new()));
            let requests = Arc::new(Mutex::new(BTreeMap::<String, usize>::new()));
            let task_responses = responses.clone();
            let task_requests = requests.clone();
            let task = tokio::spawn(async move {
                loop {
                    let Ok((mut stream, _)) = listener.accept().await else { break };
                    let mut request = vec![0; 8192];
                    let Ok(count) = stream.read(&mut request).await else { continue };
                    let request = String::from_utf8_lossy(&request[..count]);
                    let path = request
                        .lines()
                        .next()
                        .and_then(|line| line.split_whitespace().nth(1))
                        .unwrap_or("/")
                        .to_string();
                    *task_requests
                        .lock()
                        .unwrap_or_else(|error| error.into_inner())
                        .entry(path.clone())
                        .or_default() += 1;
                    let body = task_responses
                        .lock()
                        .unwrap_or_else(|error| error.into_inner())
                        .get(&path)
                        .cloned();
                    let (status, body) = body
                        .map(|body| ("200 OK", body))
                        .unwrap_or_else(|| ("404 Not Found", b"not found".to_vec()));
                    let response = format!(
                        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    if stream.write_all(response.as_bytes()).await.is_ok() {
                        let _ = stream.write_all(&body).await;
                    }
                }
            });
            Self { address, responses, requests, task }
        }

        fn base_url(&self) -> String {
            format!("http://{}/releases", self.address)
        }

        fn set_release(
            &self,
            manifest: MobileViewerArtifactManifest,
            file_name: &str,
            contents: &[u8],
        ) {
            let mut responses = self.responses.lock().unwrap_or_else(|error| error.into_inner());
            responses.insert(
                "/releases/test/slint-viewer-mobile-artifacts.json".into(),
                serde_json::to_vec(&manifest).unwrap(),
            );
            responses.insert(format!("/releases/test/{file_name}"), contents.into());
        }

        fn request_count(&self, path: &str) -> usize {
            self.requests
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .get(&format!("/releases{path}"))
                .copied()
                .unwrap_or_default()
        }
    }

    impl Drop for TestServer {
        fn drop(&mut self) {
            self.task.abort();
        }
    }
}
