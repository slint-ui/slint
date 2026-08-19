// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Local artifact resolution and caching for managed simulator targets.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{Context as _, Result, anyhow, bail};
use i_slint_live_preview::protocol::{PROTOCOL_SUBPROTOCOL, SLINT_VERSION};
use i_slint_springboard::{
    MOBILE_VIEWER_ARTIFACT_SCHEMA_VERSION, MobileViewerArtifact, MobileViewerArtifactKind,
    MobileViewerArtifactManifest,
};
use sha2::{Digest as _, Sha256};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

pub const ARTIFACT_DIR_ENVIRONMENT_VARIABLE: &str = "SLINT_SPRINGBOARD_ARTIFACT_DIR";
pub const MOBILE_VIEWER_ARTIFACT_MANIFEST_FILE: &str = "slint-viewer-mobile-artifacts.json";

/// The explicitly configured local artifact directory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ArtifactSource {
    Directory(PathBuf),
    Missing,
    Invalid(String),
}

impl ArtifactSource {
    /// Resolve the local artifact directory from Springboard's environment.
    pub fn from_environment() -> Self {
        Self::from_environment_value(std::env::var_os(ARTIFACT_DIR_ENVIRONMENT_VARIABLE))
    }

    /// Create an explicit local artifact source.
    pub fn new(directory: PathBuf) -> Result<Self> {
        if !directory.is_absolute() {
            bail!(
                "{ARTIFACT_DIR_ENVIRONMENT_VARIABLE} must name an absolute directory; got {}",
                directory.display()
            );
        }
        Ok(Self::Directory(directory))
    }

    fn from_environment_value(value: Option<OsString>) -> Self {
        let Some(value) = value else { return Self::Missing };
        let path = PathBuf::from(value);
        Self::new(path).unwrap_or_else(|error| Self::Invalid(error.to_string()))
    }

    fn directory(&self) -> Result<&Path> {
        match self {
            Self::Directory(directory) => Ok(directory),
            Self::Missing => bail!(
                "{ARTIFACT_DIR_ENVIRONMENT_VARIABLE} is not set. Set it to an absolute directory containing {MOBILE_VIEWER_ARTIFACT_MANIFEST_FILE} and the referenced iOS Simulator ZIP or Android APK."
            ),
            Self::Invalid(message) => bail!("{message}"),
        }
    }
}

/// Observable stages while Springboard prepares a managed viewer artifact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ArtifactCacheProgress {
    CheckingCache,
    ReadingManifest,
    Importing { bytes_copied: u64, total_bytes: Option<u64> },
    Validating,
    Ready { from_cache: bool },
    UsingPrevious { reason: String },
}

