// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=link-data.json");
    println!("cargo:rerun-if-changed=link-data-generated.json");

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let out_dir = env::var("OUT_DIR").unwrap();

    let manual_path = Path::new(&manifest_dir).join("link-data.json");
    let manual: HashMap<String, serde_json::Value> =
        serde_json::from_str(&fs::read_to_string(&manual_path).unwrap())
            .expect("Failed to parse link-data.json");

    let mut merged = manual;
    let generated_path = Path::new(&manifest_dir).join("link-data-generated.json");
    if generated_path.exists() {
        let generated: HashMap<String, serde_json::Value> =
            serde_json::from_str(&fs::read_to_string(&generated_path).unwrap())
                .expect("Failed to parse link-data-generated.json");
        for (k, v) in generated {
            merged.insert(k, v);
        }
    }

    let out_path = Path::new(&out_dir).join("link-data.json");
    fs::write(
        &out_path,
        serde_json::to_string_pretty(&merged).unwrap(),
    )
    .unwrap();
}
