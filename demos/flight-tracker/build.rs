// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// cSpell: ignore CACHEDIR

use std::collections::HashMap;
use std::hash::{BuildHasher, RandomState};
use std::path::{Path, PathBuf};
use std::process::Command;

#[path = "gibs.rs"]
mod gibs;
use gibs::Kind;

/// Set to embed the map tiles: to `1` or `on` to fetch them first,
/// or to the directory of tiles to embed, without fetching any.
const EMBED: &str = "FLIGHT_TRACKER_EMBED_TILES";

fn main() {
    slint_build::compile("ui/app.slint").unwrap();
    println!("cargo::rerun-if-env-changed={EMBED}");
    let wasm = std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("wasm32");
    if !wasm && embed_tiles() {
        println!("cargo::rustc-cfg=embedded_tiles");
    } else if wasm || std::env::var_os("CARGO_FEATURE_ONLINE_IMAGERY").is_some() {
        println!("cargo::rustc-cfg=online_tiles");
    }
}

/// Puts the map tiles into the binary, which the demo then shows instead of fetching them,
/// such as on a device without a network.
/// Returns whether there are tiles in the binary.
fn embed_tiles() -> bool {
    let setting = std::env::var(EMBED).unwrap_or_default();
    let (directory, fetch) = match setting.as_str() {
        "" | "0" | "off" => return false,
        "1" | "on" => {
            // Cargo marks its target directory with this file, and each target's directory in
            // it. In the outermost, the tiles stay for all targets and profiles, and in a
            // container that keeps nothing else.
            let out = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
            let marked = |directory: &&Path| directory.join("CACHEDIR.TAG").exists();
            let target = out.ancestors().filter(marked).last();
            (target.unwrap_or(&out).join("flight-tracker-tiles"), true)
        }
        directory => (PathBuf::from(directory), false),
    };

    let tiles: Vec<(Kind, u32, u32, u32)> = [Kind::Day, Kind::Night]
        .into_iter()
        .flat_map(|kind| {
            (gibs::FIRST_ZOOM..gibs::FIRST_ZOOM + gibs::LEVELS as u32).flat_map(move |zoom| {
                (0..gibs::rows(zoom)).flat_map(move |row| {
                    (0..gibs::columns(zoom)).map(move |column| (kind, zoom, row, column))
                })
            })
        })
        .collect();
    if fetch {
        fetch_missing(&directory, &tiles);
    } else {
        println!("cargo::rerun-if-changed={}", directory.display());
    }

    // The size and the time of each tile that's there say whether the pack of the last build
    // still holds them, which saves reading and writing some 150 MB.
    let files: Vec<_> = tiles
        .iter()
        .map(|&(kind, zoom, row, column)| {
            let path = directory.join(kind.path(zoom, row, column));
            let file = std::fs::metadata(&path).ok().map(|file| (file.len(), file.modified().ok()));
            file.map(|file| (path, file))
        })
        .collect();
    let embedded = files.iter().flatten().count();
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
        return false;
    }
    let out = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    let [pack, source, stamp] = ["bin", "rs", "stamp"]
        .map(|extension| out.join("embedded_tiles").with_extension(extension));
    let state = format!("{files:?}");
    if pack.exists()
        && source.exists()
        && std::fs::read_to_string(&stamp).is_ok_and(|last| last == state)
    {
        return true;
    }

    // The tiles one after the other, each kind of content once: most of the night is black.
    let mut data: Vec<u8> = Vec::new();
    let mut index = String::new();
    let mut places: HashMap<u64, (usize, usize)> = HashMap::new();
    let hasher = RandomState::new();
    for (&(kind, zoom, row, column), file) in tiles.iter().zip(&files) {
        let Some(Ok(file)) = file.as_ref().map(|(path, _)| std::fs::read(path)) else { continue };
        let place = *places.entry(hasher.hash_one(&file)).or_insert((data.len(), file.len()));
        let (offset, length) = if data.get(place.0..place.0 + place.1) == Some(&file[..]) {
            place
        } else {
            data.extend_from_slice(&file);
            (data.len() - file.len(), file.len())
        };
        index += &format!("({}, {zoom}, {row}, {column}, {offset}, {length}),\n", kind as u8);
    }
    std::fs::write(pack, data).unwrap();
    std::fs::write(
        source,
        format!(
            "/// The kind, zoom, row, and column of each tile, in that order, and its offset and
/// length in `DATA`.
pub static INDEX: &[(u8, u8, u8, u8, u32, u32)] = &[\n{index}];
pub static DATA: &[u8] = include_bytes!(concat!(env!(\"OUT_DIR\"), \"/embedded_tiles.bin\"));\n"
        ),
    )
    .unwrap();
    std::fs::write(stamp, state).unwrap();
    true
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
        "cargo::warning=Fetching {} map tiles from NASA's GIBS into {} to embed them",
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
        // Without a network, the first tile that fails ends it, rather than each in turn.
        .args(["--fail-early", "--connect-timeout", "15", "--max-time", "60", "--retry", "2"])
        .args(["--create-dirs", "--user-agent", gibs::USER_AGENT])
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
