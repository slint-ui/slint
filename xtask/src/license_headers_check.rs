// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.0 OR LicenseRef-Slint-commercial

// cSpell: ignore datetime dotdot gettext

use anyhow::Context;
use anyhow::Result;
use lazy_static::lazy_static;
use std::str::FromStr;
use std::{path::Path, path::PathBuf};

#[derive(Copy, Clone, Debug)]
struct LicenseTagStyle {
    tag_start: &'static str,
    line_prefix: &'static str,
    line_indentation: &'static str,
    tag_end: &'static str,
}

impl LicenseTagStyle {
    fn c_style_comment_style() -> Self {
        Self {
            tag_start: "// Copyright © ",
            line_prefix: "//",
            line_indentation: " ",
            tag_end: const_format::concatcp!("// ", SPDX_LICENSE_LINE),
        }
    }

    fn shell_comment_style() -> Self {
        Self {
            tag_start: "# Copyright © ",
            line_prefix: "#",
            line_indentation: " ",
            tag_end: const_format::concatcp!("# ", SPDX_LICENSE_LINE),
        }
    }

    fn rst_comment_style() -> Self {
        Self {
            tag_start: ".. Copyright © ",
            line_prefix: "..",
            line_indentation: " ",
            tag_end: const_format::concatcp!(".. ", SPDX_LICENSE_LINE),
        }
    }
}

struct SourceFileWithTags<'a> {
    source: &'a str,
    tag_style: &'a LicenseTagStyle,
    tag_location: Option<std::ops::Range<usize>>,
}

impl<'a> SourceFileWithTags<'a> {
    fn new(source: &'a str, style: &'a LicenseTagStyle) -> Self {
        // This assumes that all LicenseTagStyle end with a SPDX license identifier line!
        let location = match source.find(style.tag_start) {
            Some(start) => {
                let end_tag = source[start..]
                    .find(style.tag_end)
                    .map(|idx| start + idx + style.tag_end.len())
                    .unwrap_or_default();
                if end_tag > start {
                    let end = source[end_tag..]
                        .find('\n')
                        .map(|idx| end_tag + idx + 1)
                        .unwrap_or(end_tag);
                    Some(std::ops::Range { start, end })
                } else {
                    None
                }
            }
            None => None,
        };

        // Find default gettext copyright statements
        let location = location.or_else(|| {
            let Some(start) = source.find("# SOME DESCRIPTIVE TITLE").or_else(|| source.find("# Copyright (C) ")) else { return None; };
            let end_line = "# This file is distributed under the same license as the ";
            let Some(end) = source[start..].find(end_line) else { return None; };
            let end = start + end + end_line.len();
            let Some(end_nl) = source[end..].find('\n') else { return None; };
            Some(std::ops::Range {start, end: end + end_nl + 1})
        });

        Self { source, tag_style: style, tag_location: location }
    }

    fn has_tag(&self) -> bool {
        self.tag_location.is_some()
    }

    fn found_tag(&self) -> &'a str {
        let tag_loc = match &self.tag_location {
            Some(loc) => loc,
            None => return "",
        };

        &self.source[tag_loc.start..tag_loc.end]
    }

    fn tag_matches(&self, expected_tag: &LicenseHeader, license: &str) -> bool {
        let tag_loc = match &self.tag_location {
            Some(loc) => loc,
            None => return false,
        };

        let expected_tag_str = expected_tag.to_string(self.tag_style, license);
        let found_tag = &self.source[tag_loc.start..tag_loc.end];
        expected_tag_str == found_tag
    }

    fn replace_tag(&self, replacement: &LicenseHeader, license: &str) -> String {
        let loc = &self.tag_location;
        let next_char = if let Some(range) = loc {
            self.source.as_bytes().get(range.end)
        } else {
            self.source.as_bytes().first()
        };

        let new_header = replacement.to_string(self.tag_style, license);
        let new_header = if next_char == Some(&b'\n') || next_char.is_none() {
            new_header
        } else {
            format!("{}\n", new_header)
        };

        match loc {
            Some(loc) => {
                self.source[0..loc.start].to_string() + &new_header + &self.source[loc.end..]
            }
            None => new_header + self.source,
        }
    }
}

