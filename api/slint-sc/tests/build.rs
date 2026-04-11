// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::io::{BufWriter, Write};
use std::path::PathBuf;

fn main() -> std::io::Result<()> {
    let manifest_dir = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let cases_dir = manifest_dir.join("cases");
    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());

    println!("cargo:rerun-if-changed={}", cases_dir.display());

    let mut generated =
        BufWriter::new(std::fs::File::create(out_dir.join("generated.rs"))?);

    let mut entries: Vec<PathBuf> = std::fs::read_dir(&cases_dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("slint"))
        .collect();
    entries.sort();

    for path in entries {
        println!("cargo:rerun-if-changed={}", path.display());
        let stem = path.file_stem().unwrap().to_str().unwrap().to_owned();
        let source = std::fs::read_to_string(&path)?;

        let cfg = slint_build::CompilerConfiguration::new()
            .with_safety_critical(true)
            .embed_resources(slint_build::EmbedResourcesKind::EmbedForSoftwareRenderer);
        let generated_rs = out_dir.join(format!("{stem}.rs"));
        slint_build::compile_with_output_path(&path, &generated_rs, cfg)
            .unwrap_or_else(|e| panic!("failed to compile {}: {e:?}", path.display()));

        writeln!(generated, "#[cfg(test)] pub mod r#{stem} {{")?;
        writeln!(generated, "    #[allow(unused_imports)] use slint_sc::*;")?;
        writeln!(generated, "    include!(concat!(env!(\"OUT_DIR\"), \"/{stem}.rs\"));")?;
        for (i, test) in test_driver_lib::extract_test_functions(&source)
            .filter(|t| t.language_id == "rust")
            .enumerate()
        {
            writeln!(generated, "    #[test] fn t_{i}() {{")?;
            writeln!(generated, "        {}", test.source.replace('\n', "\n        "))?;
            writeln!(generated, "    }}")?;
        }
        writeln!(generated, "}}")?;
    }

    generated.flush()?;
    Ok(())
}
