// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::io::{BufWriter, Write};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let tests_file_path =
        std::path::Path::new(&std::env::var_os("OUT_DIR").unwrap()).join("test_functions.rs");
    let mut tests_file = BufWriter::new(std::fs::File::create(&tests_file_path)?);

    let prefix = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize()?;
    for entry in
        walkdir::WalkDir::new(&prefix).follow_links(false).into_iter().filter_entry(|entry| {
            !matches!(entry.file_name().to_str(), Some("target" | "dist" | "node_modules"))
        })
    {
        let entry = entry?;
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "md" && e != "mdx") {
            continue;
        }

        let file = std::fs::read_to_string(path)?;
        let file = file.replace('\r', ""); // Remove \r, because Windows.

        if !file.contains("```slint") {
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

        let mut lines = file.lines().enumerate();
        while let Some((n, opening)) = lines.next() {
            let trimmed = opening.trim_start();
            let Some(info) = trimmed.strip_prefix("```slint") else {
                continue;
            };
            // Permit `slint,no-preview` and `slint,no-auto-preview` but skip `slint,ignore` and others.
            if info == ",ignore" || info.contains("no-test") {
                continue;
            }

            // The fence can be indented (e.g. inside a list item); strip the
            // same indentation from the snippet lines.
            let indent = &opening[..opening.len() - trimmed.len()];
            let mut snippet_lines = Vec::new();
            loop {
                match lines.next() {
                    None => {
                        return Err(format!(
                            "Could not find the end of a code snippet in {}",
                            path.display()
                        )
                        .into());
                    }
                    Some((_, l)) if l.trim_start() == "```" => break,
                    Some((_, l)) => {
                        snippet_lines.push(l.strip_prefix(indent).unwrap_or(l.trim_start()));
                    }
                }
            }
            let snippet = snippet_lines.join("\n");

            if snippet.starts_with("{{#include") {
                // Skip non literal slint text
                continue;
            }

            write!(
                tests_file,
                r##"
    #[test]
    fn line_{}() {{
        crate::do_test("{}", "{}").unwrap();
    }}

                "##,
                n + 1,
                snippet.escape_default(),
                path.to_string_lossy().escape_default()
            )?;
        }
        writeln!(tests_file, "}}")?;
        println!("cargo:rerun-if-changed={}", path.display());
    }

    tests_file.flush()?;

    println!("cargo:rerun-if-changed=../../docs/astro/astro.config.mjs"); // This file is changed when new docs are added
    println!("cargo:rustc-env=TEST_FUNCTIONS={}", tests_file_path.to_string_lossy());
    println!("cargo:rustc-env=SLINT_ENABLE_EXPERIMENTAL_FEATURES=1");

    Ok(())
}
