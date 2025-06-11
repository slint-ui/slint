// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

pub struct TestCase {
    pub absolute_path: std::path::PathBuf,
    pub relative_path: std::path::PathBuf,
    pub requested_style: Option<&'static str>,
}

impl TestCase {
    /// Return a string which is a valid C++/Rust identifier
    pub fn identifier(&self) -> String {
        let mut result = self
            .relative_path
            .with_extension("")
            .to_string_lossy()
            .replace([std::path::MAIN_SEPARATOR, '-'], "_");
        if let Some(requested_style) = &self.requested_style {
            result.push('_');
            result.push_str(requested_style);
        }
        result
    }

    /// Returns true if the test case should be ignored for the specified driver.
    pub fn is_ignored(&self, driver: &str) -> bool {
        let source = std::fs::read_to_string(&self.absolute_path).unwrap();
        extract_ignores(&source).collect::<Vec<_>>().contains(&driver)
    }
}

/// Returns a list of all the `.slint` files in the subfolders e.g. `tests/cases` .
pub fn collect_test_cases(sub_folders: &str) -> std::io::Result<Vec<TestCase>> {
    let mut results = vec![];

    let mut all_styles = vec!["fluent", "material", "cupertino", "cosmic"];

    // It is in the target/xxx/build directory
    if std::env::var_os("OUT_DIR").is_some_and(|path| {
        // Same logic as in i-slint-backend-selector's build script to get the path
        let mut path: PathBuf = path.into();
        path.pop();
        path.pop();
        path.push("SLINT_DEFAULT_STYLE.txt");
        std::fs::read_to_string(path).is_ok_and(|style| style.trim().contains("qt"))
    }) {
        all_styles.push("qt");
    }

    let case_root_dir: std::path::PathBuf =
        [env!("CARGO_MANIFEST_DIR"), "..", "..", sub_folders].iter().collect();

    println!("cargo:rerun-if-env-changed=SLINT_TEST_FILTER");
    let filter = std::env::var("SLINT_TEST_FILTER").ok();

    for entry in walkdir::WalkDir::new(case_root_dir.clone()).follow_links(true) {
        let entry = entry?;
        if entry.file_type().is_dir() {
            println!("cargo:rerun-if-changed={}", entry.into_path().display());
            continue;
        }
        let absolute_path = entry.into_path();
        let relative_path =
            std::path::PathBuf::from(absolute_path.strip_prefix(&case_root_dir).unwrap());
        if let Some(filter) = &filter {
            if !relative_path.to_str().unwrap().contains(filter) {
                continue;
            }
        }
        if let Some(ext) = absolute_path.extension() {
            if ext == "60" || ext == "slint" {
                let styles_to_test: Vec<&'static str> = if relative_path.starts_with("widgets") {
                    let style_ignores =
                        extract_ignores(&std::fs::read_to_string(&absolute_path).unwrap())
                            .filter_map(|ignore| {
                                ignore.strip_prefix("style-").map(ToString::to_string)
                            })
                            .collect::<Vec<_>>();

                    all_styles
                        .iter()
                        .filter(|available_style| {
                            !style_ignores
                                .iter()
                                .any(|ignored_style| *available_style == ignored_style)
                        })
                        .cloned()
                        .collect::<Vec<_>>()
                } else {
                    vec![""]
                };
                results.extend(styles_to_test.into_iter().map(|style| TestCase {
                    absolute_path: absolute_path.clone(),
                    relative_path: relative_path.clone(),
                    requested_style: if style.is_empty() { None } else { Some(style) },
                }));
            }
        }
    }
    Ok(results)
}

/// A test functions looks something like
/// ````text
/// /*
///   ```cpp
///   TestCase instance;
///   assert(instance.x.get() == 0);
///   ```
/// */
/// ````
pub struct TestFunction<'a> {
    /// In the example above: `cpp`
    pub language_id: &'a str,
    /// The content of the test function
    pub source: &'a str,
}

/// Extract the test functions from
pub fn extract_test_functions(source: &str) -> impl Iterator<Item = TestFunction<'_>> {
    static RX: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?sU)\r?\n```([a-z]+)\r?\n(.+)\r?\n```\r?\n").unwrap());
    RX.captures_iter(source).map(|mat| TestFunction {
        language_id: mat.get(1).unwrap().as_str(),
        source: mat.get(2).unwrap().as_str(),
    })
}

