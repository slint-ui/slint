// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::source_path::SourcePath;
use serde::Deserialize;
use std::{
    collections::HashMap,
    error::Error,
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct ProjectFileData {
    /// The schema that editors validate the file against. The compiler ignores it.
    #[serde(rename = "$schema")]
    pub schema: Option<String>,

    pub library_paths: Option<HashMap<String, PathBuf>>,

    pub include_paths: Option<Vec<PathBuf>>,

    pub style: Option<String>,

    pub enable_experimental_features: Option<bool>,

    pub entry: Option<PathBuf>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProjectFile {
    source_path: PathBuf,
    data: ProjectFileData,
}

pub const FILE_NAME: &str = "slint-project.json";

/// Returns whether `path` names a project file, which tools compile by compiling its entry.
pub fn is_project_file(path: &Path) -> bool {
    path.file_name().is_some_and(|name| name == FILE_NAME)
}

/// Resolves the file a tool was asked to compile.
///
/// A project file resolves to its entry and applies itself, without a search.
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
    let entry = project_file.entry().ok_or_else(|| {
        format!("{} has no 'entry' to compile", project_file.source_path().display())
    })?;
    Ok((entry, Some(project_file)))
}

/// Searches `directory` and its ancestors for a project file,
/// returning the path of the first one found.
pub fn find_project_file_path(directory: &Path) -> std::io::Result<Option<PathBuf>> {
    // On wasm std::fs reports Unsupported rather than NotFound, which would turn every
    // lookup below into an error. There is no filesystem to hold a project file anyway.
    if cfg!(target_arch = "wasm32") {
        return Ok(None);
    }

    for directory in directory.ancestors() {
        let candidate = directory.join(FILE_NAME);
        if candidate.try_exists()? {
            return Ok(Some(candidate));
        }
    }

    Ok(None)
}

impl ProjectFile {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, Box<dyn Error>> {
        let source_path = normalize_project_file_path(path.as_ref());
        let mut source = fs::read(&source_path)?;
        blank_out_comments(&mut source);
        let data = if source.iter().all(u8::is_ascii_whitespace) {
            ProjectFileData::default()
        } else {
            serde_json::from_slice(&source)?
        };

        Ok(Self { source_path, data })
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

    pub fn source_path(&self) -> &Path {
        &self.source_path
    }

    pub fn library_paths(&self) -> Option<&HashMap<String, PathBuf>> {
        self.data.library_paths.as_ref()
    }

    pub fn include_paths(&self) -> Option<&Vec<PathBuf>> {
        self.data.include_paths.as_ref()
    }

    pub fn style(&self) -> Option<&str> {
        self.data.style.as_deref()
    }

    pub fn enable_experimental_features(&self) -> Option<bool> {
        self.data.enable_experimental_features
    }

    /// The `.slint` file to compile when a tool is given this project file.
    pub fn entry(&self) -> Option<PathBuf> {
        let project_directory =
            SourcePath::new(&self.source_path).parent().into_native_path().unwrap_or_default();
        self.data.entry.clone().map(|entry| resolve_relative_path(&project_directory, entry))
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
        let project_directory =
            SourcePath::new(&self.source_path).parent().into_native_path().unwrap_or_default();
        let mut data = self.data.clone();
        if let Some(include_paths) = &mut data.include_paths {
            for path in include_paths {
                *path = resolve_relative_path(&project_directory, path.clone());
            }
        }
        if let Some(library_paths) = &mut data.library_paths {
            for path in library_paths.values_mut() {
                *path = resolve_relative_path(&project_directory, path.clone());
            }
        }
        data.entry = self.entry();
        data
    }
}

impl ProjectFileData {
    pub fn apply_to(&self, compiler_config: &mut crate::CompilerConfiguration) {
        let Self {
            schema: _,
            library_paths,
            include_paths,
            style,
            enable_experimental_features,
            entry: _,
        } = self;
        if let Some(include_paths) = include_paths {
            compiler_config.include_paths = include_paths.clone();
        }
        if let Some(library_paths) = library_paths {
            compiler_config.library_paths = library_paths.clone();
        }
        if let Some(style) = style {
            compiler_config.style = Some(style.clone());
        }
        if let Some(enable_experimental_features) = enable_experimental_features {
            compiler_config.enable_experimental = *enable_experimental_features;
        }
    }
}

/// The settings that a caller set explicitly through an API, which win over the project file.
#[derive(Clone, Debug, Default)]
pub struct Overrides {
    pub project: ProjectFileData,
    pub embed_resources: Option<crate::EmbedResourcesKind>,
    pub const_scale_factor: Option<f32>,
    #[cfg(feature = "bundle-translations")]
    pub bundled_translations_path: Option<PathBuf>,
    pub default_translation_context: Option<crate::DefaultTranslationContext>,
    pub debug_info: Option<bool>,
    pub library_name: Option<String>,
    pub rust_module: Option<String>,
    #[cfg(all(feature = "renderer-software", feature = "sdf-fonts"))]
    pub use_sdf_fonts: Option<bool>,
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
            style,
            enable_experimental_features,
            entry,
        } = project;
        replace_if_supplied(&mut self.project.schema, schema);
        replace_if_supplied(&mut self.project.library_paths, library_paths);
        replace_if_supplied(&mut self.project.include_paths, include_paths);
        replace_if_supplied(&mut self.project.style, style);
        replace_if_supplied(
            &mut self.project.enable_experimental_features,
            enable_experimental_features,
        );
        replace_if_supplied(&mut self.project.entry, entry);
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

fn normalize_project_file_path(path: &Path) -> PathBuf {
    if crate::source_path::is_absolute(&path.to_string_lossy()) {
        crate::source_path::clean_path(path)
    } else {
        SourcePath::new(std::env::current_dir().ok().unwrap_or_default())
            .join(&path.to_string_lossy())
            .and_then(SourcePath::into_native_path)
            .unwrap_or_else(|| crate::source_path::clean_path(path))
    }
}

#[expect(deprecated)]
fn resolve_relative_path(project_directory: &Path, path: PathBuf) -> PathBuf {
    SourcePath::new(project_directory)
        .join(&path.to_string_lossy())
        .map(|path| path.to_legacy_path())
        .unwrap_or(path)
}

#[cfg(test)]
mod tests {
    use super::{FILE_NAME, ProjectFile, find_project_file_path, is_project_file, resolve_input};
    use crate::generator::OutputFormat;
    use std::{
        collections::HashMap,
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn build_overrides_apply_after_project_settings_and_existing_defaults() {
        with_project_file_contents(
            r#"{"include-paths":["include"],"library-paths":{"widgets":"lib"},"style":"fluent","enable-experimental-features":true}"#,
            |path| {
                let project = ProjectFile::load(path).unwrap();
                let mut config = crate::CompilerConfiguration::new(OutputFormat::Interpreter);
                config.debug_info = true;
                config.const_scale_factor = Some(2.);
                let overrides = super::Overrides {
                    project: super::ProjectFileData {
                        include_paths: Some(vec![]),
                        library_paths: Some(HashMap::new()),
                        style: Some(String::new()),
                        enable_experimental_features: Some(false),
                        ..Default::default()
                    },
                    debug_info: Some(false),
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
                style: Some("fluent".into()),
                enable_experimental_features: Some(true),
                ..Default::default()
            },
            debug_info: Some(true),
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
        assert_eq!(overrides.project.style.as_deref(), Some("fluent"));
        assert_eq!(overrides.project.enable_experimental_features, Some(false));
        assert_eq!(overrides.debug_info, Some(false));
    }

    #[test]
    fn partially_specified_project_file_is_valid() {
        let parsed = load_project_file(
            r#"{
                "style": "fluent",
                "enable-experimental-features": true
            }"#,
        )
        .unwrap();

        assert_eq!(parsed.style(), Some("fluent"));
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
                "$schema": "https://slint.dev/slint-project.schema.json",
                "style": "fluent"
            }"#,
        )
        .unwrap();

        assert_eq!(parsed.style(), Some("fluent"));
    }

    #[test]
    fn a_schema_reference_alone_is_accepted() {
        let parsed =
            load_project_file(r#"{"$schema": "https://slint.dev/slint-project.schema.json"}"#)
                .unwrap();

        assert_eq!(parsed.style(), None);
        assert_eq!(parsed.include_paths(), None);
    }

    #[test]
    fn line_comments_are_accepted() {
        let parsed = load_project_file(
            r#"{
                // The widget style for this project.
                "style": "fluent" // trailing comment
            }"#,
        )
        .unwrap();

        assert_eq!(parsed.style(), Some("fluent"));
    }

    #[test]
    fn block_comments_are_accepted() {
        let parsed = load_project_file(
            r#"{
                /* Disabled until the upgrade:
                   "style": "material",
                */
                "include-paths": ["include"] /* here too */
            }"#,
        )
        .unwrap();

        assert_eq!(parsed.style(), None);
        assert_eq!(parsed.include_paths(), Some(&vec![PathBuf::from("include")]));
    }

    #[test]
    fn comment_markers_inside_strings_are_kept() {
        let parsed = load_project_file(
            r#"{
                "include-paths": ["not//a/comment", "not/*a*/comment"],
                "style": "with \" quote // and slashes"
            }"#,
        )
        .unwrap();

        assert_eq!(
            parsed.include_paths(),
            Some(&vec![PathBuf::from("not//a/comment"), PathBuf::from("not/*a*/comment")])
        );
        assert_eq!(parsed.style(), Some(r#"with " quote // and slashes"#));
    }

    #[test]
    fn a_file_of_only_comments_is_empty() {
        let parsed = load_project_file(
            "// nothing set yet
/* not even this */
",
        )
        .unwrap();

        assert_eq!(parsed.style(), None);
        assert_eq!(parsed.include_paths(), None);
    }

    #[test]
    fn a_comment_keeps_the_line_of_a_later_error() {
        let error = load_project_file(
            r#"{
                // one
                // two
                "style": nonsense
            }"#,
        )
        .unwrap_err();

        // The blanked comments keep the offsets, so the error names line 4.
        assert!(error.to_string().contains("line 4"), "{error}");
    }

    #[test]
    fn unknown_settings_are_rejected() {
        let error = load_project_file(r#"{"unknown_setting": true}"#).unwrap_err();
        assert!(error.to_string().contains("unknown field"));
    }

    #[test]
    fn parse_empty() {
        let cases = ["", "{}", "{\n}", "  \t \n"];

        for case in cases {
            let parsed = load_project_file(case).unwrap();
            assert_eq!(parsed.library_paths(), None);
            assert_eq!(parsed.include_paths(), None);
            assert_eq!(parsed.style(), None);
            assert_eq!(parsed.enable_experimental_features(), None);
        }
    }

    #[test]
    fn stores_loaded_project_file_path() {
        with_project_file_contents("{}", |path| {
            let project = ProjectFile::load(path).unwrap();
            assert_eq!(project.source_path(), path);
        });
    }

    #[test]
    fn project_file_converts_to_compiler_configuration() {
        with_project_file_contents(
            r#"{
                "library-paths": {"widgets": "libraries/widgets.slint"},
                "include-paths": ["include", "../shared"],
                "style": "fluent",
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
                assert_eq!(compiler_config.style.as_deref(), Some("fluent"));
                assert!(compiler_config.enable_experimental);
            },
        );
    }

    #[test]
    fn project_file_conversion_preserves_defaults_for_omitted_settings() {
        with_project_file_contents("{}", |path| {
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
    fn the_nearest_project_file_is_found() {
        let root = unique_temp_file_path().parent().unwrap().to_path_buf();
        let nested = root.join("a/b");
        fs::create_dir_all(&nested).unwrap();

        fs::write(root.join(FILE_NAME), "{}").unwrap();
        assert_eq!(find_project_file_path(&nested).unwrap(), Some(root.join(FILE_NAME)));

        fs::write(root.join("a").join(FILE_NAME), "{}").unwrap();
        assert_eq!(find_project_file_path(&nested).unwrap(), Some(root.join("a").join(FILE_NAME)));

        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn the_entry_is_relative_to_the_project_file() {
        with_project_file_contents(r#"{ "entry": "ui/main.slint" }"#, |path| {
            let project = ProjectFile::load(path).unwrap();
            assert_eq!(project.entry(), Some(path.parent().unwrap().join("ui/main.slint")));
        });
    }

    #[test]
    fn a_project_file_input_resolves_to_its_entry() {
        with_project_file_contents(r#"{ "entry": "main.slint", "style": "material" }"#, |path| {
            let (slint_file, project) = resolve_input(path).unwrap();
            assert_eq!(slint_file, path.parent().unwrap().join("main.slint"));
            assert_eq!(project.unwrap().style(), Some("material"));
        });
    }

    #[test]
    fn a_project_file_input_without_an_entry_is_an_error() {
        with_project_file_contents("{}", |path| {
            let error = resolve_input(path).unwrap_err();
            assert!(error.contains("'entry'"), "{error}");
        });
    }

    #[test]
    fn a_project_file_input_ignores_a_nearer_project_file() {
        with_project_file_contents(r#"{ "entry": "ui/main.slint", "style": "outer" }"#, |path| {
            let ui = path.parent().unwrap().join("ui");
            fs::create_dir_all(&ui).unwrap();
            fs::write(ui.join(FILE_NAME), r#"{ "style": "inner" }"#).unwrap();

            let (_, project) = resolve_input(path).unwrap();

            fs::remove_dir_all(&ui).unwrap();
            assert_eq!(project.unwrap().style(), Some("outer"));
        });
    }

    #[test]
    fn a_slint_file_input_resolves_to_itself() {
        with_project_file_contents(r#"{ "entry": "other.slint" }"#, |path| {
            let main = path.parent().unwrap().join("main.slint");
            let (slint_file, project) = resolve_input(&main).unwrap();
            assert_eq!(slint_file, main);
            assert_eq!(project.unwrap().source_path(), path);
        });
    }

    #[test]
    fn only_the_exact_file_name_is_a_project_file() {
        assert!(is_project_file(Path::new("ui/slint-project.json")));
        assert!(!is_project_file(Path::new("ui/other.json")));
        assert!(!is_project_file(Path::new("ui/main.slint")));
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