/// How an artifact was resolved.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ArtifactResolution {
    Imported,
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
#[derive(Clone)]
pub struct ArtifactCache {
    root: PathBuf,
    source: ArtifactSource,
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
        Ok(Self { root, source })
    }

    /// Import or reuse an artifact that matches this Springboard build and target architecture.
    pub async fn prepare(
        &self,
        kind: MobileViewerArtifactKind,
        architecture: &str,
        mut progress: impl FnMut(ArtifactCacheProgress),
    ) -> Result<CachedViewerArtifact> {
        let architecture = normalize_architecture(architecture);
        progress(ArtifactCacheProgress::CheckingCache);
        let previous = self.find_cached(kind, architecture).await;

        progress(ArtifactCacheProgress::ReadingManifest);
        let (manifest, manifest_bytes, source_directory) = match self.read_manifest().await {
            Ok(manifest) => manifest,
            Err(error) => return use_previous(previous, error, &mut progress),
        };
        let artifact = match validate_manifest(&manifest, kind, architecture) {
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

        match self.import_artifact(&source_directory, &artifact, &mut progress).await {
            Ok(path) => {
                progress(ArtifactCacheProgress::Ready { from_cache: false });
                Ok(CachedViewerArtifact {
                    path,
                    manifest,
                    artifact,
                    resolution: ArtifactResolution::Imported,
                })
            }
            Err(error) => use_previous(previous, error, &mut progress),
        }
    }

    fn manifest_directory(&self) -> PathBuf {
        self.root.join("manifests")
    }

    fn artifact_directory(&self) -> PathBuf {
        self.root.join("artifacts")
    }

    fn artifact_path(&self, artifact: &MobileViewerArtifact) -> PathBuf {
        self.artifact_directory().join(format!(
            "{}-{}",
            artifact.sha256.to_ascii_lowercase(),
            artifact.file_name
        ))
    }

    async fn read_manifest(&self) -> Result<(MobileViewerArtifactManifest, Vec<u8>, PathBuf)> {
        const MAX_MANIFEST_SIZE: u64 = 1024 * 1024;

        let directory = self.source.directory()?.to_path_buf();
        let path = directory.join(MOBILE_VIEWER_ARTIFACT_MANIFEST_FILE);
        let file = tokio::fs::File::open(&path).await.with_context(|| {
            format!(
                "Cannot read {} from {ARTIFACT_DIR_ENVIRONMENT_VARIABLE} directory {}",
                MOBILE_VIEWER_ARTIFACT_MANIFEST_FILE,
                directory.display()
            )
        })?;
        let mut bytes = Vec::new();
        file.take(MAX_MANIFEST_SIZE + 1)
            .read_to_end(&mut bytes)
            .await
            .context("Failed while reading the local viewer artifact manifest")?;
        if bytes.len() as u64 > MAX_MANIFEST_SIZE {
            bail!("The local viewer artifact manifest exceeds {MAX_MANIFEST_SIZE} bytes");
        }
        let manifest = serde_json::from_slice(&bytes)
            .context("The local viewer artifact manifest is malformed")?;
        Ok((manifest, bytes, directory))
    }

    async fn persist_manifest(&self, bytes: &[u8]) -> Result<()> {
        let directory = self.manifest_directory();
        tokio::fs::create_dir_all(&directory)
            .await
            .context("Failed to create the viewer manifest cache")?;
        let digest = hex::encode(Sha256::digest(bytes));
        persist_bytes_noclobber(&directory, &directory.join(format!("{digest}.json")), bytes).await
    }

    async fn import_artifact(
        &self,
        source_directory: &Path,
        artifact: &MobileViewerArtifact,
        progress: &mut impl FnMut(ArtifactCacheProgress),
    ) -> Result<PathBuf> {
        let source_path = source_directory.join(&artifact.file_name);
        let mut source = tokio::fs::File::open(&source_path).await.with_context(|| {
            format!("Cannot read local viewer artifact {}", source_path.display())
        })?;
        let total_bytes = source.metadata().await.ok().map(|metadata| metadata.len());
        let directory = self.artifact_directory();
        tokio::fs::create_dir_all(&directory)
            .await
            .context("Failed to create the viewer artifact cache")?;
        let temporary = tempfile::NamedTempFile::new_in(&directory)
            .context("Failed to create a temporary viewer artifact")?;
        let mut destination = tokio::fs::File::from_std(
            temporary.reopen().context("Failed to open the temporary viewer artifact")?,
        );
        let mut digest = Sha256::new();
        let mut bytes_copied = 0u64;
        let mut buffer = vec![0; 64 * 1024];
        progress(ArtifactCacheProgress::Importing { bytes_copied, total_bytes });
        loop {
            let count = source
                .read(&mut buffer)
                .await
                .with_context(|| format!("Failed while reading {}", source_path.display()))?;
            if count == 0 {
                break;
            }
            destination.write_all(&buffer[..count]).await.with_context(|| {
                format!("Failed to import {} into the Springboard cache", artifact.file_name)
            })?;
            digest.update(&buffer[..count]);
            bytes_copied += count as u64;
            progress(ArtifactCacheProgress::Importing { bytes_copied, total_bytes });
        }
        destination.sync_all().await.context("Failed to flush the imported viewer artifact")?;
        drop(destination);

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
            let Ok(artifact) = validate_manifest(&manifest, kind, architecture).cloned() else {
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
    if manifest.release_tag != "local" {
        bail!(
            "Viewer artifact manifest release_tag is {:?}, expected \"local\"",
            manifest.release_tag
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
    use super::*;

    #[test]
    fn artifact_source_requires_an_absolute_directory() {
        let error = ArtifactSource::new("relative/artifacts".into()).unwrap_err().to_string();
        assert!(error.contains(ARTIFACT_DIR_ENVIRONMENT_VARIABLE));
        assert!(error.contains("absolute"));

        let source = ArtifactSource::new(PathBuf::from("/tmp/slint-artifacts")).unwrap();
        assert_eq!(source, ArtifactSource::Directory("/tmp/slint-artifacts".into()));
    }

    #[test]
    fn missing_and_invalid_environment_values_are_preserved() {
        assert_eq!(ArtifactSource::from_environment_value(None), ArtifactSource::Missing);
        assert!(matches!(
            ArtifactSource::from_environment_value(Some("relative".into())),
            ArtifactSource::Invalid(message)
                if message.contains(ARTIFACT_DIR_ENVIRONMENT_VARIABLE)
        ));
    }

    #[tokio::test]
    async fn matching_artifacts_are_imported_and_reused_without_the_source() {
        let viewer = b"universal simulator viewer";
        let source_directory = tempfile::tempdir().unwrap();
        write_source(source_directory.path(), viewer_manifest("viewer.zip", viewer), viewer).await;
        let cache_directory = tempfile::tempdir().unwrap();
        let cache = ArtifactCache::new(
            cache_directory.path().into(),
            ArtifactSource::new(source_directory.path().into()).unwrap(),
        )
        .unwrap();
        let mut progress = Vec::new();

        let imported = cache
            .prepare(MobileViewerArtifactKind::IosSimulatorApp, "aarch64", |event| {
                progress.push(event)
            })
            .await
            .unwrap();

        assert_eq!(imported.resolution, ArtifactResolution::Imported);
        assert_eq!(tokio::fs::read(&imported.path).await.unwrap(), viewer);
        assert!(progress.iter().any(|event| matches!(
            event,
            ArtifactCacheProgress::Importing {
                bytes_copied,
                total_bytes: Some(total_bytes)
            } if bytes_copied == total_bytes
        )));

        drop(source_directory);
        let offline_cache =
            ArtifactCache::new(cache_directory.path().into(), ArtifactSource::Missing).unwrap();
        let reused = offline_cache
            .prepare(MobileViewerArtifactKind::IosSimulatorApp, "arm64", |_| {})
            .await
            .unwrap();
        assert_eq!(reused.path, imported.path);
        assert!(matches!(
            reused.resolution,
            ArtifactResolution::CachedAfterFailure { ref reason }
                if reason.contains(ARTIFACT_DIR_ENVIRONMENT_VARIABLE)
        ));
    }

    #[tokio::test]
    async fn failed_imports_keep_the_previous_valid_artifact() {
        let first_viewer = b"first valid viewer";
        let source_directory = tempfile::tempdir().unwrap();
        write_source(
            source_directory.path(),
            viewer_manifest("viewer.zip", first_viewer),
            first_viewer,
        )
        .await;
        let cache_directory = tempfile::tempdir().unwrap();
        let cache = ArtifactCache::new(
            cache_directory.path().into(),
            ArtifactSource::new(source_directory.path().into()).unwrap(),
        )
        .unwrap();
        let first = cache
            .prepare(MobileViewerArtifactKind::IosSimulatorApp, "arm64", |_| {})
            .await
            .unwrap();

        let expected_update = b"expected updated viewer";
        write_source(
            source_directory.path(),
            viewer_manifest("viewer-update.zip", expected_update),
            b"truncated local artifact",
        )
        .await;
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
    async fn incompatible_manifests_are_rejected_before_artifact_imports() {
        let viewer = b"viewer";
        let source_directory = tempfile::tempdir().unwrap();
        let mut manifest = viewer_manifest("viewer.zip", viewer);
        manifest.protocol = "slint-preview.0.1".into();
        write_source(source_directory.path(), manifest, viewer).await;
        let cache_directory = tempfile::tempdir().unwrap();
        let cache = ArtifactCache::new(
            cache_directory.path().into(),
            ArtifactSource::new(source_directory.path().into()).unwrap(),
        )
        .unwrap();

        let error = cache
            .prepare(MobileViewerArtifactKind::IosSimulatorApp, "arm64", |_| {})
            .await
            .unwrap_err()
            .to_string();

        assert!(error.contains(PROTOCOL_SUBPROTOCOL));
        assert!(!tokio::fs::try_exists(cache.artifact_directory()).await.unwrap());
    }

    #[tokio::test]
    async fn corrupt_cache_entries_are_preserved_and_replaced() {
        let viewer = b"valid viewer";
        let manifest = viewer_manifest("viewer.zip", viewer);
        let artifact = manifest.artifacts[0].clone();
        let source_directory = tempfile::tempdir().unwrap();
        write_source(source_directory.path(), manifest, viewer).await;
        let cache_directory = tempfile::tempdir().unwrap();
        let cache = ArtifactCache::new(
            cache_directory.path().into(),
            ArtifactSource::new(source_directory.path().into()).unwrap(),
        )
        .unwrap();
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
    fn manifest_validation_checks_schema_release_and_architecture() {
        let mut manifest = viewer_manifest("viewer.zip", b"viewer");
        assert!(
            validate_manifest(&manifest, MobileViewerArtifactKind::IosSimulatorApp, "arm64")
                .is_ok()
        );

        manifest.schema_version += 1;
        assert!(
            validate_manifest(&manifest, MobileViewerArtifactKind::IosSimulatorApp, "arm64")
                .unwrap_err()
                .to_string()
                .contains("schema")
        );
        manifest.schema_version = MOBILE_VIEWER_ARTIFACT_SCHEMA_VERSION;
        manifest.release_tag = "nightly".into();
        assert!(
            validate_manifest(&manifest, MobileViewerArtifactKind::IosSimulatorApp, "arm64")
                .unwrap_err()
                .to_string()
                .contains("local")
        );
        manifest.release_tag = "local".into();
        assert!(
            validate_manifest(&manifest, MobileViewerArtifactKind::IosSimulatorApp, "riscv64")
                .unwrap_err()
                .to_string()
                .contains("riscv64")
        );
    }

    async fn write_source(
        directory: &Path,
        manifest: MobileViewerArtifactManifest,
        contents: &[u8],
    ) {
        let artifact_path = directory.join(&manifest.artifacts[0].file_name);
        tokio::fs::write(
            directory.join(MOBILE_VIEWER_ARTIFACT_MANIFEST_FILE),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .await
        .unwrap();
        tokio::fs::write(artifact_path, contents).await.unwrap();
    }

    fn viewer_manifest(file_name: &str, contents: &[u8]) -> MobileViewerArtifactManifest {
        MobileViewerArtifactManifest {
            schema_version: MOBILE_VIEWER_ARTIFACT_SCHEMA_VERSION,
            release_tag: "local".into(),
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
}