#[test]
fn test_extract_test_functions() {
    let source = r"
/*
```cpp
auto xx = 0;
auto yy = 0;
```

```rust
let xx = 0;
let yy = 0;
```
*/
";
    let mut r = extract_test_functions(source);

    let r1 = r.next().unwrap();
    assert_eq!(r1.language_id, "cpp");
    assert_eq!(r1.source, "auto xx = 0;\nauto yy = 0;");

    let r2 = r.next().unwrap();
    assert_eq!(r2.language_id, "rust");
    assert_eq!(r2.source, "let xx = 0;\nlet yy = 0;");
}

#[test]
fn test_extract_test_functions_win() {
    let source = "/*\r\n```cpp\r\nfoo\r\nbar\r\n```\r\n*/\r\n";
    let mut r = extract_test_functions(source);
    let r1 = r.next().unwrap();
    assert_eq!(r1.language_id, "cpp");
    assert_eq!(r1.source, "foo\r\nbar");
}

/// Extract extra include paths from a comment in the source if present.
pub fn extract_include_paths(source: &str) -> impl Iterator<Item = &'_ str> {
    static RX: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"//include_path:\s*(.+)\s*\n").unwrap());
    RX.captures_iter(source).map(|mat| mat.get(1).unwrap().as_str().trim())
}

#[test]
fn test_extract_include_paths() {
    assert!(extract_include_paths("something").next().is_none());

    let source = r"
    //include_path: ../first
    //include_path: ../second
    Blah {}
";

    let r = extract_include_paths(source).collect::<Vec<_>>();
    assert_eq!(r, ["../first", "../second"]);

    // Windows \r\n
    let source = "//include_path: ../first\r\n//include_path: ../second\r\nBlah {}\r\n";
    let r = extract_include_paths(source).collect::<Vec<_>>();
    assert_eq!(r, ["../first", "../second"]);
}

/// Extract extra library paths from a comment in the source if present.
pub fn extract_library_paths(source: &str) -> impl Iterator<Item = (&'_ str, &'_ str)> {
    static RX: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"//library_path\((.+)\):\s*(.+)\s*\n").unwrap());
    RX.captures_iter(source)
        .map(|mat| (mat.get(1).unwrap().as_str().trim(), mat.get(2).unwrap().as_str().trim()))
}

#[test]
fn test_extract_library_paths() {
    use std::collections::HashMap;

    assert!(extract_library_paths("something").next().is_none());

    let source = r"
    //library_path(first): ../first/lib.slint
    //library_path(second): ../second/lib.slint
    Blah {}
";

    let r = extract_library_paths(source).collect::<HashMap<_, _>>();
    assert_eq!(
        r,
        HashMap::from([("first", "../first/lib.slint"), ("second", "../second/lib.slint")])
    );
}

/// Extract `//ignore` comments from the source.
fn extract_ignores(source: &str) -> impl Iterator<Item = &'_ str> {
    static RX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"//ignore:\s*(.+)\s*\n").unwrap());
    RX.captures_iter(source).flat_map(|mat| {
        mat.get(1).unwrap().as_str().split(&[' ', ',']).map(str::trim).filter(|s| !s.is_empty())
    })
}

#[test]
fn test_extract_ignores() {
    assert!(extract_ignores("something").next().is_none());

    let source = r"
    //ignore: cpp
    //ignore: rust, nodejs
    Blah {}
";

    let r = extract_ignores(source).collect::<Vec<_>>();
    assert_eq!(r, ["cpp", "rust", "nodejs"]);
}

pub fn extract_cpp_namespace(source: &str) -> Option<String> {
    static RX: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"//cpp-namespace:\s*(.+)\s*\n").unwrap());
    RX.captures(source).map(|mat| mat.get(1).unwrap().as_str().trim().to_string())
}

#[test]
fn test_extract_cpp_namespace() {
    assert!(extract_cpp_namespace("something").is_none());

    let source = r"
    //cpp-namespace: ui
    Blah {}
";

    let r = extract_cpp_namespace(source);
    assert_eq!(r, Some("ui".to_string()));
}
