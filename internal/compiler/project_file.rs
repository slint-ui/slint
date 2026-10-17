// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::source_path::SourcePath;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    error::Error,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct ProjectFileData {
    /// The schema that editors validate the file against. The compiler ignores it.
    #[serde(rename = "$schema")]
    pub schema: Option<String>,

    pub library_paths: Option<HashMap<String, PathBuf>>,

    pub include_paths: Option<Vec<PathBuf>>,

    pub enable_experimental_features: Option<bool>,

    pub main: Option<PathBuf>,

    /// Selects the language behavior of this Slint release. Required in a project file.
    pub slint_edition: Option<SlintVersion>,
}

/// A Slint release, as `major.minor`.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SlintVersion {
    pub major: u32,
    pub minor: u32,
}

impl SlintVersion {
    pub const V1_19: Self = Self { major: 1, minor: 19 };

    /// The release of this compiler.
    pub fn current() -> Self {
        Self {
            major: env!("CARGO_PKG_VERSION_MAJOR").parse().unwrap(),
            minor: env!("CARGO_PKG_VERSION_MINOR").parse().unwrap(),
        }
    }
}

impl std::fmt::Display for SlintVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

impl std::str::FromStr for SlintVersion {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let number = |part: &str| {
            let valid = !part.is_empty()
                && part.bytes().all(|b| b.is_ascii_digit())
                && (part == "0" || !part.starts_with('0'));
            valid.then(|| part.parse().ok()).flatten()
        };
        s.split_once('.')
            .and_then(|(major, minor)| Some(Self { major: number(major)?, minor: number(minor)? }))
            .ok_or_else(|| {
                format!(
                    "'{s}' is not a Slint version: write the major and minor version, such as \"{}\"",
                    Self::current()
                )
            })
    }
}

impl Serialize for SlintVersion {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for SlintVersion {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl serde::de::Visitor<'_> for Visitor {
            type Value = SlintVersion;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "a Slint version string, such as \"{}\"", SlintVersion::current())
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
                v.parse().map_err(E::custom)
            }
        }
        deserializer.deserialize_str(Visitor)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectFile {
    source_path: SourcePath,
    data: ProjectFileData,
    edition: SlintVersion,
}

pub const FILE_NAME: &str = "slint-project.json";

/// Returns whether `path` names a project file, which tools compile by compiling its main file.
pub fn is_project_file(path: &Path) -> bool {
    path.file_name().is_some_and(|name| name == FILE_NAME)
}

/// Resolves the file a tool was asked to compile.
///
/// A project file resolves to its main file and applies itself, without a search.
/// Any other path resolves to itself, with the project file found for its directory.
pub fn resolve_input(input: &Path) -> Result<(PathBuf, Option<ProjectFile>), String> {
    if !is_project_file(input) {
        return Ok((
            input.to_path_buf(),
            ProjectFile::find(
                &SourcePath::new(input).parent().into_native_path().unwrap_or_default(),
            )?,
        ));
    }
    let project_file = ProjectFile::load(input)
        .map_err(|error| format!("Cannot load {}: {error}", input.display()))?;
    let entry = project_file
        .main()
        .ok_or_else(|| format!("{} has no 'main' to compile", project_file.source_path()))?;
    let entry = entry.into_native_path().ok_or_else(|| {
        format!("{} must specify a local file for 'main'", project_file.source_path())
    })?;
    Ok((entry, Some(project_file)))
}

#[derive(Clone, Debug)]
pub struct ResolvedInput {
    pub source: SourcePath,
    pub project_file: Option<ProjectFile>,
}

pub async fn resolve_input_with_file_loader(
    input: SourcePath,
    file_loader: Option<&crate::OpenImportCallback>,
) -> Result<Option<ResolvedInput>, String> {
    if input.file_name() != Some(FILE_NAME) {
        let project_file = ProjectFile::find_with_file_loader(input.parent(), file_loader).await?;
        return Ok(Some(ResolvedInput { source: input, project_file }));
    }
    let project_file = ProjectFile::load_with_file_loader(input.clone(), file_loader)
        .await
        .map_err(|error| format!("Cannot load {input}: {error}"))?;
    Ok(project_file.main().map(|source| ResolvedInput { source, project_file: Some(project_file) }))
}

/// Searches `directory` and its ancestors for a project file,
/// returning the path of the first one found.
pub fn find_project_file_path(directory: &Path) -> std::io::Result<Option<PathBuf>> {
    // On wasm std::fs reports Unsupported rather than NotFound, which would turn every
    // lookup below into an error. There is no filesystem to hold a project file anyway.
    if cfg!(target_arch = "wasm32") {
        return Ok(None);
    }

    find_project_file_path_with_exists(directory, |path| path.try_exists())
}

pub fn find_project_file_path_with_exists(
    directory: &Path,
    mut file_exists: impl FnMut(&Path) -> std::io::Result<bool>,
) -> std::io::Result<Option<PathBuf>> {
    for directory in directory.ancestors() {
        let candidate = directory.join(FILE_NAME);
        if file_exists(&candidate)? {
            return Ok(Some(candidate));
        }
    }
    Ok(None)
}

impl ProjectFile {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, Box<dyn Error>> {
        let source_path = normalize_project_file_path(SourcePath::File(
            crate::source_path::clean_path(path.as_ref()),
        ));
        let source = source_path.read()?.into_owned();
        Self::parse(source_path, source)
    }

    fn parse(source_path: SourcePath, mut source: Vec<u8>) -> Result<Self, Box<dyn Error>> {
        blank_out_comments(&mut source);
        let data: ProjectFileData = if source.iter().all(u8::is_ascii_whitespace) {
            ProjectFileData::default()
        } else {
            serde_json::from_slice(&source)?
        };
        let current = SlintVersion::current();
        let edition = match data.slint_edition {
            None => {
                return Err(format!(
                    "'slint-edition' is missing: add \"slint-edition\": \"{current}\""
                )
                .into());
            }
            Some(version) if version > current => {
                return Err(format!(
                    "'slint-edition' is {version}, newer than this Slint {current}: upgrade Slint, or set \"slint-edition\": \"{current}\""
                )
                .into());
            }
            Some(version) => version,
        };

        Ok(Self { source_path, data, edition })
    }

