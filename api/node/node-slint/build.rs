// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Locate libnode and compile the small C++ embed shim.
//!
//! Search order:
//!   1. `NODE_DIR` env var pointing at an installation
//!      (`<dir>/include/node/node.h`, `<dir>/lib/{libnode.so,libnode*.a}`).
//!   2. pkg-config `libnode` (Debian `libnode-dev`).
//!
//! No automatic download — the libnode build takes ~15 min, so we ask
//! the user to run `./build-libnode.sh` and set `NODE_DIR` instead.

use std::env;
use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=src/embed.cpp");
    println!("cargo:rerun-if-changed=src/embed.h");
    println!("cargo:rerun-if-env-changed=NODE_DIR");

    let (include_dirs, link_kind) = locate_libnode();

    let mut build = cc::Build::new();
    build.cpp(true).std("c++17").file("src/embed.cpp");
    for dir in &include_dirs {
        build.include(dir);
    }
    build.compile("node_slint_embed");

    match link_kind {
        LinkKind::Shared { search_dirs } => {
            for dir in &search_dirs {
                println!("cargo:rustc-link-search=native={}", dir.display());
            }
            println!("cargo:rustc-link-lib=dylib=node");
        }
        LinkKind::Static { archives } => {
            for archive in &archives {
                let parent = archive.parent().expect("archive has no parent dir");
                let stem = archive
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .expect("archive name not utf-8");
                let name = stem.strip_prefix("lib").unwrap_or(stem);
                println!("cargo:rustc-link-search=native={}", parent.display());
                println!("cargo:rustc-link-lib=static={name}");
            }
            // libnode static needs: threads, dl, rt (linux), stdc++.
            if cfg!(target_os = "linux") {
                println!("cargo:rustc-link-lib=dylib=pthread");
                println!("cargo:rustc-link-lib=dylib=dl");
                println!("cargo:rustc-link-lib=dylib=rt");
                println!("cargo:rustc-link-lib=dylib=stdc++");
            } else if cfg!(target_os = "macos") {
                println!("cargo:rustc-link-lib=dylib=pthread");
                println!("cargo:rustc-link-lib=dylib=dl");
                println!("cargo:rustc-link-lib=dylib=c++");
            }
        }
    }

    // Export napi_register_module_v1 in the binary's dynamic symbol table
    // so `process.dlopen(mod, process.execPath)` can find it.
    if cfg!(target_os = "linux") {
        println!("cargo:rustc-link-arg=-rdynamic");
    } else if cfg!(target_os = "macos") {
        println!("cargo:rustc-link-arg=-Wl,-undefined,dynamic_lookup");
        println!("cargo:rustc-link-arg=-Wl,-export_dynamic");
    }
}

enum LinkKind {
    Shared { search_dirs: Vec<PathBuf> },
    Static { archives: Vec<PathBuf> },
}

fn locate_libnode() -> (Vec<PathBuf>, LinkKind) {
    if let Some(dir) = env::var_os("NODE_DIR") {
        let dir = PathBuf::from(dir);
        return from_node_dir(&dir).unwrap_or_else(|err| panic!("NODE_DIR={}: {err}", dir.display()));
    }

    if let Ok(lib) = pkg_config::Config::new()
        .atleast_version("18")
        .probe("libnode")
    {
        return (
            lib.include_paths.clone(),
            LinkKind::Shared {
                search_dirs: lib.link_paths,
            },
        );
    }

    panic!(
        "libnode not found.\n\
         Set NODE_DIR to point at a Node.js installation, or install libnode-dev.\n\
         To build libnode from source: ./build-libnode.sh --prefix <dir> && \
         NODE_DIR=<dir> cargo build -p node-slint",
    );
}

fn from_node_dir(dir: &Path) -> Result<(Vec<PathBuf>, LinkKind), String> {
    // Headers.
    let candidates = [dir.join("include/node"), dir.join("include"), dir.join("src")];
    let header = candidates
        .iter()
        .find(|p| p.join("node.h").exists())
        .ok_or_else(|| {
            format!(
                "cannot find node.h under {} (looked in include/node/, include/, src/)",
                dir.display()
            )
        })?;
    let mut include_dirs = vec![header.clone()];
    for sub in ["deps/v8/include", "deps/uv/include", "deps/cares/include"] {
        let p = dir.join(sub);
        if p.is_dir() {
            include_dirs.push(p);
        }
    }

    // Static archives first (preferred — self-contained binary).
    let lib_dir = dir.join("lib");
    if lib_dir.is_dir() {
        let archives: Vec<PathBuf> = std::fs::read_dir(&lib_dir)
            .map_err(|e| e.to_string())?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("a"))
            .collect();
        if !archives.is_empty() {
            return Ok((include_dirs, LinkKind::Static { archives }));
        }
    }

    // Shared library.
    let search_dirs: Vec<PathBuf> = ["lib", "out/Release/lib", "out/Release", "Release"]
        .iter()
        .map(|s| dir.join(s))
        .filter(|p| p.is_dir())
        .collect();
    if search_dirs.is_empty() {
        return Err(format!("no lib directories under {}", dir.display()));
    }
    Ok((include_dirs, LinkKind::Shared { search_dirs }))
}
