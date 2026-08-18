// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#[cfg(not(feature = "preview-ui"))]
compile_error!("Springboard must enable the configured live-preview feature");

use std::io::{BufRead as _, BufReader, Write as _};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

const ENDPOINT: &str = "SLINT_SPRINGBOARD_ENDPOINT";
const TOKEN: &str = "SLINT_SPRINGBOARD_TOKEN";

struct Reporter {
    reader: BufReader<TcpStream>,
    writer: TcpStream,
    token: String,
}

impl Reporter {
    fn from_environment() -> Self {
        let endpoint = std::env::var(ENDPOINT).unwrap();
        let writer = TcpStream::connect(endpoint).unwrap();
        writer.set_nodelay(true).unwrap();
        let reader = BufReader::new(writer.try_clone().unwrap());
        Self { reader, writer, token: std::env::var(TOKEN).unwrap() }
    }

    fn report(&mut self, event: &str, fields: &str) {
        writeln!(
            self.writer,
            "{{\"protocol_version\":1,\"token\":\"{}\",\"event\":\"{event}\"{fields}}}",
            json_string(&self.token),
        )
        .unwrap();
        self.writer.flush().unwrap();

        let mut acknowledgement = String::new();
        self.reader.read_line(&mut acknowledgement).unwrap();
        assert!(acknowledgement.contains("\"accepted\":true"), "{acknowledgement}");
    }
}

fn json_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn path_array(paths: &[PathBuf]) -> String {
    paths
        .iter()
        .map(|path| format!("\"{}\"", json_string(&path.to_string_lossy())))
        .collect::<Vec<_>>()
        .join(",")
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap()
}

fn main() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let entry = root.join("ui/app.slint");
    let resource = root.join("ui/resource.txt");
    let hot_reload_paths = vec![entry.clone(), resource.clone()];
    let paths = path_array(&hot_reload_paths);
    let mut reporter = Reporter::from_environment();
    reporter.report("ready", &format!(",\"hot_reload_paths\":[{paths}]"));
    println!("SPRINGBOARD_FIXTURE_PID={}", std::process::id());

    let mut last_entry = read(&entry);
    let mut last_resource = read(&resource);
    loop {
        std::thread::sleep(Duration::from_millis(20));
        let entry_source = read(&entry);
        let resource_source = read(&resource);
        if entry_source == last_entry && resource_source == last_resource {
            continue;
        }
        last_entry = entry_source.clone();
        last_resource = resource_source;

        let fields = format!(",\"hot_reload_paths\":[{paths}]");
        if entry_source.contains("springboard-fixture: compile-error") {
            reporter.report("compile-error", &fields);
        } else if entry_source.contains("springboard-fixture: interface-change") {
            reporter.report(
                "rebuild-required",
                &format!(",\"diff\":\"exported property changed\",\"hot_reload_paths\":[{paths}]"),
            );
        } else {
            reporter.report("reloaded", &fields);
        }
    }
}
