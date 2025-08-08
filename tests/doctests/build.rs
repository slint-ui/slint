// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::io::{BufWriter, Write};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let tests_file_path =
        std::path::Path::new(&std::env::var_os("OUT_DIR").unwrap()).join("test_functions.rs");
    let mut tests_file = BufWriter::new(std::fs::File::create(&tests_file_path)?);

    let prefix = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize()?;
    for entry in walkdir::WalkDir::new(&prefix)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| entry.file_name() != "target")
    {
        let entry = entry?;
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "md" && e != "mdx") {
            continue;
        }

        let file = std::fs::read_to_string(path)?;
        let file = file.replace('\r', ""); // Remove \r, because Windows.

        const BEGIN_MARKER: &str = "\n```slint";
        if !file.contains(BEGIN_MARKER) {
            continue;
        }

        let stem = path
            .strip_prefix(&prefix)?
            .to_string_lossy()
            .replace('-', "ˍ")
            .replace(['/', '\\'], "Ⳇ")
            .replace(['.'], "ᐧ")
            .to_lowercase();

        writeln!(tests_file, "\nmod {stem} {{")?;

        let mut rest = file.as_str();
        let mut line = 1;

        while let Some(begin) = rest.find(BEGIN_MARKER) {
            line += rest[..begin].bytes().filter(|&c| c == b'\n').count() + 1;
            rest = rest[begin..].strip_prefix(BEGIN_MARKER).unwrap();

            // Permit `slint,no-preview` and `slint,no-auto-preview` but skip `slint,ignore` and others.
            rest = match rest.split_once('\n') {
                Some((",ignore", _)) => continue,
                Some((x, _)) if x.contains("no-test") => continue,
                Some((_, rest)) => rest,
                _ => continue,
            };

            let end = rest.find("\n```\n").ok_or_else(|| {
                format!("Could not find the end of a code snippet in {}", path.display())
            })?;
            let snippet = &rest[..end];

            if snippet.starts_with("{{#include") {
                // Skip non literal slint text
                continue;
            }

            rest = &rest[end..];

            write!(
                tests_file,
                r##"
    #[test]
    fn line_{}() {{
        crate::do_test("{}", "{}").unwrap();
    }}

                "##,
                line,
                snippet.escape_default(),
                path.to_string_lossy().escape_default()
            )?;

            line += snippet.bytes().filter(|&c| c == b'\n').count() + 1;
        }
        writeln!(tests_file, "}}")?;
        println!("cargo:rerun-if-changed={}", path.display());
    }

    tests_file.flush()?;

    println!("cargo:rustc-env=TEST_FUNCTIONS={}", tests_file_path.to_string_lossy());
    println!("cargo:rustc-env=SLINT_ENABLE_EXPERIMENTAL_FEATURES=1");

    Ok(())
}