#[test]
fn test_license_tag_c_style() {
    let style = LicenseTagStyle::c_style_comment_style();
    {
        let source = format!(
            r#"// Copyright © something <bar@something.com>
foobar
// SP{}-License-Identifier: {}
blah"#,
            "DX", EXPECTED_SPDX_EXPRESSION
        );
        let test_source = SourceFileWithTags::new(&source, &style);
        assert_eq!(
            test_source.replace_tag(&LicenseHeader(&["TEST_LICENSE"]), "foo"),
            r#"// TEST_LICENSE

blah"#
                .to_string()
        );
    }
    {
        let source = format!(
            r#"// Copyright © something <bar@something.com>
foobar
// SP{}-License-Identifier: {}

blah"#,
            "DX", EXPECTED_SPDX_EXPRESSION
        );
        let test_source = SourceFileWithTags::new(&source, &style);
        assert_eq!(
            test_source.replace_tag(&LicenseHeader(&["TEST_LICENSE"]), "bar"),
            r#"// TEST_LICENSE

blah"#
                .to_string()
        );
    }
    {
        let test_source = SourceFileWithTags::new("blah", &style);
        assert_eq!(
            test_source.replace_tag(&LicenseHeader(&["TEST_LICENSE"]), "bar"),
            r#"// TEST_LICENSE

blah"#
                .to_string()
        );
    }
    {
        let test_source = SourceFileWithTags::new("\nblah", &style);
        assert_eq!(
            test_source.replace_tag(&LicenseHeader(&[SPDX_LICENSE_LINE]), "bar"),
            String::from("// ")
                + SPDX_LICENSE_LINE
                + r#"bar

blah"#
        );
    }
    {
        let test_source = SourceFileWithTags::new("", &style);
        assert_eq!(
            test_source.replace_tag(&LicenseHeader(&["TEST_LICENSE"]), "bar"),
            r#"// TEST_LICENSE
"#
            .to_string()
        );
    }
}

