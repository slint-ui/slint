// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The compiler picks up `slint.project.json` next to the compiled `.slint` file,
//! and the command line still wins over it.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

struct Project {
    _directory: tempfile::TempDir,
    root: PathBuf,
}

impl Project {
    fn new(settings: Option<&str>) -> Self {
        let directory = tempfile::TempDir::new().unwrap();
        // canonicalize so the paths match what the compiler reports back.
        let root = std::fs::canonicalize(directory.path()).unwrap();
        if let Some(settings) = settings {
            std::fs::write(root.join("slint.project.json"), settings).unwrap();
        }
        Self { _directory: directory, root }
    }

    fn write(&self, relative_path: &str, contents: &str) -> PathBuf {
        let path = self.root.join(relative_path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, contents).unwrap();
        path
    }

    fn compile(&self, input: &Path, extra_arguments: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_slint-compiler"))
            .arg("-f")
            .arg("cpp")
            .arg("-o")
            .arg(self.root.join("generated.h"))
            .args(extra_arguments)
            .arg(input)
            .output()
            .unwrap()
    }
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

const IMPORTS_SHARED: &str = r#"import { Shared } from "shared.slint";
export component Main inherits Window { Shared { } }"#;

const PLAIN: &str = "export component Main inherits Window { }";

#[test]
fn include_directories_come_from_the_project_file() {
    let project = Project::new(Some(r#"{ "include-directories": ["include"] }"#));
    project.write("include/shared.slint", "export component Shared { }");
    let main = project.write("main.slint", IMPORTS_SHARED);

    let output = project.compile(&main, &[]);
    assert!(output.status.success(), "{}", stderr(&output));
}

#[test]
fn an_include_path_argument_wins_over_the_project_file() {
    let project = Project::new(Some(r#"{ "include-directories": ["unusable"] }"#));
    project.write("other/shared.slint", "export component Shared { }");
    let main = project.write("main.slint", IMPORTS_SHARED);

    let include_path = project.root.join("other");
    let output = project.compile(&main, &["-I", include_path.to_str().unwrap()]);
    assert!(output.status.success(), "{}", stderr(&output));
}

#[test]
fn library_paths_come_from_the_project_file() {
    let project = Project::new(Some(r#"{ "library-paths": {"widgets": "widgets.slint"} }"#));
    project.write("widgets.slint", "export component Widget { }");
    let main = project.write(
        "main.slint",
        r#"import { Widget } from "@widgets";
           export component Main inherits Window { Widget { } }"#,
    );

    let output = project.compile(&main, &[]);
    assert!(output.status.success(), "{}", stderr(&output));
}

#[test]
fn the_project_file_style_reaches_the_compiler() {
    // A style name the compiler rejects shows which style it actually used.
    let project = Project::new(Some(r#"{ "style": "no-such-style" }"#));
    let main = project.write("main.slint", PLAIN);

    let output = project.compile(&main, &[]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("no-such-style"), "{}", stderr(&output));
}

#[test]
fn a_style_argument_wins_over_the_project_file() {
    let project = Project::new(Some(r#"{ "style": "no-such-style" }"#));
    let main = project.write("main.slint", PLAIN);

    let output = project.compile(&main, &["--style", "fluent"]);
    assert!(output.status.success(), "{}", stderr(&output));
}

#[test]
fn an_unreadable_project_file_is_reported() {
    let project = Project::new(Some("{"));
    let main = project.write("main.slint", PLAIN);

    let output = project.compile(&main, &[]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("slint.project.json"), "{}", stderr(&output));
}

#[test]
fn the_project_file_is_a_dependency_in_the_depfile() {
    let project = Project::new(Some(r#"{ "style": "fluent" }"#));
    let main = project.write("main.slint", PLAIN);
    let depfile = project.root.join("generated.d");

    let output = project.compile(&main, &["--depfile", depfile.to_str().unwrap()]);
    assert!(output.status.success(), "{}", stderr(&output));

    let dependencies = std::fs::read_to_string(&depfile).unwrap();
    let project_file = project.root.join("slint.project.json");
    assert!(
        dependencies.contains(project_file.to_str().unwrap()),
        "expected {} in {dependencies}",
        project_file.display()
    );
}

#[test]
fn compiling_without_a_project_file_works() {
    let project = Project::new(None);
    let main = project.write("main.slint", PLAIN);

    let output = project.compile(&main, &[]);
    assert!(output.status.success(), "{}", stderr(&output));
}
