// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore datetime dotdot gettext

use anyhow::anyhow;
use anyhow::Context;
use anyhow::Result;
use lazy_static::lazy_static;
use std::cell::RefCell;
use std::str::FromStr;
use std::{path::Path, path::PathBuf};

#[derive(Copy, Clone, Debug)]
struct LicenseTagStyle {
    tag_start: &'static str,
    line_prefix: &'static str,
    line_indentation: &'static str,
    line_break: &'static str,
    tag_end: &'static str,
    overall_start: &'static str,
    overall_end: &'static str,
    is_real_end: bool,
}

impl LicenseTagStyle {
    fn c_style_comment_style() -> Self {
        Self {
            tag_start: "// Copyright © ",
            line_prefix: "//",
            line_indentation: " ",
            line_break: "\n",
            tag_end: const_format::concatcp!("// ", SPDX_LICENSE_LINE),
            overall_start: "",
            overall_end: "\n",
            is_real_end: false,
        }
    }

    fn shell_comment_style() -> Self {
        Self {
            tag_start: "# Copyright © ",
            line_prefix: "#",
            line_indentation: " ",
            line_break: "\n",
            tag_end: const_format::concatcp!("# ", SPDX_LICENSE_LINE),
            overall_start: "",
            overall_end: "\n",
            is_real_end: false,
        }
    }

    fn rst_comment_style() -> Self {
        Self {
            tag_start: ".. Copyright © ",
            line_prefix: "..",
            line_indentation: " ",
            line_break: "\n",
            tag_end: const_format::concatcp!(".. ", SPDX_LICENSE_LINE),
            overall_start: "",
            overall_end: "\n",
            is_real_end: false,
        }
    }