    pub async fn load_with_file_loader(
        path: SourcePath,
        file_loader: Option<&crate::OpenImportCallback>,
    ) -> Result<Self, Box<dyn Error>> {
        let path = normalize_project_file_path(path);
        let source = if let Some(result) = match file_loader {
            Some(file_loader) => file_loader(path.clone()).await,
            None => None,
        } {
            result?.into_bytes()
        } else {
            if cfg!(target_arch = "wasm32") && matches!(path, SourcePath::File(_)) {
                return Err(std::io::Error::from(std::io::ErrorKind::NotFound).into());
            }
            path.read()?.into_owned()
        };
        Self::parse(path, source)
    }

    pub async fn find_with_file_loader(
        mut directory: SourcePath,
        file_loader: Option<&crate::OpenImportCallback>,
    ) -> Result<Option<Self>, String> {
        directory = normalize_project_file_path(directory);
        directory = directory.join(".").unwrap_or(directory);
        loop {
            let Some(candidate) = directory.join(FILE_NAME) else { return Ok(None) };
            match Self::load_with_file_loader(candidate.clone(), file_loader).await {
                Ok(project_file) => return Ok(Some(project_file)),
                Err(error)
                    if error
                        .downcast_ref::<std::io::Error>()
                        .is_some_and(|error| error.kind() == std::io::ErrorKind::NotFound) => {}
                Err(error) => return Err(format!("Cannot load {candidate}: {error}")),
            }
            let parent = match &directory {
                SourcePath::File(_) => directory.parent(),
                _ => directory
                    .to_url()
                    .and_then(|url| url.join("..").ok())
                    .map(SourcePath::from)
                    .unwrap_or_else(|| directory.clone()),
            };
            if parent == directory {
                return Ok(None);
            }
            directory = parent;
        }
    }

    /// Loads the project file that applies to the files in `directory`, if there is one.
    pub fn find(directory: &Path) -> Result<Option<Self>, String> {
        let Some(path) = find_project_file_path(directory)
            .map_err(|error| format!("Cannot look for {FILE_NAME}: {error}"))?
        else {
            return Ok(None);
        };
        Self::load(&path)
            .map(Some)
            .map_err(|error| format!("Cannot load {}: {error}", path.display()))
    }

    pub fn source_path(&self) -> &SourcePath {
        &self.source_path
    }

    pub fn library_paths(&self) -> Option<&HashMap<String, PathBuf>> {
        self.data.library_paths.as_ref()
    }

    pub fn include_paths(&self) -> Option<&Vec<PathBuf>> {
        self.data.include_paths.as_ref()
    }

    pub fn enable_experimental_features(&self) -> Option<bool> {
        self.data.enable_experimental_features
    }

    pub fn slint_edition(&self) -> SlintVersion {
        self.edition
    }

    /// The `.slint` file to compile when a tool is given this project file.
    pub fn main(&self) -> Option<SourcePath> {
        self.source_path.parent().join(&self.data.main.as_ref()?.to_string_lossy())
    }

    pub fn into_compiler_configuration(
        &self,
        output_format: crate::generator::OutputFormat,
    ) -> crate::CompilerConfiguration {
        let mut compiler_config = crate::CompilerConfiguration::new(output_format);
        self.apply_to(&mut compiler_config);
        compiler_config
    }

    /// Applies the settings of the project file to `compiler_config`,
    /// leaving the settings the project file doesn't specify untouched.
    pub fn apply_to(&self, compiler_config: &mut crate::CompilerConfiguration) {
        self.resolved_data().apply_to(compiler_config);
    }

    pub fn resolved_data(&self) -> ProjectFileData {
        let project_directory = self.source_path.parent();
        let mut data = self.data.clone();
        if let Some(include_paths) = &mut data.include_paths {
            for path in include_paths {
                *path = resolve_source_relative_path(&project_directory, path.clone());
            }
        }
        if let Some(library_paths) = &mut data.library_paths {
            for path in library_paths.values_mut() {
                *path = resolve_source_relative_path(&project_directory, path.clone());
            }
        }
        #[expect(deprecated)]
        {
            data.main = self.main().map(|entry| entry.to_legacy_path());
        }
        data
    }
}

impl ProjectFileData {
    pub fn apply_to(&self, compiler_config: &mut crate::CompilerConfiguration) {
        let Self {
            schema: _,
            library_paths,
            include_paths,
            enable_experimental_features,
            main: _,
            slint_edition,
        } = self;
        if let Some(include_paths) = include_paths {
            compiler_config.include_paths = include_paths.clone();
        }
        if let Some(library_paths) = library_paths {
            compiler_config.library_paths = library_paths.clone();
        }
        if let Some(enable_experimental_features) = enable_experimental_features {
            compiler_config.enable_experimental = *enable_experimental_features;
        }
        if let Some(slint_edition) = slint_edition {
            compiler_config.edition = Some(*slint_edition);
        }
    }
}

/// The settings that a caller set explicitly through an API, which win over the project file.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(default)]
pub struct Overrides {
    pub project: ProjectFileData,
    pub style: Option<String>,
    #[serde(default, deserialize_with = "deserialize_supported_enum")]
    pub embed_resources: Option<crate::EmbedResourcesKind>,
    pub const_scale_factor: Option<f32>,
    #[cfg(feature = "bundle-translations")]
    pub bundled_translations_path: Option<PathBuf>,
    #[serde(default, deserialize_with = "deserialize_supported_enum")]
    pub default_translation_context: Option<crate::DefaultTranslationContext>,
    pub debug_info: Option<bool>,
    pub library_name: Option<String>,
    pub rust_module: Option<String>,
    #[cfg(all(feature = "renderer-software", feature = "sdf-fonts"))]
    pub use_sdf_fonts: Option<bool>,
}

