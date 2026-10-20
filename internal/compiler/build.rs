// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

fn main() -> std::io::Result<()> {
    println!("cargo:rustc-check-cfg=cfg(slint_debug_property)");

    let cargo_manifest_dir = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let library_dir = PathBuf::from("widgets");

    println!("cargo:rerun-if-changed={}", library_dir.display());

    let output_file_path = Path::new(&std::env::var_os("OUT_DIR").unwrap())
        .join(Path::new("included_library").with_extension("rs"));

    let mut file = BufWriter::new(std::fs::File::create(&output_file_path)?);
    write!(
        file,
        r#"
fn widget_library() -> &'static [(&'static str, &'static BuiltinDirectory<'static>)] {{
    &[
"#
    )?;

    // Sorted so the generated code, and with it the binaries, don't depend on
    // the file system's directory order.
    let mut styles: Vec<PathBuf> = cargo_manifest_dir
        .join(&library_dir)
        .read_dir()?
        .filter_map(Result::ok)
        .filter(|style| style.file_type().is_ok_and(|f| f.is_dir()))
        .map(|style| style.path())
        .collect();
    styles.sort();
    for path in styles {
        writeln!(
            file,
            "(\"{}\", &[{}]),",
            path.file_name().unwrap().to_string_lossy(),
            process_style(&cargo_manifest_dir, &path)?
        )?;
    }

    writeln!(file, "]\n}}")?;
    file.flush()?;

    println!("cargo:rustc-env=SLINT_WIDGETS_LIBRARY={}", output_file_path.display());

    write_format_id(&cargo_manifest_dir)
}

/// The sources that define the types `llr::serialize` writes.
const SERIALIZED_SOURCES: &[&str] = &[
    "builtin_functions.rs",
    "embedded_resources.rs",
    "expression_types.rs",
    "langtype.rs",
    "llr/expression.rs",
    "llr/item_tree.rs",
    "llr/serialize.rs",
    "native_class.rs",
    "source_path.rs",
    "translations.rs",
    "typeregister.rs",
];

/// The builtin enums and structs, when building from the repository.
/// A published crate relies on its version instead.
const SERIALIZED_REPOSITORY_SOURCES: &[&str] =
    &["../common/bitmapfont.rs", "../common/enums.rs", "../common/builtin_structs.rs"];

/// Computes `llr::serialize::FORMAT_ID`: a hash of the version and of [`SERIALIZED_SOURCES`]
/// and [`SERIALIZED_REPOSITORY_SOURCES`].
fn write_format_id(cargo_manifest_dir: &Path) -> std::io::Result<()> {
    // FNV-1a, which is stable across Rust versions, unlike `DefaultHasher`
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut write = |bytes: &[u8]| {
        for byte in bytes {
            hash = (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3);
        }
    };
    write(std::env::var("CARGO_PKG_VERSION").unwrap().as_bytes());
    for source in SERIALIZED_SOURCES {
        let path = cargo_manifest_dir.join(source);
        println!("cargo:rerun-if-changed={}", path.display());
        write(&std::fs::read(&path)?);
    }
    for source in SERIALIZED_REPOSITORY_SOURCES {
        let path = cargo_manifest_dir.join(source);
        if path.exists() {
            println!("cargo:rerun-if-changed={}", path.display());
            write(&std::fs::read(&path)?);
        }
    }
    let out = Path::new(&std::env::var_os("OUT_DIR").unwrap()).join("format_id.rs");
    std::fs::write(out, format!("{hash}u64"))
}

fn process_style(cargo_manifest_dir: &Path, path: &Path) -> std::io::Result<String> {
    let mut library_files: Vec<PathBuf> = cargo_manifest_dir
        .join(path)
        .read_dir()?
        .filter_map(Result::ok)
        .filter(|entry| {
            entry.file_type().is_ok_and(|f| !f.is_dir())
                && entry
                    .path()
                    .extension()
                    .map(|ext| {
                        ext == std::ffi::OsStr::new("slint")
                            || ext == std::ffi::OsStr::new("60")
                            || ext == std::ffi::OsStr::new("svg")
                            || ext == std::ffi::OsStr::new("svgz")
                    })
                    .unwrap_or_default()
        })
        .map(|entry| entry.path())
        .collect();
    library_files.sort();

    Ok(library_files
        .iter()
        .map(|file| {
            format!(
                "&BuiltinFile {{path: r#\"{}\"# , contents: include_bytes!(concat!(env!(\"CARGO_MANIFEST_DIR\"), r#\"/{}\"#))}}",
                file.file_name().unwrap().to_string_lossy(),
                file.strip_prefix(cargo_manifest_dir).unwrap().display()
            )
        })
        .collect::<Vec<_>>()
        .join(","))
}
