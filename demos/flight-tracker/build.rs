// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use std::collections::HashMap;
use std::hash::{BuildHasher, RandomState};
use std::path::{Path, PathBuf};
use std::process::Command;

#[path = "gibs.rs"]
mod gibs;
use gibs::Kind;

/// Says whether to embed the map tiles: `0` or `off` for never, `1` or `on` for always, or the
/// directory of tiles to embed, without fetching any. Unset, cross-compiled builds embed them.
const EMBED: &str = "FLIGHT_TRACKER_EMBED_TILES";

fn main() {
    slint_build::compile("ui/app.slint").unwrap();
    println!("cargo::rustc-check-cfg=cfg(embedded_tiles)");
    println!("cargo::rerun-if-env-changed={EMBED}");
    embed_tiles();
}

/// Puts the map tiles into the binary, which the demo then shows instead of fetching them, as
/// the cfg `embedded_tiles` tells it. A cross-compiled build is likely for a device without a
/// network; the web build fetches them, to stay small.
fn embed_tiles() {
    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("wasm32") {
        return;
    }
    let setting = std::env::var(EMBED).unwrap_or_default();
    let (directory, fetch) = match setting.as_str() {
        "0" | "off" => return,
        "" if std::env::var("TARGET") == std::env::var("HOST") => return,
        "" | "1" | "on" => {
            // The cache of the demo itself, which a native build fills while it runs.
            let Some(directories) =
                directories::ProjectDirs::from("dev", "slint", "flight-tracker")
            else {
                println!("cargo::warning=No cache directory to fetch the map tiles into");
                return;
            };
            (directories.cache_dir().join("tiles"), true)
        }
        directory => (PathBuf::from(directory), false),
    };

    let tiles: Vec<(Kind, u32, u32, u32)> = [Kind::Day, Kind::Night]
        .into_iter()
        .flat_map(|kind| {
            (gibs::FIRST_ZOOM..gibs::FIRST_ZOOM + gibs::LEVELS as u32).flat_map(move |zoom| {
                let columns = gibs::columns(zoom);
                (0..columns / 2)
                    .flat_map(move |row| (0..columns).map(move |column| (kind, zoom, row, column)))
            })
        })
        .collect();
    if fetch {
        fetch_missing(&directory, &tiles);
    } else {
        println!("cargo::rerun-if-changed={}", directory.display());
    }

    // The tiles one after the other, each kind of content once: most of the night is black.
    let mut data: Vec<u8> = Vec::new();
    let mut index = String::new();
    let mut places: HashMap<u64, (usize, usize)> = HashMap::new();
    let hasher = RandomState::new();
    let mut embedded = 0;
    for &(kind, zoom, row, column) in &tiles {
        let Ok(file) = std::fs::read(directory.join(kind.path(zoom, row, column))) else {
            continue;
        };
        let place = *places.entry(hasher.hash_one(&file)).or_insert((data.len(), file.len()));
        let (offset, length) = if data.get(place.0..place.0 + place.1) == Some(&file[..]) {
            place
        } else {
            data.extend_from_slice(&file);
            (data.len() - file.len(), file.len())
        };
        index += &format!("({}, {zoom}, {row}, {column}, {offset}, {length}),\n", kind as u8);
        embedded += 1;
    }
    if embedded < tiles.len() {
        println!(
            "cargo::warning={} of {} map tiles in {} to embed",
            embedded,
            tiles.len(),
            directory.display()
        );
        if fetch {
            // A path that doesn't exist makes cargo run this again, to fetch the rest.
            println!("cargo::rerun-if-changed={}", directory.join("incomplete").display());
        }
    }
    if embedded == 0 {
        return;
    }

    let out = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    std::fs::write(out.join("embedded_tiles.bin"), data).unwrap();
    std::fs::write(
        out.join("embedded_tiles.rs"),
        format!(
            "/// The kind, zoom, row, and column of each tile, in that order, and its offset and
/// length in `DATA`.
pub static INDEX: &[(u8, u8, u8, u8, u32, u32)] = &[\n{index}];
pub static DATA: &[u8] = include_bytes!(concat!(env!(\"OUT_DIR\"), \"/embedded_tiles.bin\"));\n"
        ),
    )
    .unwrap();
    println!("cargo::rustc-cfg=embedded_tiles");
}

/// Fetches the `tiles` that `directory` doesn't have yet from NASA's GIBS, with curl.
fn fetch_missing(directory: &Path, tiles: &[(Kind, u32, u32, u32)]) {
    let missing: Vec<(String, PathBuf)> = tiles
        .iter()
        .map(|&(kind, zoom, row, column)| {
            (kind.url(zoom, row, column), directory.join(kind.path(zoom, row, column)))
        })
        .filter(|(_, path)| !path.exists())
        .collect();
    if missing.is_empty() {
        return;
    }
    println!(
        "cargo::warning=Fetching {} map tiles from NASA's GIBS into {} to embed them. \
         Set {EMBED}=0 to build without.",
        missing.len(),
        directory.display()
    );
    // curl takes a backslash in a quoted value as an escape.
    let quoted = |text: &str| format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""));
    let part = |path: &Path| path.with_extension("part");
    let config: String = missing
        .iter()
        .map(|(url, path)| {
            format!("url = {}\noutput = {}\n", quoted(url), quoted(&part(path).to_string_lossy()))
        })
        .collect();
    let config_path = Path::new(&std::env::var_os("OUT_DIR").unwrap()).join("tiles.curl");
    std::fs::write(&config_path, config).unwrap();
    let status = Command::new("curl")
        .args(["--parallel", "--parallel-max", "8", "--fail", "--silent", "--show-error"])
        .args(["--retry", "2", "--create-dirs", "--user-agent", "slint-flight-tracker-demo"])
        .arg("--config")
        .arg(&config_path)
        .status();
    if !status.is_ok_and(|status| status.success()) {
        println!("cargo::warning=curl couldn't fetch all of the map tiles");
    }
    // A tile is renamed into place only if it's whole: a JPEG file ends with its end of image
    // marker, and a PNG file with the checksum of its last chunk.
    for (_, path) in &missing {
        let part = part(path);
        let end: &[u8] = if path.extension().is_some_and(|extension| extension == "png") {
            &[0xae, 0x42, 0x60, 0x82]
        } else {
            &[0xff, 0xd9]
        };
        if std::fs::read(&part).is_ok_and(|file| file.ends_with(end)) {
            let _ = std::fs::rename(&part, path);
        } else {
            let _ = std::fs::remove_file(&part);
        }
    }
}