fn deserialize_supported_enum<'de, D: serde::Deserializer<'de>, T: serde::de::DeserializeOwned>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    let value = serde_json::Value::deserialize(deserializer)?;
    match serde_json::from_value(value) {
        Ok(value) => Ok(value),
        Err(error) if error.to_string().starts_with("unknown variant") => Ok(None),
        Err(error) => Err(serde::de::Error::custom(error)),
    }
}

impl Overrides {
    /// Applies `project_file` to `config`, then these overrides.
    pub fn apply(
        &self,
        project_file: Option<&ProjectFile>,
        config: &mut crate::CompilerConfiguration,
    ) {
        if let Some(project_file) = project_file {
            project_file.apply_to(config);
        }
        let Self {
            project,
            style,
            embed_resources,
            const_scale_factor,
            #[cfg(feature = "bundle-translations")]
            bundled_translations_path,
            default_translation_context,
            debug_info,
            library_name,
            rust_module,
            #[cfg(all(feature = "renderer-software", feature = "sdf-fonts"))]
            use_sdf_fonts,
        } = self;
        project.apply_to(config);
        if let Some(style) = style {
            config.style = Some(style.clone());
        }
        if let Some(embed_resources) = embed_resources {
            config.embed_resources = *embed_resources;
        }
        if let Some(const_scale_factor) = const_scale_factor {
            config.const_scale_factor = Some(*const_scale_factor);
        }
        #[cfg(feature = "bundle-translations")]
        if let Some(bundled_translations_path) = bundled_translations_path {
            config.bundled_translations_path = Some(bundled_translations_path.clone());
        }
        if let Some(default_translation_context) = default_translation_context {
            config.default_translation_context = default_translation_context.clone();
        }
        if let Some(debug_info) = debug_info {
            config.debug_info = *debug_info;
        }
        if let Some(library_name) = library_name {
            config.library_name = Some(library_name.clone());
        }
        if let Some(rust_module) = rust_module {
            config.rust_module = Some(rust_module.clone());
        }
        #[cfg(all(feature = "renderer-software", feature = "sdf-fonts"))]
        if let Some(use_sdf_fonts) = use_sdf_fonts {
            config.use_sdf_fonts = *use_sdf_fonts;
        }
    }

    pub fn merge(&mut self, other: Self) {
        let Self {
            project,
            style,
            embed_resources,
            const_scale_factor,
            #[cfg(feature = "bundle-translations")]
            bundled_translations_path,
            default_translation_context,
            debug_info,
            library_name,
            rust_module,
            #[cfg(all(feature = "renderer-software", feature = "sdf-fonts"))]
            use_sdf_fonts,
        } = other;
        let ProjectFileData {
            schema,
            library_paths,
            include_paths,
            enable_experimental_features,
            main,
            slint_edition,
        } = project;
        replace_if_supplied(&mut self.project.schema, schema);
        replace_if_supplied(&mut self.project.library_paths, library_paths);
        replace_if_supplied(&mut self.project.include_paths, include_paths);
        replace_if_supplied(&mut self.style, style);
        replace_if_supplied(
            &mut self.project.enable_experimental_features,
            enable_experimental_features,
        );
        replace_if_supplied(&mut self.project.main, main);
        replace_if_supplied(&mut self.project.slint_edition, slint_edition);
        replace_if_supplied(&mut self.embed_resources, embed_resources);
        replace_if_supplied(&mut self.const_scale_factor, const_scale_factor);
        #[cfg(feature = "bundle-translations")]
        replace_if_supplied(&mut self.bundled_translations_path, bundled_translations_path);
        replace_if_supplied(&mut self.default_translation_context, default_translation_context);
        replace_if_supplied(&mut self.debug_info, debug_info);
        replace_if_supplied(&mut self.library_name, library_name);
        replace_if_supplied(&mut self.rust_module, rust_module);
        #[cfg(all(feature = "renderer-software", feature = "sdf-fonts"))]
        replace_if_supplied(&mut self.use_sdf_fonts, use_sdf_fonts);
    }
}

fn replace_if_supplied<T>(target: &mut Option<T>, supplied: Option<T>) {
    if supplied.is_some() {
        *target = supplied;
    }
}

/// Overwrites `//` and `/* */` comments with spaces, so that serde_json accepts the file.
/// Newlines are kept, so the line and column of a parse error stay correct.
///
/// Scanning bytes is enough: every byte of a multi-byte UTF-8 character has the high bit set.
fn blank_out_comments(source: &mut [u8]) {
    let mut index = 0;

    while index < source.len() {
        match source[index] {
            // Skip over string literals, a slash inside one is content.
            b'"' => {
                index += 1;
                while index < source.len() {
                    match source[index] {
                        b'\\' => index += 2,
                        b'"' => {
                            index += 1;
                            break;
                        }
                        _ => index += 1,
                    }
                }
            }
            b'/' if source.get(index + 1) == Some(&b'/') => {
                while index < source.len() && source[index] != b'\n' {
                    source[index] = b' ';
                    index += 1;
                }
            }
            b'/' if source.get(index + 1) == Some(&b'*') => {
                source[index] = b' ';
                source[index + 1] = b' ';
                index += 2;
                while index < source.len() {
                    if source[index] == b'*' && source.get(index + 1) == Some(&b'/') {
                        source[index] = b' ';
                        source[index + 1] = b' ';
                        index += 2;
                        break;
                    }
                    if source[index] != b'\n' {
                        source[index] = b' ';
                    }
                    index += 1;
                }
            }
            _ => index += 1,
        }
    }
}