    fn html_comment_style() -> Self {
        Self {
            tag_start: "<!-- Copyright © ",
            line_prefix: " ",
            line_indentation: "",
            line_break: " ;",
            tag_end: " -->",
            overall_start: "<!--",
            overall_end: " -->\n",
            is_real_end: true,
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
                    if style.is_real_end {
                        if source.as_bytes()[end_tag] == b'\n' {
                            Some(std::ops::Range { start, end: end_tag + 1 })
                        } else {
                            Some(std::ops::Range { start, end: end_tag })
                        }
                    } else {
                        let end = source[end_tag..]
                            .find('\n')
                            .map(|idx| end_tag + idx + 1)
                            .unwrap_or(end_tag);
                        Some(std::ops::Range { start, end })
                    }
                } else {
                    None
                }
            }
            None => None,
        };

        // Find default gettext copyright statements
        let location = location.or_else(|| {
            let Some(start) =
                source.find("# SOME DESCRIPTIVE TITLE").or_else(|| source.find("# Copyright (C) "))
            else {
                return None;
            };
            let end_line = "# This file is distributed under the same license as the ";
            let Some(end) = source[start..].find(end_line) else {
                return None;
            };
            let end = start + end + end_line.len();
            let Some(end_nl) = source[end..].find('\n') else {
                return None;
            };
            Some(std::ops::Range { start, end: end + end_nl + 1 })
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

    fn has_license_header(&self, expected_tag: &LicenseHeader) -> bool {
        let tag_loc = match &self.tag_location {
            Some(loc) => loc,
            None => return false,
        };

        let found_tag = &self.source[tag_loc.start..tag_loc.end]
            .trim_start_matches(self.tag_style.overall_start)
            .trim_end_matches(self.tag_style.overall_end);
        let mut tag_entries = found_tag.split(self.tag_style.line_break);
        let Some(_copyright_entry) = tag_entries.next() else { return false };
        // Require _some_ license ...
        let Some(_) = tag_entries.next() else { return false };
        // ... as well as the SPDX license line at the start
        expected_tag.0 == SPDX_LICENSE_LINE
    }

    fn replace_tag(&self, replacement: &LicenseHeader, license: &str) -> String {
        let loc = &self.tag_location;
        let next_char = if let Some(range) = loc {
            self.source.as_bytes().get(range.end)
        } else {
            self.source.as_bytes().first()
        };

        let existing_copyright = loc.as_ref().and_then(|tag_loc| {
            self.source[tag_loc.start..tag_loc.end]
                .trim_start_matches(self.tag_style.overall_start)
                .trim_end_matches(self.tag_style.overall_end)
                .split(self.tag_style.line_break)
                .next()
        });

        let new_header = [
            self.tag_style.overall_start,
            &existing_copyright.map_or_else(
                || {
                    [
                        self.tag_style.line_prefix,
                        self.tag_style.line_indentation,
                        "Copyright © SixtyFPS GmbH <info@slint.dev>",
                    ]
                    .concat()
                },
                ToString::to_string,
            ),
            self.tag_style.line_break,
            &replacement.to_string(self.tag_style, license),
            self.tag_style.overall_end,
        ]
        .concat();
        let new_header = if next_char == Some(&b'\n') || next_char.is_none() {
            new_header
        } else {
            format!("{new_header}\n")
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
            test_source.replace_tag(&LicenseHeader("TEST_LICENSE"), "foo"),
            r#"// Copyright © something <bar@something.com>
// TEST_LICENSE

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
            test_source.replace_tag(&LicenseHeader("TEST_LICENSE"), "bar"),
            r#"// Copyright © something <bar@something.com>
// TEST_LICENSE

blah"#
                .to_string()
        );
    }
    {
        let test_source = SourceFileWithTags::new("blah", &style);
        assert_eq!(
            test_source.replace_tag(&LicenseHeader("TEST_LICENSE"), "bar"),
            r#"// Copyright © SixtyFPS GmbH <info@slint.dev>
// TEST_LICENSE

blah"#
                .to_string()
        );
    }
    {
        let test_source = SourceFileWithTags::new("\nblah", &style);
        assert_eq!(
            test_source.replace_tag(&LicenseHeader(SPDX_LICENSE_LINE), "bar"),
            String::from("// Copyright © SixtyFPS GmbH <info@slint.dev>\n// ")
                + SPDX_LICENSE_LINE
                + r#"bar

blah"#
        );
    }
    {
        let test_source = SourceFileWithTags::new("", &style);
        assert_eq!(
            test_source.replace_tag(&LicenseHeader("TEST_LICENSE"), "bar"),
            r#"// Copyright © SixtyFPS GmbH <info@slint.dev>
// TEST_LICENSE
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
            test_source.replace_tag(&LicenseHeader("TEST_LICENSE"), "bar"),
            r#"# Copyright © something <bar@something.com>
# TEST_LICENSE

blah"#
                .to_string()
        );
    }
    {
        let test_source = SourceFileWithTags::new(r#"blah"#, &style);
        assert_eq!(
            test_source.replace_tag(&LicenseHeader("TEST_LICENSE"), "bar"),
            r#"# Copyright © SixtyFPS GmbH <info@slint.dev>
# TEST_LICENSE

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
            test_source.replace_tag(&LicenseHeader("TEST_LICENSE"), "bar"),
            r#".. Copyright © something <bar@something.com>
.. TEST_LICENSE

blah"#
                .to_string()
        );
    }
    {
        let test_source = SourceFileWithTags::new(r#"blah"#, &style);
        assert_eq!(
            test_source.replace_tag(&LicenseHeader("TEST_LICENSE"), "bar"),
            r#".. Copyright © SixtyFPS GmbH <info@slint.dev>
.. TEST_LICENSE

blah"#
                .to_string()
        );
    }
}

#[test]
fn test_license_tag_html_style() {
    let style = LicenseTagStyle::html_comment_style();
    {
        let source = format!(
            r#"<!-- Copyright © something <bar@something.com> ; SP{}-License-Identifier: {} -->
blah"#,
            "DX", EXPECTED_SPDX_EXPRESSION
        );
        let test_source = SourceFileWithTags::new(&source, &style);
        assert_eq!(
            test_source.replace_tag(&LicenseHeader("TEST_LICENSE"), "foo"),
            r#"<!-- Copyright © something <bar@something.com> ; TEST_LICENSE -->

blah"#
                .to_string()
        );
    }
    {
        let source = format!(
            r#"<!-- Copyright © something <bar@something.com> ; SP{}-License-Identifier: {} -->

blah"#,
            "DX", EXPECTED_SPDX_EXPRESSION
        );
        let test_source = SourceFileWithTags::new(&source, &style);
        assert_eq!(
            test_source.replace_tag(&LicenseHeader("TEST_LICENSE"), "bar"),
            r#"<!-- Copyright © something <bar@something.com> ; TEST_LICENSE -->

blah"#
                .to_string()
        );
    }
    {
        let test_source = SourceFileWithTags::new("blah", &style);
        assert_eq!(
            test_source.replace_tag(&LicenseHeader("TEST_LICENSE"), "bar"),
            r#"<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; TEST_LICENSE -->

blah"#
                .to_string()
        );
    }
    {
        let test_source = SourceFileWithTags::new("\nblah", &style);
        assert_eq!(
            test_source.replace_tag(&LicenseHeader(SPDX_LICENSE_LINE), "bar"),
            String::from("<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; ")
                + SPDX_LICENSE_LINE
                + r#"bar -->

blah"#
        );
    }
    {
        let test_source = SourceFileWithTags::new("", &style);
        assert_eq!(
            test_source.replace_tag(&LicenseHeader("TEST_LICENSE"), "bar"),
            r#"<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; TEST_LICENSE -->
"#
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
        ("^\\.github/.*\\.md$", LicenseLocation::NoLicense),
        ("^\\.mailmap$", LicenseLocation::NoLicense),
        ("^\\.mise/tasks/", LicenseLocation::Tag(LicenseTagStyle::shell_comment_style())),
        ("^api/cpp/docs/conf\\.py$", LicenseLocation::NoLicense),
        ("^docs/reference/Pipfile$", LicenseLocation::NoLicense),
        ("^docs/reference/conf\\.py$", LicenseLocation::NoLicense),
        ("^editors/vscode/src/snippets\\.ts$", LicenseLocation::NoLicense), // liberal license
        ("^editors/vscode/tests/grammar/.*\\.slint$", LicenseLocation::NoLicense), // License header breaks these tests
        ("^editors/tree-sitter-slint/binding\\.gyp$", LicenseLocation::NoLicense), // liberal license
        ("^editors/tree-sitter-slint/test-to-corpus\\.py$", LicenseLocation::Tag(LicenseTagStyle::shell_comment_style())),
        ("^Cargo\\.lock$", LicenseLocation::NoLicense),
        ("^demos/printerdemo/zephyr/VERSION$", LicenseLocation::NoLicense),
        ("^examples/mcu-board-support/pico2_st7789/rp_pico2.rs$", LicenseLocation::NoLicense), // third-party file

        // filename based matches:
        ("(^|/)CMakeLists\\.txt$", LicenseLocation::Tag(LicenseTagStyle::shell_comment_style())),
        ("(^|/)Cargo\\.toml$", LicenseLocation::Crate),
        ("(^|/)Dockerfile", LicenseLocation::Tag(LicenseTagStyle::shell_comment_style())),
        ("(^|/)LICENSE$", LicenseLocation::NoLicense),
        ("(^|/)LICENSE\\.QT$", LicenseLocation::NoLicense),
        ("(^|/)README$", LicenseLocation::NoLicense),
        ("(^|/)\\.eslintrc\\.yml$", LicenseLocation::NoLicense),
        ("(^|/)memory\\.x$", LicenseLocation::NoLicense), // third-party file
        ("(^|/)webpack\\..+\\.js$", LicenseLocation::NoLicense),
        ("(^|/)partitions\\.csv$", LicenseLocation::NoLicense),
        ("(^|/)sdkconfig", LicenseLocation::NoLicense), // auto-generated
        ("(^|/)Pipfile$", LicenseLocation::Tag(LicenseTagStyle::shell_comment_style())),
        ("(^|/)\\.npmrc$", LicenseLocation::Tag(LicenseTagStyle::shell_comment_style())),
        ("(^|/)pnpm-lock\\.yaml$", LicenseLocation::NoLicense),
        ("(^|/)biome\\.json$", LicenseLocation::NoLicense),
        ("(^|/)package-lock\\.json$", LicenseLocation::NoLicense),
        ("(^|/)py.typed$", LicenseLocation::Tag(LicenseTagStyle::shell_comment_style())),

        // Path prefix matches:
        ("^editors/tree-sitter-slint/corpus/", LicenseLocation::NoLicense), // liberal license
        ("^api/cpp/docs/_static/", LicenseLocation::NoLicense),
        ("^api/cpp/docs/_templates/", LicenseLocation::NoLicense),
        ("^docs/quickstart/theme/", LicenseLocation::NoLicense),
        ("^editors/tree-sitter-slint/queries/", LicenseLocation::NoLicense), // liberal license

        // directory based matches
        ("(^|/)LICENSES/", LicenseLocation::NoLicense),

        // Extension matches:
        ("\\.60$", LicenseLocation::Tag(LicenseTagStyle::c_style_comment_style())),
        ("\\.60\\.disabled$", LicenseLocation::Tag(LicenseTagStyle::c_style_comment_style())),
        ("\\.astro$", LicenseLocation::Tag(LicenseTagStyle::c_style_comment_style())),
        ("\\.cmake$", LicenseLocation::Tag(LicenseTagStyle::shell_comment_style())),
        ("\\.cmake.in$", LicenseLocation::Tag(LicenseTagStyle::shell_comment_style())),
        ("\\.conf$", LicenseLocation::Tag(LicenseTagStyle::shell_comment_style())),
        ("\\.cpp$", LicenseLocation::Tag(LicenseTagStyle::c_style_comment_style())),
        ("\\.css$", LicenseLocation::NoLicense),
        ("\\.gitattributes$", LicenseLocation::NoLicense),
        ("\\.gitignore$", LicenseLocation::NoLicense),
        ("\\.vscodeignore$", LicenseLocation::NoLicense),
        ("\\.dockerignore$", LicenseLocation::NoLicense),
        ("\\.dockerignore$", LicenseLocation::NoLicense),
        ("\\.prettierignore$", LicenseLocation::NoLicense),
        ("\\.bazelignore$", LicenseLocation::NoLicense),
        ("\\.npmignore$", LicenseLocation::NoLicense),
        ("\\.h$", LicenseLocation::Tag(LicenseTagStyle::c_style_comment_style())),
        ("\\.html$", LicenseLocation::NoLicense),
        ("\\.java$", LicenseLocation::Tag(LicenseTagStyle::c_style_comment_style())),
        ("\\.jpg$", LicenseLocation::NoLicense),
        ("\\.js$", LicenseLocation::Tag(LicenseTagStyle::c_style_comment_style())),
        ("\\.json$", LicenseLocation::NoLicense),
        ("\\.jsonc$", LicenseLocation::NoLicense),
        ("\\.license$", LicenseLocation::NoLicense),
        ("\\.md$", LicenseLocation::Tag(LicenseTagStyle::html_comment_style())),
        ("\\.mdx$", LicenseLocation::Tag(LicenseTagStyle::html_comment_style())),
        ("\\.mjs$", LicenseLocation::Tag(LicenseTagStyle::c_style_comment_style())),
        ("\\.mts$", LicenseLocation::Tag(LicenseTagStyle::c_style_comment_style())),
        ("\\.hbs$", LicenseLocation::Tag(LicenseTagStyle::html_comment_style())),
        ("\\.overlay$", LicenseLocation::Tag(LicenseTagStyle::c_style_comment_style())),
        ("\\.pdf$", LicenseLocation::NoLicense),
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
        ("\\.tsx$", LicenseLocation::Tag(LicenseTagStyle::c_style_comment_style())),
        ("\\.ttf$", LicenseLocation::NoLicense),
        ("\\.txt$", LicenseLocation::NoLicense),
        ("\\.ui$", LicenseLocation::NoLicense),
        ("\\.webp$", LicenseLocation::NoLicense),
        ("\\.wgsl$", LicenseLocation::Tag(LicenseTagStyle::c_style_comment_style())),
        ("\\.xml$", LicenseLocation::NoLicense),
        ("\\.yaml$", LicenseLocation::Tag(LicenseTagStyle::shell_comment_style())),
        ("\\.yml$", LicenseLocation::Tag(LicenseTagStyle::shell_comment_style())),
        ("\\.py$", LicenseLocation::Tag(LicenseTagStyle::shell_comment_style())),
        ("\\.pyi$", LicenseLocation::Tag(LicenseTagStyle::shell_comment_style())),
        ("\\.proto$", LicenseLocation::Tag(LicenseTagStyle::c_style_comment_style())),
        ("\\.bazelrc$", LicenseLocation::Tag(LicenseTagStyle::shell_comment_style())),
        ("MODULE.bazel$", LicenseLocation::Tag(LicenseTagStyle::shell_comment_style())),
        ("BUILD.bazel$", LicenseLocation::Tag(LicenseTagStyle::shell_comment_style())),
        ("MODULE.bazel.lock$", LicenseLocation::NoLicense),
        ("\\.patch$", LicenseLocation::Tag(LicenseTagStyle::shell_comment_style())), // Doesn't really need a # prefix, but better than nothing
        ("\\.bazelversion$", LicenseLocation::NoLicense),
    ]
    .iter()
    .map(|(re, ty)| (regex::Regex::new(re).unwrap(), *ty))
    .collect();
}

lazy_static! {
    static ref LICENSE_FOR_FILE: Vec<(regex::Regex, &'static str)> = [
        ("^editors/tree-sitter-slint/grammar.js$", MIT_LICENSE),
        ("^helper_crates/const-field-offset/", MIT_OR_APACHE2_LICENSE),
        ("^helper_crates/vtable/", MIT_OR_APACHE2_LICENSE),
        ("^api/cpp/esp-idf/LICENSE$", TRIPLE_LICENSE),
        ("^examples/", MIT_LICENSE),
        ("^demos/", MIT_LICENSE),
        ("^docs/", MIT_LICENSE),
        ("^api/cpp/docs/", MIT_LICENSE),
        ("(^|/)(README|CONTRIBUTING|CHANGELOG|LICENSE)\\.md", TRIPLE_LICENSE),
        (".*\\.md$", MIT_LICENSE),
        (".*", TRIPLE_LICENSE),
    ]
    .iter()
    .map(|(re, ty)| (regex::Regex::new(re).unwrap(), *ty))
    .collect();
    // cspell:enable
}

const TRIPLE_LICENSE: &str =
    "GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0";
const MIT_LICENSE: &str = "MIT";
const MIT_OR_APACHE2_LICENSE: &str = "MIT OR Apache-2.0";

// This is really just the SPDX expression after the copyright line. The existence of the
// Copyright prefix is enforced by the tag scanning (tag_start).
pub struct LicenseHeader<'a>(&'a str);

impl LicenseHeader<'_> {
    fn to_string(&self, style: &LicenseTagStyle, license: &str) -> String {
        let mut result = [style.line_prefix, style.line_indentation, self.0].concat();

        if self.0 == SPDX_LICENSE_LINE {
            result.push_str(license);
        }

        result
    }
}

#[cfg(test)]
const EXPECTED_SPDX_EXPRESSION: &str =
    "GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0";

const SPDX_LICENSE_ID: &str = const_format::concatcp!("SP", "DX-License-Identifier:"); // Do not confuse the reuse tool
const SPDX_LICENSE_LINE: &str = const_format::concatcp!(SPDX_LICENSE_ID, " "); // Do not confuse the reuse tool

const EXPECTED_HEADER: LicenseHeader<'static> = LicenseHeader(SPDX_LICENSE_LINE);

const EXPECTED_HOMEPAGE: &str = "https://slint.dev";
const ALLOWED_HOMEPAGE: &str = "https://slint.rs";
const EXPECTED_REPOSITORY: &str = "https://github.com/slint-ui/slint";

fn collect_files() -> Result<Vec<PathBuf>> {
    let root = super::root_dir();

    let mut files = Vec::new();
    let (ls_files_output, split_char) = if root.join(".jj").exists() {
        (
            super::run_command(
                "jj",
                &["file", "list"],
                std::iter::empty::<(std::ffi::OsString, std::ffi::OsString)>(),
            )?
            .stdout,
            b'\n',
        )
    } else {
        (
            super::run_command(
                "git",
                &["ls-files", "-z"],
                std::iter::empty::<(std::ffi::OsString, std::ffi::OsString)>(),
            )?
            .stdout,
            b'\0',
        )
    };

    for path in ls_files_output.split(|ch| *ch == split_char) {
        if path.is_empty() {
            continue;
        }
        let path = PathBuf::from_str(
            std::str::from_utf8(path)
                .context("Error decoding file list command output from VCS as utf-8")?,
        )
        .context("Failed to decide path output in VCS file list")?;

        if !path.is_dir() {
            files.push(root.join(path));
        }
    }

    Ok(files)
}

#[derive(Debug)]
enum CargoDependency {
    Workspace,
    Full { path: String, version: String },
}

impl CargoDependency {
    fn new(encoded_value: &toml_edit::Value) -> Option<Self> {
        match encoded_value {
            toml_edit::Value::String(s) => {
                Some(Self::Full { version: s.value().clone(), path: String::new() })
            }
            toml_edit::Value::Float(_) => None,
            toml_edit::Value::Datetime(_) => None,
            toml_edit::Value::Boolean(_) => None,
            toml_edit::Value::Array(_) => None,
            toml_edit::Value::Integer(_) => None,
            toml_edit::Value::InlineTable(table) => {
                if table.get("workspace").is_some() {
                    Some(Self::Workspace)
                } else {
                    Some(Self::Full {
                        path: table.get("path").and_then(|x| x.as_str()).unwrap_or("").to_owned(),
                        version: table
                            .get("version")
                            .and_then(|x| x.as_str())
                            .unwrap_or("")
                            .to_owned(),
                    })
                }
            }
        }
    }
}

struct CargoToml {
    path: std::path::PathBuf,
    doc: toml_edit::DocumentMut,
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

    fn workspace_version(&self) -> Result<&str> {
        if !self.is_workspace() {
            Err(anyhow!("Not a workspace, can not extract version info."))
        } else {
            Ok(self
                .doc
                .as_table()
                .get("workspace")
                .unwrap()
                .get("package")
                .unwrap()
                .get("version")
                .unwrap()
                .as_str()
                .unwrap())
        }
    }

    fn package(&self) -> Result<&toml_edit::Table> {
        if self.is_workspace() {
            self.doc
                .as_table()
                .get("workspace")
                .and_then(|w| w.as_table())
                .and_then(|w| w.get("package"))
                .and_then(|p| p.as_table())
                .ok_or_else(|| anyhow!("Invalid Cargo.toml -- cannot find workspace package section in workspace file"))
        } else {
            self.doc
                .as_table()
                .get("package")
                .and_then(|p| p.as_table())
                .ok_or_else(|| anyhow!("Invalid Cargo.toml -- cannot find package section"))
        }
    }

    fn dependencies(&self, dep_type: &str) -> Vec<(String, CargoDependency)> {
        let table = if self.is_workspace() {
            self.doc
                .as_table()
                .get("workspace")
                .and_then(|w| w.as_table())
                .and_then(|w| w.get(dep_type))
                .and_then(|d| d.as_table())
        } else {
            self.doc.as_table().get(dep_type).and_then(|d| d.as_table())
        };
        match table {
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
        let is_workspace = self.is_workspace();
        match self.package()?.get(field) {
            Some(field_value) => {
                match field_value.get("workspace").and_then(|v| v.as_bool()) {
                    Some(true) if is_workspace => {
                        return Err(anyhow!(
                            "Using workspace {}.workspace = true in workspace",
                            field
                        ))
                    }
                    Some(true) => { /* nothing to do */ }
                    Some(false) => {
                        if fix_it {
                            eprintln!(
                                "Fixing up {:?} as instructed. It has {field}.workspace = false",
                                self.path
                            );
                            self.doc["package"][field]["workspace"] = toml_edit::value(true);
                            self.edited = true;
                        } else {
                            return Err(anyhow!(
                                "Incorrect {}.workspace found: expected true, found false",
                                field,
                            ));
                        }
                    }
                    None => match field_value.as_str() {
                        Some(text) => {
                            if text != expected_str {
                                if fix_it {
                                    eprintln!("Fixing up {:?} as instructed. It has unexpected data in {field}.", self.path);
                                    self.doc["package"][field] = toml_edit::value(expected_str);
                                    self.edited = true;
                                } else {
                                    return Err(anyhow!(
                                        "Incorrect {}. Found {} expected {}",
                                        field,
                                        text,
                                        expected_str
                                    ));
                                }
                            }
                        }
                        None => return Err(anyhow!("{} field is not a string", field)),
                    },
                };
            }
            None => {
                if fix_it {
                    eprintln!("Fixing up {:?} as instructed. It has no {field}.", self.path);
                    self.doc["package"][field] = toml_edit::value(expected_str);
                    self.edited = true;
                } else {
                    return Err(anyhow!("Missing {} field", field));
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

    #[arg(skip)]
    workspace_version: RefCell<String>,
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
                    eprintln!("Error: {result:?}");
                } else {
                    return result.map_err(|e| e.into());
                }
            }
        }
        if seen_errors {
            Err(anyhow!("Encountered one or multiple errors. See above for details.").into())
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
                Err(anyhow!("Missing tag"))
            }
        } else if source.has_license_header(&EXPECTED_HEADER) {
            Ok(())
        } else if self.fix_it {
            eprintln!("Fixing up {path:?} as instructed. It has a wrong license header.");
            let source = source.replace_tag(&EXPECTED_HEADER, license);
            std::fs::write(path, source).context("Error writing new source")
        } else {
            Err(anyhow!(
                "unexpected header.\nexpected: {}\nfound: {}",
                EXPECTED_HEADER.to_string(style, license),
                source.found_tag()
            ))
        }
    }

    fn check_cargo_toml(&self, path: &Path, license: &str) -> Result<()> {
        let mut doc = CargoToml::new(path)?;

        if doc.is_workspace() {
            let mut wv = self.workspace_version.borrow_mut();
            if &*wv == "" {
                *wv = doc.workspace_version()?.to_string();
            }
            let expected_version = wv.clone();

            // Check the workspace.dependencies:
            self.check_dependencies(&doc, &expected_version)?;

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
            return Err(anyhow!("Missing description field"));
        }

        // Check that version of slint- dependencies are matching this version
        let expected_version = {
            let wv = self.workspace_version.borrow().clone();
            doc.package()?
                .get("version")
                .and_then(|v| v.as_value())
                .and_then(|v| v.as_str())
                .unwrap_or(&wv)
                .to_string()
        };

        self.check_dependencies(&doc, &expected_version)?;

        doc.save_if_changed()
    }

    fn check_dependencies(&self, doc: &CargoToml, expected_version: &str) -> Result<()> {
        let expected_version = format!("={expected_version}");

        for (dep_name, dep) in doc
            .dependencies("dependencies")
            .iter()
            .chain(doc.dependencies("build-dependencies").iter())
        {
            if dep_name.starts_with("slint") || dep_name.starts_with("i-slint") {
                match dep {
                    CargoDependency::Workspace => (),
                    CargoDependency::Full { path, version } => {
                        if path.is_empty() {
                            return Err(anyhow!(
                                "slint package '{}' outside of the repository?",
                                dep_name
                            ));
                        }
                        if version != &expected_version {
                            return Err(anyhow!(
                                "Version \"{}\" must be specified for dependency {}",
                                expected_version,
                                dep_name
                            ));
                        }
                    }
                }
            }
        }

        for (dep_name, dep) in doc.dependencies("dev-dependencies").iter() {
            if dep_name.starts_with("slint") || dep_name.starts_with("i-slint") {
                match dep {
                    CargoDependency::Workspace => {
                        return Err(anyhow!(
                            "dev-dependencies cannot be from the workspace because version must be empty {dep_name}"
                        ));
                    }
                    CargoDependency::Full { path, version } => {
                        if path.is_empty() {
                            return Err(anyhow!(
                                "slint package '{}' outside of the repository?",
                                dep_name
                            ));
                        }
                        if !version.is_empty() {
                            return Err(anyhow!(
                                "dev-dependencies version must be empty for dependency {dep_name}"
                            ));
                        }
                    }
                }
            }
        }
        Ok(())
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
                    println!("Skipping {path_str} as configured");
                }
                Ok(())
            }
        }
    }
}