#[test]
fn test_license_tag_hash() {
    let style = LicenseTagStyle::shell_comment_style();
    {
        let source = format!(
            r#"# Copyright © something <bar@something.com>
foobar
# SP{}-License-Identifier: {}

blah"#,
            "DX", EXPECTED_SPDX_EXPRESSION
        );
        let test_source = SourceFileWithTags::new(&source, &style);
        assert_eq!(
            test_source.replace_tag(&LicenseHeader(&["TEST_LICENSE"]), "bar"),
            r#"# TEST_LICENSE

blah"#
                .to_string()
        );
    }
    {
        let test_source = SourceFileWithTags::new(r#"blah"#, &style);
        assert_eq!(
            test_source.replace_tag(&LicenseHeader(&["TEST_LICENSE"]), "bar"),
            r#"# TEST_LICENSE

blah"#
                .to_string()
        );
    }
}

#[test]
fn test_license_tag_dotdot() {
    let style = LicenseTagStyle::rst_comment_style();
    {
        let source = format!(
            r#".. Copyright © something <bar@something.com>
foobar
.. SP{}-License-Identifier: {}

blah"#,
            "DX", EXPECTED_SPDX_EXPRESSION
        );
        let test_source = SourceFileWithTags::new(&source, &style);
        assert_eq!(
            test_source.replace_tag(&LicenseHeader(&["TEST_LICENSE"]), "bar"),
            r#".. TEST_LICENSE

blah"#
                .to_string()
        );
    }
    {
        let test_source = SourceFileWithTags::new(r#"blah"#, &style);
        assert_eq!(
            test_source.replace_tag(&LicenseHeader(&["TEST_LICENSE"]), "bar"),
            r#".. TEST_LICENSE

blah"#
                .to_string()
        );
    }
}

#[derive(Copy, Clone, Debug)]
enum LicenseLocation {
    Tag(LicenseTagStyle),
    Crate,
    NoLicense,
}

lazy_static! {
    // cspell:disable
    static ref LICENSE_LOCATION_FOR_FILE: Vec<(regex::Regex, LicenseLocation)> = [
        // full matches
        ("^\\.cargo/config$", LicenseLocation::Tag(LicenseTagStyle::shell_comment_style())),
        ("^\\.clang-format$", LicenseLocation::NoLicense),
        ("^\\.mailmap$", LicenseLocation::NoLicense),
        ("^\\.pre-commit-config\\.yaml$", LicenseLocation::NoLicense),
        ("^\\.reuse/dep5$", LicenseLocation::NoLicense), // .reuse files have no license headers
        ("^api/cpp/docs/Pipfile$", LicenseLocation::NoLicense),
        ("^api/cpp/docs/conf.py$", LicenseLocation::NoLicense),
        ("^docs/language/Pipfile$", LicenseLocation::NoLicense),
        ("^docs/language/conf.py$", LicenseLocation::NoLicense),
        ("^editors/tree-sitter-slint/binding.gyp$", LicenseLocation::NoLicense), // liberal license
        ("^editors/tree-sitter-slint/test-to-corpus.py$", LicenseLocation::Tag(LicenseTagStyle::shell_comment_style())),
        ("^Cargo.lock$", LicenseLocation::NoLicense),

        // filename based matches:
        ("(^|/)CMakeLists.txt$", LicenseLocation::Tag(LicenseTagStyle::shell_comment_style())),
        ("(^|/)Cargo.toml$", LicenseLocation::Crate),
        ("(^|/)Dockerfile", LicenseLocation::Tag(LicenseTagStyle::shell_comment_style())),
        ("(^|/)\\.eslintrc.yml$", LicenseLocation::NoLicense),
        ("(^|/)memory.x$", LicenseLocation::NoLicense), // third-party file
        ("(^|/)webpack\\..+\\.js$", LicenseLocation::NoLicense),

        // Path prefix matches:
        ("^editors/tree-sitter-slint/corpus/", LicenseLocation::NoLicense), // liberal license
        ("^api/cpp/docs/_static/", LicenseLocation::NoLicense),
        ("^api/cpp/docs/_templates/", LicenseLocation::NoLicense),
        ("^docs/tutorial/theme/", LicenseLocation::NoLicense),
        ("^editors/tree-sitter-slint/queries/", LicenseLocation::NoLicense), // liberal license
        ("^helper_crates/const-field-offset/", LicenseLocation::NoLicense), // liberal license
        ("^helper_crates/document-features/", LicenseLocation::NoLicense), // liberal license

        // directory based matches
        ("(^|/)LICENSES/", LicenseLocation::NoLicense),
        ("(^|/)LICENSE", LicenseLocation::NoLicense),
        ("(^|/)README$", LicenseLocation::NoLicense),

        // Extension matches:
        ("\\.60$", LicenseLocation::Tag(LicenseTagStyle::c_style_comment_style())),
        ("\\.60\\.disabled$", LicenseLocation::Tag(LicenseTagStyle::c_style_comment_style())),
        ("\\.cmake$", LicenseLocation::Tag(LicenseTagStyle::shell_comment_style())),
        ("\\.cmake.in$", LicenseLocation::Tag(LicenseTagStyle::shell_comment_style())),
        ("\\.cpp$", LicenseLocation::Tag(LicenseTagStyle::c_style_comment_style())),
        ("\\.css$", LicenseLocation::NoLicense),
        ("\\.gitattributes$", LicenseLocation::NoLicense),
        ("\\.gitignore$", LicenseLocation::NoLicense),
        ("\\.dockerignore$", LicenseLocation::NoLicense),
        ("\\.prettierignore$", LicenseLocation::NoLicense),
        ("\\.h$", LicenseLocation::Tag(LicenseTagStyle::c_style_comment_style())),
        ("\\.html$", LicenseLocation::NoLicense),
        ("\\.jpg$", LicenseLocation::NoLicense),
        ("\\.js$", LicenseLocation::Tag(LicenseTagStyle::c_style_comment_style())),
        ("\\.json$", LicenseLocation::NoLicense),
        ("\\.license$", LicenseLocation::NoLicense),
        ("\\.md$", LicenseLocation::NoLicense),
        ("\\.mjs$", LicenseLocation::Tag(LicenseTagStyle::c_style_comment_style())),
        ("\\.png$", LicenseLocation::NoLicense),
        ("\\.mo$", LicenseLocation::NoLicense),
        ("\\.po$", LicenseLocation::Tag(LicenseTagStyle::shell_comment_style())),
        ("\\.pot$", LicenseLocation::Tag(LicenseTagStyle::shell_comment_style())),
        ("\\.rs$", LicenseLocation::Tag(LicenseTagStyle::c_style_comment_style())),
        ("\\.rst$", LicenseLocation::Tag(LicenseTagStyle::rst_comment_style())),
        ("\\.sh$", LicenseLocation::Tag(LicenseTagStyle::shell_comment_style())),
        ("\\.slint$", LicenseLocation::Tag(LicenseTagStyle::c_style_comment_style())),
        ("\\.slint\\.disabled$", LicenseLocation::Tag(LicenseTagStyle::c_style_comment_style())),
        ("\\.sublime-commands$", LicenseLocation::NoLicense),
        ("\\.sublime-settings$", LicenseLocation::NoLicense),
        ("\\.sublime-syntax$", LicenseLocation::Tag(LicenseTagStyle::shell_comment_style())),
        ("\\.svg$", LicenseLocation::NoLicense),
        ("\\.tmPreferences$", LicenseLocation::NoLicense),
        ("\\.toml$", LicenseLocation::NoLicense),
        ("\\.ts$", LicenseLocation::Tag(LicenseTagStyle::c_style_comment_style())),
        ("\\.ttf$", LicenseLocation::NoLicense),
        ("\\.txt$", LicenseLocation::NoLicense),
        ("\\.ui$", LicenseLocation::NoLicense),
        ("\\.xml$", LicenseLocation::NoLicense),
        ("\\.yaml$", LicenseLocation::Tag(LicenseTagStyle::shell_comment_style())),
    ]
    .iter()
    .map(|(re, ty)| (regex::Regex::new(re).unwrap(), *ty))
    .collect();
}

lazy_static! {
    static ref LICENSE_FOR_FILE: Vec<(regex::Regex, &'static str)> = [
        ("^helper_crates/const-field-offset/", MIT_OR_APACHE2_LICENSE),
        ("^examples/", MIT_LICENSE),
        ("^docs/", MIT_LICENSE),
        ("^api/cpp/docs/", MIT_LICENSE),
        (".*", TRIPLE_LICENSE),
    ]
    .iter()
    .map(|(re, ty)| (regex::Regex::new(re).unwrap(), *ty))
    .collect();
    // cspell:enable
}

const TRIPLE_LICENSE: &str =
    "GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.0 OR LicenseRef-Slint-commercial";
const MIT_LICENSE: &str = "MIT";
const MIT_OR_APACHE2_LICENSE: &str = "MIT OR Apache-2.0";

pub struct LicenseHeader<'a>(&'a [&'a str]);

impl<'a> LicenseHeader<'a> {
    fn to_string(&self, style: &LicenseTagStyle, license: &str) -> String {
        let mut result = String::new();
        for line in self.0 {
            result += style.line_prefix;
            if !line.is_empty() {
                result += style.line_indentation;
            }
            result += line;
            if line == &SPDX_LICENSE_LINE {
                result.push_str(license);
            }
            result += "\n";
        }
        result
    }
}

#[cfg(test)]
const EXPECTED_SPDX_EXPRESSION: &str =
    "GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.0 OR LicenseRef-Slint-commercial";

const SPDX_LICENSE_ID: &str = const_format::concatcp!("SP", "DX-License-Identifier:"); // Do not confuse the reuse tool
const SPDX_LICENSE_LINE: &str = const_format::concatcp!(SPDX_LICENSE_ID, " "); // Do not confuse the reuse tool

const EXPECTED_HEADER: LicenseHeader<'static> =
    LicenseHeader(&["Copyright © SixtyFPS GmbH <info@slint.dev>", SPDX_LICENSE_LINE]);

const EXPECTED_HOMEPAGE: &str = "https://slint.dev";
const ALLOWED_HOMEPAGE: &str = "https://slint.rs";
const EXPECTED_REPOSITORY: &str = "https://github.com/slint-ui/slint";

fn collect_files() -> Result<Vec<PathBuf>> {
    let root = super::root_dir();
    let ls_files_output = super::run_command(
        "git",
        &["ls-files", "-z"],
        std::iter::empty::<(std::ffi::OsString, std::ffi::OsString)>(),
    )?
    .stdout;
    let mut files = Vec::new();
    for path in ls_files_output.split(|ch| *ch == 0) {
        if path.is_empty() {
            continue;
        }
        let path = PathBuf::from_str(
            std::str::from_utf8(path)
                .context("Error decoding git ls-files command output as utf-8")?,
        )
        .context("Failed to decide path output in git ls-files")?;

        if !path.is_dir() {
            files.push(root.join(path));
        }
    }
    Ok(files)
}

enum CargoDependency {
    Simple { _version: String },
    Full { path: String, version: String },
}

impl CargoDependency {
    fn new(encoded_value: &toml_edit::Value) -> Option<Self> {
        match encoded_value {
            toml_edit::Value::String(s) => {
                return Some(Self::Simple { _version: s.value().clone() })
            }
            toml_edit::Value::Float(_) => None,
            toml_edit::Value::Datetime(_) => None,
            toml_edit::Value::Boolean(_) => None,
            toml_edit::Value::Array(_) => None,
            toml_edit::Value::Integer(_) => None,
            toml_edit::Value::InlineTable(table) => {
                if let (Some(path), Some(version)) = (table.get("path"), table.get("version")) {
                    Some(Self::Full {
                        path: path.as_str().unwrap_or_default().to_owned(),
                        version: version.as_str().unwrap_or_default().to_owned(),
                    })
                } else {
                    None
                }
            }
        }
    }
}

struct CargoToml {
    path: std::path::PathBuf,
    doc: toml_edit::Document,
    edited: bool,
}

impl CargoToml {
    fn new(path: &Path) -> Result<Self> {
        let source = &std::fs::read_to_string(path).context("Error reading file")?;
        Ok(Self { doc: source.parse()?, edited: false, path: path.to_path_buf() })
    }

    fn is_workspace(&self) -> bool {
        self.doc.as_table().get("workspace").is_some()
    }

    fn package(&self) -> Result<&toml_edit::Table> {
        self.doc
            .as_table()
            .get("package")
            .and_then(|p| p.as_table())
            .ok_or_else(|| anyhow::anyhow!("Invalid Cargo.toml -- cannot find package section"))
    }

    fn dependencies(&self, dep_type: &str) -> Vec<(String, CargoDependency)> {
        match self.doc.as_table().get(dep_type).and_then(|d| d.as_table()) {
            Some(dep_table) => dep_table
                .iter()
                .filter_map(|(name, entry)| {
                    entry.as_value().and_then(|entry| {
                        CargoDependency::new(entry).map(|entry| (name.to_owned(), entry))
                    })
                })
                .collect(),
            None => Vec::new(),
        }
    }

    fn check_and_fix_package_string_field<'a>(
        &mut self,
        fix_it: bool,
        field: &'a str,
        expected_str: &'a str,
    ) -> Result<()> {
        match self.package()?.get(field) {
            Some(field_value) => {
                match field_value.as_str() {
                    Some(text) => {
                        if text != expected_str {
                            if fix_it {
                                eprintln!("Fixing up {:?} as instructed. It has unexpected data in {field}.", self.path);
                                self.doc["package"][field] = toml_edit::value(expected_str);
                                self.edited = true;
                            } else {
                                return Err(anyhow::anyhow!(
                                    "Incorrect {}. Found {} expected {}",
                                    field,
                                    text,
                                    expected_str
                                ));
                            }
                        }
                    }
                    None => return Err(anyhow::anyhow!("{} field is not a string", field)),
                }
            }
            None => {
                if fix_it {
                    eprintln!("Fixing up {:?} as instructed. It has no {field}.", self.path);
                    self.doc["package"][field] = toml_edit::value(expected_str);
                    self.edited = true;
                } else {
                    return Err(anyhow::anyhow!("Missing {} field", field));
                }
            }
        };
        Ok(())
    }

    fn published(&self) -> Result<bool> {
        Ok(self.package()?.get("publish").map(|v| v.as_bool().unwrap()).unwrap_or(true))
    }

    fn save_if_changed(&self) -> Result<()> {
        if self.edited {
            std::fs::write(&self.path, self.doc.to_string()).context("Error writing new Cargo.toml")
        } else {
            Ok(())
        }
    }
}

#[derive(Debug, clap::Parser)]
pub struct LicenseHeaderCheck {
    #[arg(long, action)]
    fix_it: bool,

    #[arg(long, action)]
    show_all: bool,

    #[arg(long, action)]
    verbose: bool,
}

impl LicenseHeaderCheck {
    pub fn check_license_headers(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut seen_errors = false;
        for path in &collect_files()? {
            let result = self
                .check_file(path.as_path())
                .with_context(|| format!("checking {}", &path.to_string_lossy()));

            if result.is_err() {
                seen_errors = true;
                if self.show_all {
                    eprintln!("Error: {:?}", result);
                } else {
                    return result.map_err(|e| e.into());
                }
            }
        }
        if seen_errors {
            Err(anyhow::anyhow!("Encountered one or multiple errors. See above for details.")
                .into())
        } else {
            println!("All files are ok.");
            Ok(())
        }
    }

    fn check_file_tags(&self, path: &Path, style: &LicenseTagStyle, license: &str) -> Result<()> {
        let source = &std::fs::read_to_string(path).context("Error reading file")?;

        let source = SourceFileWithTags::new(source, style);

        if !source.has_tag() {
            if self.fix_it {
                eprintln!("Fixing up {path:?} as instructed. It's missing a license header.");
                let source = source.replace_tag(&EXPECTED_HEADER, license);
                std::fs::write(path, source).context("Error writing source")
            } else {
                Err(anyhow::anyhow!("Missing tag"))
            }
        } else if source.tag_matches(&EXPECTED_HEADER, license) {
            Ok(())
        } else if self.fix_it {
            eprintln!("Fixing up {path:?} as instructed. It has a wrong license header.");
            let source = source.replace_tag(&EXPECTED_HEADER, license);
            std::fs::write(path, source).context("Error writing new source")
        } else {
            Err(anyhow::anyhow!(
                "unexpected header.\nexpected: {}\nfound: {}",
                EXPECTED_HEADER.to_string(style, license),
                source.found_tag()
            ))
        }
    }

    fn check_cargo_toml(&self, path: &Path, license: &str) -> Result<()> {
        let mut doc = CargoToml::new(path)?;

        if doc.is_workspace() {
            return Ok(());
        }

        doc.check_and_fix_package_string_field(self.fix_it, "license", license)?;

        if !doc.published()? {
            // Skip further tests for package that are not published
            return doc.save_if_changed();
        }

        if doc.check_and_fix_package_string_field(false, "homepage", ALLOWED_HOMEPAGE).is_err() {
            doc.check_and_fix_package_string_field(self.fix_it, "homepage", EXPECTED_HOMEPAGE)?;
        }
        doc.check_and_fix_package_string_field(self.fix_it, "repository", EXPECTED_REPOSITORY)?;

        if doc.package()?["description"].is_none() {
            return Err(anyhow::anyhow!("Missing description field"));
        }

        // Check that version of slint- dependencies are matching this version
        let expected_version = format!(
            "={}",
            doc.package()?.get("version").unwrap().as_value().unwrap().as_str().unwrap()
        );

        for (dep_name, dep) in doc
            .dependencies("dependencies")
            .iter()
            .chain(doc.dependencies("build-dependencies").iter())
        {
            if dep_name.starts_with("slint") {
                match dep {
                    CargoDependency::Simple { .. } => {
                        return Err(anyhow::anyhow!(
                            "slint package '{}' outside of the repository?",
                            dep_name
                        ))
                    }
                    CargoDependency::Full { path, version } => {
                        if path.is_empty() {
                            return Err(anyhow::anyhow!(
                                "slint package '{}' outside of the repository?",
                                dep_name
                            ));
                        }
                        if version != &expected_version {
                            return Err(anyhow::anyhow!(
                                "Version \"{}\" must be specified for dependency {}",
                                expected_version,
                                dep_name
                            ));
                        }
                    }
                }
            }
        }

        doc.save_if_changed()
    }

    fn check_file(&self, path: &Path) -> Result<()> {
        let repo_relative_path = path.strip_prefix(super::root_dir())?;
        let path_str = repo_relative_path.to_str().unwrap();
        let location = LICENSE_LOCATION_FOR_FILE
            .iter()
            .find_map(|(regex, style)| if regex.is_match(path_str) { Some(style) } else { None })
            .with_context(|| "Cannot determine the expected license header style. Please fix the license checking xtask.")?;

        let license = LICENSE_FOR_FILE
            .iter()
            .find_map(
                |(regex, license)| if regex.is_match(path_str) { Some(license) } else { None },
            )
            .with_context(|| {
                "Cannot determine the expected license. Please fix the license checking xtask."
            })?;

        match location {
            LicenseLocation::Tag(tag_style) => self.check_file_tags(path, tag_style, license),
            LicenseLocation::Crate => {
                self.check_file_tags(path, &LicenseTagStyle::shell_comment_style(), license)?;
                self.check_cargo_toml(path, license)
            }
            LicenseLocation::NoLicense => {
                if self.verbose {
                    println!("Skipping {} as configured", path_str);
                }
                Ok(())
            }
        }
    }
}