pub fn normalize_project_file_path(path: SourcePath) -> SourcePath {
    match path {
        SourcePath::File(path) if !crate::source_path::is_absolute(&path.to_string_lossy()) => {
            SourcePath::new(std::env::current_dir().ok().unwrap_or_default())
                .join(&path.to_string_lossy())
                .unwrap_or(SourcePath::File(path))
        }
        path => path,
    }
}

#[expect(deprecated)]
fn resolve_source_relative_path(project_directory: &SourcePath, path: PathBuf) -> PathBuf {
    project_directory
        .join(&path.to_string_lossy())
        .map(|path| path.to_legacy_path())
        .unwrap_or(path)
}

/// Resolves `path` against `directory` the way an import is resolved.
/// `\` separates directories on every host, and an absolute `path` is returned cleaned up.
/// Use it for a path written by hand relative to a directory, such as in a project file
/// or in a build script.
#[expect(deprecated)]
pub fn resolve_relative_path(directory: &Path, path: PathBuf) -> PathBuf {
    path.to_str()
        .and_then(|relative| SourcePath::new(directory).join(relative))
        .map(|path| path.to_legacy_path())
        .unwrap_or_else(|| directory.join(path))
}

#[cfg(test)]
mod tests {
    use super::{
        FILE_NAME, ProjectFile, SlintVersion, find_project_file_path, is_project_file,
        resolve_input, resolve_relative_path,
    };
    use crate::{generator::OutputFormat, source_path::SourcePath};
    use std::{
        collections::HashMap,
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn resolve_relative_path_uses_import_separators() {
        let directory = std::env::temp_dir().join("project");
        let expected = directory.join("ui").join("main.slint");
        assert_eq!(resolve_relative_path(&directory, "ui/main.slint".into()), expected);
        // A build script written on Windows passes this, and Slint 1.18 accepted it everywhere
        assert_eq!(resolve_relative_path(&directory, "ui\\main.slint".into()), expected);
        assert_eq!(resolve_relative_path(&directory, expected.clone()), expected);
    }

    #[test]
    fn build_overrides_apply_after_project_settings_and_existing_defaults() {
        with_project_file_contents(
            r#"{"slint-edition":"1.19","include-paths":["include"],"library-paths":{"widgets":"lib"},"enable-experimental-features":true}"#,
            |path| {
                let project = ProjectFile::load(path).unwrap();
                let mut config = crate::CompilerConfiguration::new(OutputFormat::Interpreter);
                config.debug_info = true;
                config.const_scale_factor = Some(2.);
                let overrides = super::Overrides {
                    project: super::ProjectFileData {
                        include_paths: Some(vec![]),
                        library_paths: Some(HashMap::new()),
                        enable_experimental_features: Some(false),
                        ..Default::default()
                    },
                    debug_info: Some(false),
                    style: Some(String::new()),
                    const_scale_factor: Some(0.),
                    ..Default::default()
                };
                overrides.apply(Some(&project), &mut config);
                assert!(config.include_paths.is_empty());
                assert!(config.library_paths.is_empty());
                assert_eq!(config.style.as_deref(), Some(""));
                assert!(!config.enable_experimental);
                assert!(!config.debug_info);
                assert_eq!(config.const_scale_factor, Some(0.));
            },
        );
    }

    #[test]
    fn option_merge_replaces_supplied_collections_and_preserves_omitted_settings() {
        let mut overrides = super::Overrides {
            project: super::ProjectFileData {
                include_paths: Some(vec![PathBuf::from("startup")]),
                library_paths: Some(HashMap::from([("startup".into(), PathBuf::from("lib"))])),
                enable_experimental_features: Some(true),
                ..Default::default()
            },
            debug_info: Some(true),
            style: Some("fluent".into()),
            ..Default::default()
        };
        overrides.merge(super::Overrides {
            project: super::ProjectFileData {
                include_paths: Some(vec![]),
                library_paths: Some(HashMap::new()),
                enable_experimental_features: Some(false),
                ..Default::default()
            },
            debug_info: Some(false),
            ..Default::default()
        });
        assert_eq!(overrides.project.include_paths, Some(vec![]));
        assert_eq!(overrides.project.library_paths, Some(HashMap::new()));
        assert_eq!(overrides.style.as_deref(), Some("fluent"));
        assert_eq!(overrides.project.enable_experimental_features, Some(false));
        assert_eq!(overrides.debug_info, Some(false));
    }

    #[test]
    fn partially_specified_project_file_is_valid() {
        let parsed = load_project_file(
            r#"{
                "slint-edition": "1.19",
                "enable-experimental-features": true
            }"#,
        )
        .unwrap();

        assert_eq!(parsed.enable_experimental_features(), Some(true));
        assert_eq!(parsed.library_paths(), None);
        assert_eq!(parsed.include_paths(), None);
    }

    #[test]
    fn snake_case_keys_are_rejected() {
        let error = load_project_file(r#"{"include_paths": ["include"]}"#).unwrap_err();
        assert!(error.to_string().contains("unknown field"), "{error}");
    }

    #[test]
    fn kebab_case_project_file_keys_are_valid() {
        let parsed = load_project_file(
            r#"{
                "slint-edition": "1.19",
                "library-paths": {"widgets": "libs"},
                "include-paths": ["include"],
                "enable-experimental-features": true
            }"#,
        )
        .unwrap();

        assert_eq!(
            parsed.library_paths(),
            Some(&HashMap::from([("widgets".into(), PathBuf::from("libs"))]))
        );
        assert_eq!(parsed.include_paths(), Some(&vec![PathBuf::from("include")]));
        assert_eq!(parsed.enable_experimental_features(), Some(true));
    }

    #[test]
    fn a_schema_reference_is_accepted() {
        let parsed = load_project_file(
            r#"{
                "slint-edition": "1.19",
                "$schema": "https://slint.dev/slint-project.schema.json",
                "include-paths": ["include"]
            }"#,
        )
        .unwrap();

