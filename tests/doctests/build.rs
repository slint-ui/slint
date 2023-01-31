// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use std::io::Write;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let tests_file_path =
        std::path::Path::new(&std::env::var_os("OUT_DIR").unwrap()).join("test_functions.rs");
    let mut tests_file = std::fs::File::create(&tests_file_path)?;

    let prefix = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize()?;
    for entry in std::fs::read_dir(prefix.join("docs/langref/src"))?
        .chain(std::fs::read_dir(prefix.join("docs"))?)
        .chain(std::fs::read_dir(prefix.join("docs/recipes"))?)
    {
        let entry = entry?;
        let path = entry.path();
        if path.extension().map_or(true, |e| e != "md") {
            continue;
        }
        let stem = path
            .strip_prefix(&prefix)?
            .to_string_lossy()
            .replace(|c: char| !c.is_ascii_alphanumeric(), "_")
            .to_lowercase();

        writeln!(tests_file, "\nmod {} {{", stem)?;

        let file = std::fs::read_to_string(&path)?;
        let file = file.replace('\r', ""); // Remove \r, because Windows.
        let mut rest = file.as_str();
        let mut count = 0;
        const BEGIN_MARKER: &str = "\n```slint";
        while let Some(begin) = rest.find(BEGIN_MARKER) {
            rest = rest[begin..].strip_prefix(BEGIN_MARKER).unwrap();

            // Permit `slint,no-preview` and `slint,no-auto-preview` but skip `slint,ignore` and others.
            rest = match rest.split_once('\n') {
                Some((",no-preview", rest)) | Some((",no-auto-preview", rest)) => rest,
                Some(("", _)) => rest,
                _ => continue,
            };

            let end = rest.find("\n```\n").ok_or_else(|| {
                format!("Could not find the end of a code snippet in {}", path.display())
            })?;
            let snippet = &rest[..end];
            rest = &rest[end..];
            count += 1;
            write!(
                tests_file,
                r##"
    #[test]
    fn {}_{}() {{
        crate::do_test("{}").unwrap();
    }}

                "##,
                stem,
                count,
                snippet.escape_default(),
            )?;
        }
        writeln!(tests_file, "}}")?;
        println!("cargo:rerun-if-changed={}", path.display());
    }

    let cargo_manifest_dir = env!("CARGO_MANIFEST_DIR").replace('\\', "/");

    for entry in std::fs::read_dir(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/tutorial/rust/src"),
    )? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().map_or(true, |e| e != "rs") {
            continue;
        }
        let stem = path
            .file_stem()
            .unwrap()
            .to_string_lossy()
            .replace(|c: char| !c.is_ascii_alphanumeric(), "_");

        writeln!(
            tests_file,
            "\n#[cfg(test)]\n#[path = \"{}/../../docs/tutorial/rust/src/{}\"]\nmod {};",
            cargo_manifest_dir,
            path.file_name().unwrap().to_string_lossy(),
            stem
        )?;

        println!("cargo:rerun-if-changed={}", path.display());
    }

    println!("cargo:rustc-env=TEST_FUNCTIONS={}", tests_file_path.to_string_lossy());

    Ok(())
}
