/* LICENSE BEGIN

    This file is part of the Sixty FPS Project

    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only

LICENSE END */
use anyhow::Context;
use anyhow::Result;
use lazy_static::lazy_static;
use std::str::FromStr;
use std::{path::Path, path::PathBuf, process::Command};
use structopt::StructOpt;

#[derive(Copy, Clone)]
struct LicenseTagStyle {
    tag_start: &'static str,
    line_prefix: &'static str,
    line_indentation: &'static str,
    tag_end: &'static str,
}

impl LicenseTagStyle {
    fn c_style_comment_style() -> Self {
        Self {
            tag_start: "/* LICENSE BEGIN\n",
            line_prefix: "",
            line_indentation: "    ",
            tag_end: "LICENSE END */\n",
        }
    }

    fn shell_comment_style() -> Self {
        Self {
            tag_start: "# LICENSE BEGIN\n",
            line_prefix: "#",
            line_indentation: " ",
            tag_end: "# LICENSE END\n",
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
        let location = match source.find(style.tag_start) {
            Some(start) => {
                let end = source[start..]
                    .find(style.tag_end)
                    .map(|idx| start + idx + style.tag_end.len())
                    .unwrap_or_default();
                if end > start {
                    Some(std::ops::Range { start, end })
                } else {
                    None
                }
            }
            None => None,
        };

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

    fn tag_matches(&self, expected_tag: &LicenseHeader) -> bool {
        let tag_loc = match &self.tag_location {
            Some(loc) => loc,
            None => return false,
        };

        let expected_tag_str = expected_tag.to_string(&self.tag_style);
        let found_tag = &self.source[tag_loc.start..tag_loc.end];
        expected_tag_str == found_tag
    }

    fn replace_tag(&self, replacement: &LicenseHeader) -> String {
        let new_header = replacement.to_string(&self.tag_style);

        match &self.tag_location {
            Some(loc) => {
                self.source[0..loc.start].to_string() + &new_header + &self.source[loc.end..]
            }
            None => return new_header + &self.source,
        }
    }
}

#[test]
fn test_license_tag_cstyle() {
    let style = LicenseTagStyle::c_style_comment_style();
    {
        let test_source = SourceFileWithTags::new(
            r#"/* LICENSE BEGIN
        foobar
        LICENSE END */

blah"#,
            &style,
        );
        assert_eq!(
            test_source.replace_tag(&LicenseHeader(&["TEST_LICENSE"])),
            r#"/* LICENSE BEGIN
    TEST_LICENSE
LICENSE END */

blah"#
                .to_string()
        );
    }
    {
        let test_source = SourceFileWithTags::new(r#"blah"#, &style);
        assert_eq!(
            test_source.replace_tag(&LicenseHeader(&["TEST_LICENSE"])),
            r#"/* LICENSE BEGIN
    TEST_LICENSE
LICENSE END */
blah"#
                .to_string()
        );
    }
}

#[test]
fn test_license_tag_hash() {
    let style = LicenseTagStyle::shell_comment_style();
    {
        let test_source = SourceFileWithTags::new(
            r#"# LICENSE BEGIN
# blub
# LICENSE END

blah"#,
            &style,
        );
        assert_eq!(
            test_source.replace_tag(&LicenseHeader(&["TEST_LICENSE"])),
            r#"# LICENSE BEGIN
# TEST_LICENSE
# LICENSE END

blah"#
                .to_string()
        );
    }
    {
        let test_source = SourceFileWithTags::new(r#"blah"#, &style);
        assert_eq!(
            test_source.replace_tag(&LicenseHeader(&["TEST_LICENSE"])),
            r#"# LICENSE BEGIN
# TEST_LICENSE
# LICENSE END
blah"#
                .to_string()
        );
    }
}

#[derive(Copy, Clone)]
enum LicenseLocation {
    Tag(LicenseTagStyle),
    Crate,
    NoLicense,
}

lazy_static! {
    static ref LICENSE_LOCATION_FOR_FILE: Vec<(regex::Regex, LicenseLocation)> = [
        ("^helper_crates/const-field-offset/src/lib.rs$".into(), LicenseLocation::NoLicense), // Upstream fork
        ("^helper_crates/const-field-offset/Cargo.toml$".into(), LicenseLocation::NoLicense), // Upstream fork
        (".+\\.rs$".into(), LicenseLocation::Tag(LicenseTagStyle::c_style_comment_style())),
        (".+\\.js$", LicenseLocation::Tag(LicenseTagStyle::c_style_comment_style())),
        (".+\\.mjs$", LicenseLocation::Tag(LicenseTagStyle::c_style_comment_style())),
        (".+\\.h$", LicenseLocation::Tag(LicenseTagStyle::c_style_comment_style())),
        (".+\\.cpp$", LicenseLocation::Tag(LicenseTagStyle::c_style_comment_style())),
        (".+\\.md$", LicenseLocation::NoLicense),
        (".+\\.png$", LicenseLocation::NoLicense),
        (".+\\.svg$", LicenseLocation::NoLicense),
        (".+\\.json$", LicenseLocation::NoLicense),
        (".+\\.html$", LicenseLocation::NoLicense),
        (".*\\.gitignore$", LicenseLocation::NoLicense),
        ("\\.clang-format$", LicenseLocation::NoLicense),
        ("\\.cargo/config$", LicenseLocation::Tag(LicenseTagStyle::shell_comment_style())),
        (
            "\\.github/workflows/rust.yaml$",
            LicenseLocation::Tag(LicenseTagStyle::shell_comment_style())
        ),
        ("^Cargo.toml$", LicenseLocation::NoLicense),
        (".+Cargo.toml$", LicenseLocation::Crate),
        (".+\\.toml$", LicenseLocation::NoLicense),
        (".*CMakeLists.txt$", LicenseLocation::Tag(LicenseTagStyle::shell_comment_style())),
        (".+\\.cmake.in$", LicenseLocation::Tag(LicenseTagStyle::shell_comment_style())),
        (".+\\.sh$", LicenseLocation::Tag(LicenseTagStyle::shell_comment_style())),
        (".+\\.60$", LicenseLocation::Tag(LicenseTagStyle::c_style_comment_style())),
        (".*README$", LicenseLocation::NoLicense),
        (".*README\\.txt$", LicenseLocation::NoLicense),
        ("LICENSE\\.GPL3$", LicenseLocation::NoLicense),
    ]
    .iter()
    .map(|(re, ty)| (regex::Regex::new(re).unwrap(), *ty))
    .collect();
}

pub struct LicenseHeader<'a>(&'a [&'a str]);

impl<'a> LicenseHeader<'a> {
    fn to_string(&self, style: &LicenseTagStyle) -> String {
        let mut result = style.tag_start.to_string();
        for line in self.0 {
            result += style.line_prefix;
            if !line.is_empty() {
                result += style.line_indentation;
            }
            result += line;
            result += "\n";
        }
        result += style.tag_end;
        result
    }
}

const EXPECTED_SPDX_EXPRESSION: &str = "GPL-3.0-only";
const EXPECTED_SPDX_ID: &str = "SPDX-License-Identifier: GPL-3.0-only";

const EXPECTED_HEADER: LicenseHeader<'static> = LicenseHeader(&[
    "",
    "This file is part of the Sixty FPS Project",
    "",
    "Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>",
    "Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>",
    "",
    EXPECTED_SPDX_ID,
    "",
]);

fn run_command(program: &str, args: &[&str]) -> Result<Vec<u8>> {
    let cmdline = || format!("{} {}", program, args.join(" "));
    let output = Command::new(program)
        .args(args)
        .current_dir(super::cmake::root_dir()?)
        .output()
        .with_context(|| format!("Error launching {}", cmdline()))?;
    let code =
        output.status.code().with_context(|| format!("Command received signal: {}", cmdline()))?;
    if code != 0 {
        Err(anyhow::anyhow!("Command {} exited with non-zero status: {}", cmdline(), code))
    } else {
        Ok(output.stdout)
    }
}

fn collect_files() -> Result<Vec<PathBuf>> {
    let root = super::cmake::root_dir()?;
    let ls_files_output = run_command("git", &["ls-files", "-z"])?;
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

        files.push(root.join(path));
    }
    Ok(files)
}

#[derive(Debug, StructOpt)]
pub struct LicenseHeaderCheck {
    #[structopt(long)]
    fixit: bool,

    #[structopt(long)]
    show_all: bool,

    #[structopt(long)]
    verbose: bool,
}

impl LicenseHeaderCheck {
    pub fn check_license_headers(&self) -> Result<(), Box<dyn std::error::Error>> {
        for path in &collect_files()? {
            let result = self
                .check_file(path.as_path())
                .with_context(|| format!("checking {}", &path.to_string_lossy()));

            if result.is_err() {
                if self.show_all {
                    eprintln!("Error: {:?}", result);
                } else {
                    return result.map_err(|e| e.into());
                }
            }
        }
        Ok(())
    }

    fn check_file_tags(&self, path: &Path, style: &LicenseTagStyle) -> Result<()> {
        let source = &std::fs::read_to_string(path).context("Error reading file")?;

        let source = SourceFileWithTags::new(source, style);

        if !source.has_tag() {
            if self.fixit {
                eprintln!(
                    "Fixing up {} as instructed. It's missing a license header.",
                    path.to_str().unwrap()
                );
                let source = source.replace_tag(&EXPECTED_HEADER);
                std::fs::write(path, &source).context("Error writing source")
            } else {
                Err(anyhow::anyhow!("Missing tag"))
            }
        } else if source.tag_matches(&EXPECTED_HEADER) {
            Ok(())
        } else {
            if self.fixit {
                let source = source.replace_tag(&EXPECTED_HEADER);
                std::fs::write(path, &source).context("Error writing new source")
            } else {
                Err(anyhow::anyhow!(
                    "unexpected header.\nexpected: {}\nfound: {}",
                    EXPECTED_HEADER.to_string(style),
                    source.found_tag()
                ))
            }
        }
    }

    fn check_crate_license(&self, path: &Path) -> Result<()> {
        use cargo_toml2::{from_path, CargoToml};

        let toml: CargoToml = from_path(path).context("Failed to read Cargo.toml")?;

        match toml.package.license {
            Some(license) => {
                if license == EXPECTED_SPDX_EXPRESSION {
                    Ok(())
                } else {
                    Err(anyhow::anyhow!(
                        "Incorrect license. Found {} expected {}",
                        license,
                        EXPECTED_SPDX_EXPRESSION
                    ))
                }
            }
            None => Err(anyhow::anyhow!("Missing license field")),
        }
    }

    fn check_file(&self, path: &Path) -> Result<()> {
        let repo_relative_path = path.strip_prefix(super::cmake::root_dir()?)?;
        let path_str = repo_relative_path.to_str().unwrap();
        let location = LICENSE_LOCATION_FOR_FILE
            .iter()
            .find_map(|(regex, style)| if regex.is_match(path_str) { Some(style) } else { None })
            .with_context(|| format!("Cannot determine the expected license header style. Please the license checking xtask."))?;

        match location {
            LicenseLocation::Tag(tag_style) => self.check_file_tags(path, tag_style),
            LicenseLocation::Crate => self.check_crate_license(path),
            LicenseLocation::NoLicense => {
                if self.verbose {
                    println!("Skipping {} as configured", path_str);
                }
                Ok(())
            }
        }
    }
}