        assert_eq!(parsed.include_paths(), Some(&vec![PathBuf::from("include")]));
    }

    #[test]
    fn a_schema_reference_alone_is_accepted() {
        let parsed = load_project_file(
            r#"{"slint-edition":"1.19","$schema": "https://slint.dev/slint-project.schema.json"}"#,
        )
        .unwrap();

        assert_eq!(parsed.include_paths(), None);
    }

    #[test]
    fn line_comments_are_accepted() {
        let parsed = load_project_file(
            r#"{
                "slint-edition": "1.19",
                "include-paths": ["include"] // trailing comment
            }"#,
        )
        .unwrap();

        assert_eq!(parsed.include_paths(), Some(&vec![PathBuf::from("include")]));
    }

    #[test]
    fn block_comments_are_accepted() {
        let parsed = load_project_file(
            r#"{
                "slint-edition": "1.19",
                /* Disabled until the upgrade:
                   "include-paths": ["disabled"],
                */
                "include-paths": ["include"] /* here too */
            }"#,
        )
        .unwrap();

        assert_eq!(parsed.include_paths(), Some(&vec![PathBuf::from("include")]));
    }

    #[test]
    fn comment_markers_inside_strings_are_kept() {
        let parsed = load_project_file(
            r#"{
                "slint-edition": "1.19",
                "include-paths": ["not//a/comment", "not/*a*/comment", "with \" quote // and slashes"]
            }"#,
        )
        .unwrap();

        assert_eq!(
            parsed.include_paths(),
            Some(&vec![
                PathBuf::from("not//a/comment"),
                PathBuf::from("not/*a*/comment"),
                PathBuf::from("with \" quote // and slashes")
            ])
        );
    }

    #[test]
    fn a_comment_keeps_the_line_of_a_later_error() {
        let error = load_project_file(
            r#"{
                "slint-edition": "1.19",
                // one
                // two
                "include-paths": nonsense
            }"#,
        )
        .unwrap_err();

        // The blanked comments keep the offsets, so the error names line 5.
        assert!(error.to_string().contains("line 5"), "{error}");
    }

    #[test]
    fn field_errors_keep_the_first_field_and_its_source_position() {
        for (source, message, line, column) in [
            (
                r#"{
  "$schema": 42,
  "main": "main.slint"
}"#,
                "invalid type",
                2,
                15,
            ),
            (
                r#"{
  "first-unknown": true,
  "second-unknown": false
}"#,
                "unknown field `first-unknown`",
                2,
                17,
            ),
        ] {
            let error = load_project_file(&source).unwrap_err();
            let error = error.downcast_ref::<serde_json::Error>().unwrap();
            assert!(error.to_string().contains(message), "{error}");
            assert_eq!((error.line(), error.column()), (line, column), "{error}");
        }
    }

    #[test]
    fn unknown_settings_are_rejected() {
        let error = load_project_file(r#"{"unknown_setting": true}"#).unwrap_err();
        assert!(error.to_string().contains("unknown field"));
    }

    #[test]
    fn the_edition_is_required() {
        let cases = ["", "{}", "{\n}", "  \t \n", "// nothing set yet\n/* not even this */\n"];

        for case in cases {
            let error = load_project_file(case).unwrap_err().to_string();
            let current = SlintVersion::current();
            assert!(
                error.contains(&format!(r#"add "slint-edition": "{current}""#)),
                "{case:?}: {error}"
            );
        }
    }

    #[test]
    fn a_minimal_project_file_is_valid() {
        let parsed = load_project_file(r#"{ "slint-edition": "1.19" }"#).unwrap();
        assert_eq!(parsed.slint_edition(), SlintVersion::V1_19);
        assert_eq!(parsed.library_paths(), None);
        assert_eq!(parsed.include_paths(), None);
        assert_eq!(parsed.enable_experimental_features(), None);
    }

    #[test]
    fn an_older_edition_is_valid() {
        let parsed = load_project_file(r#"{ "slint-edition": "1.10" }"#).unwrap();
        assert_eq!(parsed.slint_edition(), SlintVersion { major: 1, minor: 10 });
    }

    #[test]
    fn a_newer_edition_is_rejected() {
        let current = SlintVersion::current();
        let newer = SlintVersion { minor: current.minor + 1, ..current };
        let error = load_project_file(&format!(r#"{{ "slint-edition": "{newer}" }}"#)).unwrap_err();
        let expected = format!(
            r#"'slint-edition' is {newer}, newer than this Slint {current}: upgrade Slint, or set "slint-edition": "{current}""#
        );
        assert!(error.to_string().contains(&expected), "{error}");
    }

    #[test]
    fn a_malformed_edition_is_rejected() {
        for value in [r#""1.19.0""#, r#""1""#, r#""1.x""#, r#""1.019""#, r#""""#, "1.19", "119"] {
            let error = load_project_file(&format!(r#"{{ "slint-edition": {value} }}"#))
                .unwrap_err()
                .to_string();
            assert!(error.contains("such as"), "{value}: {error}");
            assert!(error.contains("line 1"), "{value}: {error}");
        }
    }

    #[test]
    fn editions_compare_by_number() {
        let parse = |s: &str| s.parse::<SlintVersion>().unwrap();
        assert!(parse("1.9") < parse("1.10"));
        assert!(parse("1.19") < parse("2.0"));
        assert_eq!(parse("1.0").to_string(), "1.0");
    }

    #[test]
    fn the_edition_selects_the_behavior() {
        let config = |source: &str| {
            with_project_file_contents(source, |path| {
                ProjectFile::load(path).unwrap().into_compiler_configuration(OutputFormat::Llr)
            })
        };
        assert!(config(r#"{ "slint-edition": "1.19" }"#).repeated_children_constrain_parent());
        assert!(!config(r#"{ "slint-edition": "1.18" }"#).repeated_children_constrain_parent());
    }

    #[test]
    fn stores_loaded_project_file_path() {
        with_project_file_contents(r#"{ "slint-edition": "1.19" }"#, |path| {
            let project = ProjectFile::load(path).unwrap();
            assert_eq!(project.source_path(), &crate::source_path::SourcePath::new(path));
        });
    }

    #[cfg(unix)]
    #[test]
    fn native_loading_preserves_colons_in_relative_directory_names() {
        let unique_path = unique_temp_file_path();
        let directory = PathBuf::from(format!(
            "data:{}",
            unique_path.parent().unwrap().file_name().unwrap().to_string_lossy(),
        ));
        let path = directory.join(FILE_NAME);
        fs::create_dir(&directory).unwrap();
        fs::write(&path, r#"{"slint-edition":"1.19","main":"native"}"#).unwrap();
        let loaded = ProjectFile::load(&path);
        fs::remove_file(&path).unwrap();
        fs::remove_dir(&directory).unwrap();
        let project = loaded.unwrap();
        assert_eq!(project.main(), Some(SourcePath::File(directory.join("native"))));
        assert_eq!(project.source_path(), &SourcePath::File(path));
    }

    #[test]
    fn project_file_converts_to_compiler_configuration() {
        with_project_file_contents(
            r#"{
                "slint-edition": "1.19",
                "library-paths": {"widgets": "libraries/widgets.slint"},
                "include-paths": ["include", "../shared"],
                "enable-experimental-features": true
            }"#,
            |path| {
                let project = ProjectFile::load(path).unwrap();
                let compiler_config =
                    project.into_compiler_configuration(OutputFormat::Interpreter);
                let project_directory = path.parent().unwrap();

                assert_eq!(
                    compiler_config.include_paths,
                    vec![
                        project_directory.join("include"),
                        project_directory.parent().unwrap().join("shared"),
                    ]
                );
                assert_eq!(
                    compiler_config.library_paths,
                    HashMap::from([(
                        "widgets".into(),
                        project_directory.join("libraries/widgets.slint"),
                    )])
                );
                assert!(compiler_config.enable_experimental);
            },
        );
    }

    #[test]
    fn project_file_conversion_preserves_defaults_for_omitted_settings() {
        with_project_file_contents(r#"{ "slint-edition": "1.19" }"#, |path| {
            let project = ProjectFile::load(path).unwrap();
            let compiler_config = project.into_compiler_configuration(OutputFormat::Interpreter);
            let default_config = crate::CompilerConfiguration::new(OutputFormat::Interpreter);

            assert_eq!(compiler_config.include_paths, default_config.include_paths);
            assert_eq!(compiler_config.library_paths, default_config.library_paths);
            assert_eq!(compiler_config.style, default_config.style);
            assert_eq!(compiler_config.enable_experimental, default_config.enable_experimental);
        });
    }

    #[test]
    fn existence_callback_probes_nearest_first_and_stops_at_a_match() {
        let directory = Path::new("project/ui/widgets");
        let target = Path::new("project/ui").join(FILE_NAME);
        let mut probes = Vec::new();
        let found = super::find_project_file_path_with_exists(directory, |path| {
            probes.push(path.to_path_buf());
            Ok(path == target)
        })
        .unwrap();
        assert_eq!(found, Some(target.clone()));
        assert_eq!(probes, [directory.join(FILE_NAME), target]);
    }

    #[test]
    fn existence_callback_errors_stop_the_ancestor_search() {
        let directory = Path::new("project/ui/widgets");
        let mut probes = Vec::new();
        let error = super::find_project_file_path_with_exists(directory, |path| {
            probes.push(path.to_path_buf());
            if probes.len() == 2 {
                Err(std::io::ErrorKind::PermissionDenied.into())
            } else {
                Ok(false)
            }
        })
        .unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
        assert_eq!(probes, [directory.join(FILE_NAME), Path::new("project/ui").join(FILE_NAME)]);
    }

    #[test]
    fn the_nearest_project_file_is_found() {
        let root = unique_temp_file_path().parent().unwrap().to_path_buf();
        let nested = root.join("a/b");
        fs::create_dir_all(&nested).unwrap();

        fs::write(root.join(FILE_NAME), r#"{ "slint-edition": "1.19" }"#).unwrap();
        assert_eq!(find_project_file_path(&nested).unwrap(), Some(root.join(FILE_NAME)));

        fs::write(root.join("a").join(FILE_NAME), r#"{ "slint-edition": "1.19" }"#).unwrap();
        assert_eq!(find_project_file_path(&nested).unwrap(), Some(root.join("a").join(FILE_NAME)));

        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn the_main_file_is_relative_to_the_project_file() {
        with_project_file_contents(
            r#"{ "slint-edition": "1.19", "main": "ui/main.slint" }"#,
            |path| {
                let project = ProjectFile::load(path).unwrap();
                assert_eq!(
                    project.main(),
                    Some(crate::source_path::SourcePath::new(
                        path.parent().unwrap().join("ui/main.slint")
                    ))
                );
            },
        );
    }

    #[test]
    fn a_project_file_input_resolves_to_its_main_file() {
        with_project_file_contents(
            r#"{ "slint-edition": "1.19", "main": "main.slint", "include-paths": ["include"] }"#,
            |path| {
                let (slint_file, project) = resolve_input(path).unwrap();
                assert_eq!(slint_file, path.parent().unwrap().join("main.slint"));
                assert_eq!(project.unwrap().include_paths(), Some(&vec![PathBuf::from("include")]));
            },
        );
    }

    #[test]
    fn a_project_file_input_without_a_main_file_is_an_error() {
        with_project_file_contents(r#"{ "slint-edition": "1.19" }"#, |path| {
            let error = resolve_input(path).unwrap_err();
            assert!(error.contains("'main'"), "{error}");
        });
    }

    #[test]
    fn a_project_file_input_with_a_non_local_main_file_is_an_error() {
        with_project_file_contents(
            r#"{ "slint-edition": "1.19", "main": "https://example.com/main.slint" }"#,
            |path| {
                assert_eq!(
                    resolve_input(path).unwrap_err(),
                    format!("{} must specify a local file for 'main'", SourcePath::new(path))
                );
            },
        );
    }

    #[test]
    fn a_project_file_input_ignores_a_nearer_project_file() {
        with_project_file_contents(
            r#"{ "slint-edition": "1.19", "main": "ui/main.slint", "include-paths": ["outer"] }"#,
            |path| {
                let ui = path.parent().unwrap().join("ui");
                fs::create_dir_all(&ui).unwrap();
                fs::write(
                    ui.join(FILE_NAME),
                    r#"{ "slint-edition": "1.19", "include-paths": ["inner"] }"#,
                )
                .unwrap();

                let (_, project) = resolve_input(path).unwrap();

                fs::remove_dir_all(&ui).unwrap();
                assert_eq!(project.unwrap().include_paths(), Some(&vec![PathBuf::from("outer")]));
            },
        );
    }

    #[test]
    fn a_slint_file_input_resolves_to_itself() {
        with_project_file_contents(
            r#"{ "slint-edition": "1.19", "main": "other.slint" }"#,
            |path| {
                let main = path.parent().unwrap().join("main.slint");
                let (slint_file, project) = resolve_input(&main).unwrap();
                assert_eq!(slint_file, main);
                assert_eq!(
                    project.unwrap().source_path(),
                    &crate::source_path::SourcePath::new(path)
                );
            },
        );
    }

    #[test]
    fn only_the_exact_file_name_is_a_project_file() {
        assert!(is_project_file(Path::new("ui/slint-project.json")));
        assert!(!is_project_file(Path::new("ui/other.json")));
        assert!(!is_project_file(Path::new("ui/main.slint")));
    }

    fn callback_loader(
        files: HashMap<SourcePath, String>,
        requested: std::rc::Rc<std::cell::RefCell<Vec<SourcePath>>>,
    ) -> crate::OpenImportCallback {
        std::rc::Rc::new(move |path| {
            requested.borrow_mut().push(path.clone());
            let contents = files.get(&path).cloned();
            Box::pin(
                async move { Some(contents.ok_or_else(|| std::io::ErrorKind::NotFound.into())) },
            )
        })
    }

    #[test]
    fn callback_loading_shares_parsing_and_relative_paths() {
        for source in [
            "// empty\n/* manifest */",
            r#"{"slint-edition":"1.19","main":"ui/main.slint","include-paths":["include","../shared"],"library-paths":{"widgets":"lib/main.slint"}}"#,
            r#"{"unknown":true}"#,
            "{\n// preserved line\n\"main\": invalid}",
        ] {
            with_project_file_contents(source, |path| {
                let source_path = SourcePath::new(path);
                let loader = callback_loader(
                    HashMap::from([(source_path.clone(), source.into())]),
                    Default::default(),
                );
                let disk = ProjectFile::load(path);
                let callback = spin_on::spin_on(ProjectFile::load_with_file_loader(
                    source_path,
                    Some(&loader),
                ));
                match (disk, callback) {
                    (Ok(disk), Ok(callback)) => {
                        assert_eq!(disk, callback);
                        assert_eq!(disk.resolved_data(), callback.resolved_data());
                    }
                    (Err(disk), Err(callback)) => {
                        assert_eq!(disk.to_string(), callback.to_string())
                    }
                    results => panic!("Different load results: {results:?}"),
                }
            });
        }
    }

    #[test]
    #[expect(deprecated)]
    fn callback_paths_resolve_against_the_manifest_url() {
        let manifest = SourcePath::new("https://example.com/project/slint-project.json");
        let loader = callback_loader(
            HashMap::from([(manifest.clone(), r#"{"slint-edition":"1.19","main":"ui/main.slint","include-paths":["include","../shared"],"library-paths":{"widgets":"lib/main.slint"}}"#.into())]),
            Default::default(),
        );
        let project =
            spin_on::spin_on(ProjectFile::load_with_file_loader(manifest.clone(), Some(&loader)))
                .unwrap();
        assert_eq!(project.source_path(), &manifest);
        assert_eq!(
            project.main(),
            Some(SourcePath::new("https://example.com/project/ui/main.slint"))
        );
        let resolved = project.resolved_data();
        assert_eq!(
            resolved.include_paths,
            Some(vec![
                PathBuf::from("https://example.com/project/include"),
                PathBuf::from("https://example.com/shared")
            ])
        );
        assert_eq!(
            resolved.library_paths,
            Some(HashMap::from([(
                "widgets".into(),
                PathBuf::from("https://example.com/project/lib/main.slint")
            )]))
        );
        assert_eq!(resolved.main, project.main().map(|entry| entry.to_legacy_path()));
    }

    #[test]
    fn callback_result_is_authoritative_and_none_falls_back_to_disk() {
        with_project_file_contents(r#"{"slint-edition":"1.19","main":"disk"}"#, |path| {
            let path = SourcePath::new(path);
            for (response, expected) in [
                (Some(Ok(r#"{"slint-edition":"1.19","main":"callback"}"#)), Some("callback")),
                (None, Some("disk")),
                (Some(Err(std::io::ErrorKind::NotFound)), None),
                (Some(Err(std::io::ErrorKind::PermissionDenied)), None),
            ] {
                let loader: crate::OpenImportCallback = std::rc::Rc::new(move |_| {
                    Box::pin(async move {
                        response.map(|result| result.map(str::to_owned).map_err(Into::into))
                    })
                });
                let result = spin_on::spin_on(ProjectFile::load_with_file_loader(
                    path.clone(),
                    Some(&loader),
                ));
                if let Some(expected) = expected {
                    assert_eq!(result.unwrap().main(), path.parent().join(expected));
                } else {
                    assert_eq!(
                        result.unwrap_err().downcast_ref::<std::io::Error>().unwrap().kind(),
                        response.unwrap().unwrap_err()
                    );
                }
            }
        });
    }

    #[test]
    fn callback_discovery_checks_nearest_ancestor_and_stops_at_roots() {
        for base in [
            "/virtual/project/",
            "https://example.com/project/",
            "vscode-remote://host/project/",
            "file:///C:/project/",
        ] {
            let directory = SourcePath::new(base);
            let manifest = directory.join(FILE_NAME).unwrap();
            let nested = directory.join("ui/nested").unwrap();
            let requested = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
            let loader = callback_loader(
                HashMap::from([
                    (manifest.clone(), r#"{"slint-edition":"1.19"}"#.into()),
                    (
                        directory.join("../slint-project.json").unwrap(),
                        r#"{"slint-edition":"1.19","main":"outer"}"#.into(),
                    ),
                ]),
                requested.clone(),
            );
            let project =
                spin_on::spin_on(ProjectFile::find_with_file_loader(nested.clone(), Some(&loader)))
                    .unwrap()
                    .unwrap();
            assert_eq!(project.source_path(), &manifest);
            assert_eq!(requested.borrow().len(), 3);
            let loader = callback_loader(HashMap::new(), requested.clone());
            requested.borrow_mut().clear();
            assert!(
                spin_on::spin_on(ProjectFile::find_with_file_loader(nested, Some(&loader)))
                    .unwrap()
                    .is_none()
            );
            let requested = requested.borrow();
            let distinct = requested.iter().collect::<std::collections::HashSet<_>>();
            assert_eq!(requested.len(), distinct.len());
            assert!(requested.len() < 8);
        }
    }

    #[cfg(target_arch = "wasm32")]
    #[test]
    fn wasm_discovery_without_a_filesystem_loader_finds_no_project() {
        let declined_loader: crate::OpenImportCallback =
            std::rc::Rc::new(|_| Box::pin(async { None }));
        for loader in [None, Some(&declined_loader)] {
            let source = SourcePath::new("/project/ui/main.slint");
            assert!(
                spin_on::spin_on(ProjectFile::find_with_file_loader(source.parent(), loader))
                    .unwrap()
                    .is_none()
            );
            let resolved =
                spin_on::spin_on(super::resolve_input_with_file_loader(source.clone(), loader))
                    .unwrap()
                    .unwrap();
            assert_eq!(resolved.source, source);
            assert!(resolved.project_file.is_none());
        }
    }

    #[test]
    fn callback_discovery_only_continues_after_not_found() {
        for directory in ["https://example.com/a/b/", "/virtual/project/ui"] {
            for contents in [
                Ok("not json"),
                Err(std::io::ErrorKind::PermissionDenied),
                Err(std::io::ErrorKind::Unsupported),
                Err(std::io::ErrorKind::Other),
            ] {
                let requested = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
                let seen = requested.clone();
                let loader: crate::OpenImportCallback = std::rc::Rc::new(move |path| {
                    seen.borrow_mut().push(path);
                    Box::pin(async move { Some(contents.map(str::to_owned).map_err(Into::into)) })
                });
                assert!(
                    spin_on::spin_on(ProjectFile::find_with_file_loader(
                        SourcePath::new(directory),
                        Some(&loader)
                    ))
                    .is_err()
                );
                assert_eq!(requested.borrow().len(), 1);
            }
        }
    }

    #[test]
    fn callback_input_resolution_preserves_source_and_explicit_manifest() {
        let root = SourcePath::new("https://example.com/project/");
        let manifest = root.join(FILE_NAME).unwrap();
        let source = root.join("ui/main.slint").unwrap();
        for entry in [None, Some("ui/main.slint")] {
            let contents = entry.map_or_else(
                || r#"{"slint-edition":"1.19"}"#.to_string(),
                |entry| format!(r#"{{"slint-edition":"1.19","main":"{entry}"}}"#),
            );
            let loader = callback_loader(
                HashMap::from([
                    (manifest.clone(), contents),
                    (
                        root.join("ui/slint-project.json").unwrap(),
                        r#"{"slint-edition":"1.19","main":"nested"}"#.into(),
                    ),
                ]),
                Default::default(),
            );
            let resolved = spin_on::spin_on(super::resolve_input_with_file_loader(
                manifest.clone(),
                Some(&loader),
            ))
            .unwrap();
            assert_eq!(resolved.is_some(), entry.is_some());
            if let Some(resolved) = resolved {
                assert_eq!(resolved.source, source);
                assert_eq!(resolved.project_file.unwrap().source_path(), &manifest);
            }
            let resolved = spin_on::spin_on(super::resolve_input_with_file_loader(
                source.clone(),
                Some(&loader),
            ))
            .unwrap()
            .unwrap();
            assert_eq!(resolved.source, source);
            assert_eq!(resolved.project_file.unwrap().main(), root.join("ui/nested"));
        }
        let loader = callback_loader(
            HashMap::from([(manifest.clone(), "broken".into())]),
            Default::default(),
        );
        assert!(
            spin_on::spin_on(super::resolve_input_with_file_loader(manifest, Some(&loader)))
                .is_err()
        );
    }

    fn load_project_file(source: &str) -> Result<ProjectFile, Box<dyn std::error::Error>> {
        with_project_file_contents(source, |path| ProjectFile::load(path))
    }

    fn with_project_file_contents<R>(source: &str, f: impl FnOnce(&Path) -> R) -> R {
        let path = unique_temp_file_path();
        let directory = path.parent().unwrap();
        fs::create_dir_all(directory).unwrap();
        fs::write(&path, source).unwrap();

        let result = f(&path);

        fs::remove_file(&path).unwrap();
        fs::remove_dir(directory).unwrap();
        result
    }

    fn unique_temp_file_path() -> PathBuf {
        // The clock is too coarse to separate tests running in parallel on its own.
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let count = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        std::env::temp_dir()
            .join(format!("slint-project-file-test-{stamp}-{count}"))
            .join(FILE_NAME)
    }
}
