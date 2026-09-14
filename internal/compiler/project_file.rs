// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use serde::Deserialize;
use std::{
    collections::HashMap,
    error::Error,
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
struct ProjectFileData {
    /// The schema that editors validate the file against. The compiler ignores it.
    #[serde(rename = "$schema")]
    schema: Option<String>,

    library_paths: Option<HashMap<String, PathBuf>>,

    include_paths: Option<Vec<PathBuf>>,

    style: Option<String>,

    enable_experimental_features: Option<bool>,
}

#[derive(Clone, Debug, Default)]
pub struct ProjectFile {
    source_path: PathBuf,
    data: ProjectFileData,
}

pub const FILE_NAME: &str = "slint-project.json";

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
        let project_directory = crate::pathutils::dirname(&self.source_path);

        if let Some(include_paths) = &self.data.include_paths {
            compiler_config.include_paths = include_paths
                .iter()
                .cloned()
                .map(|path| resolve_relative_path(&project_directory, path))
                .collect();
        }

        if let Some(library_paths) = &self.data.library_paths {
            compiler_config.library_paths = library_paths
                .iter()
                .map(|(library_name, path)| {
                    (library_name.clone(), resolve_relative_path(&project_directory, path.clone()))
                })
                .collect();
        }

        if let Some(style) = &self.data.style {
            compiler_config.style = Some(style.clone());
        }

        if let Some(enable_experimental_features) = self.data.enable_experimental_features {
            compiler_config.enable_experimental = enable_experimental_features;
        }
    }
}

/// The settings that a caller set explicitly through an API, which win over the project file.
#[derive(Clone, Debug, Default)]
pub struct Overrides {
    pub include_paths: Option<Vec<PathBuf>>,
    pub library_paths: Option<HashMap<String, PathBuf>>,
    pub style: Option<String>,
}

impl Overrides {
    /// Applies the project file for the files in `directory` to `config`, then these overrides.
    /// Returns the project file that was applied.
    pub fn apply_with_project_file(
        &self,
        config: &mut crate::CompilerConfiguration,
        directory: &Path,
    ) -> Result<Option<ProjectFile>, String> {
        let project_file = ProjectFile::find(directory)?;
        if let Some(project_file) = &project_file {
            project_file.apply_to(config);
        }
        if let Some(include_paths) = &self.include_paths {
            config.include_paths = include_paths.clone();
        }
        if let Some(library_paths) = &self.library_paths {
            config.library_paths = library_paths.clone();
        }
        if let Some(style) = &self.style {
            config.style = Some(style.clone());
        }
        Ok(project_file)
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
    if crate::pathutils::is_absolute(path) {
        crate::pathutils::clean_path(path)
    } else {
        crate::pathutils::join(&std::env::current_dir().ok().unwrap_or_default(), path)
            .unwrap_or_else(|| crate::pathutils::clean_path(path))
    }
}

fn resolve_relative_path(project_directory: &Path, path: PathBuf) -> PathBuf {
    crate::pathutils::join(project_directory, &path).unwrap_or(path)
}

#[cfg(test)]
mod tests {
    use super::{FILE_NAME, ProjectFile, find_project_file_path};
    use crate::generator::OutputFormat;
    use std::{
        collections::HashMap,
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

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
