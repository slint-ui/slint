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
#[serde(deny_unknown_fields)]
struct ProjectFileData {
    #[serde(alias = "library-paths")]
    library_paths: Option<HashMap<String, PathBuf>>,

    #[serde(alias = "include-directories")]
    include_directories: Option<Vec<PathBuf>>,

    style: Option<String>,

    #[serde(alias = "enable-experimental-features")]
    enable_experimental_features: Option<bool>,
}

#[derive(Clone, Debug, Default)]
pub struct ProjectFile {
    source_path: PathBuf,
    data: ProjectFileData,
}

pub const FILE_NAME: &str = "slint.project.json";

impl ProjectFile {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, Box<dyn Error>> {
        let source_path = normalize_project_file_path(path.as_ref());
        let source = fs::read(&source_path)?;
        let data = if source.iter().all(u8::is_ascii_whitespace) {
            ProjectFileData::default()
        } else {
            serde_json::from_slice(&source)?
        };

        Ok(Self { source_path, data })
    }

    pub fn source_path(&self) -> &Path {
        &self.source_path
    }

    pub fn library_paths(&self) -> Option<&HashMap<String, PathBuf>> {
        self.data.library_paths.as_ref()
    }

    pub fn include_directories(&self) -> Option<&Vec<PathBuf>> {
        self.data.include_directories.as_ref()
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
        let project_directory = crate::pathutils::dirname(&self.source_path);

        if let Some(include_directories) = &self.data.include_directories {
            compiler_config.include_paths = include_directories
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

        compiler_config
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
    use super::{FILE_NAME, ProjectFile};
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
                "enable_experimental_features": true
            }"#,
        )
        .unwrap();

        assert_eq!(parsed.style(), Some("fluent"));
        assert_eq!(parsed.enable_experimental_features(), Some(true));
        assert_eq!(parsed.library_paths(), None);
        assert_eq!(parsed.include_directories(), None);
    }

    #[test]
    fn kebab_case_project_file_keys_are_valid() {
        let parsed = load_project_file(
            r#"{
                "library-paths": {"widgets": "libs"},
                "include-directories": ["include"],
                "enable-experimental-features": true
            }"#,
        )
        .unwrap();

        assert_eq!(
            parsed.library_paths(),
            Some(&HashMap::from([("widgets".into(), PathBuf::from("libs"))]))
        );
        assert_eq!(parsed.include_directories(), Some(&vec![PathBuf::from("include")]));
        assert_eq!(parsed.enable_experimental_features(), Some(true));
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
            assert_eq!(parsed.include_directories(), None);
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
                "library_paths": {"widgets": "libraries/widgets.slint"},
                "include_directories": ["include", "../shared"],
                "style": "fluent",
                "enable_experimental_features": true
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
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        std::env::temp_dir().join(format!("slint-project-file-test-{stamp}")).join(FILE_NAME)
    }
}
