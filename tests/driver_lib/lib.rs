use cargo_metadata::diagnostic::DiagnosticLevel;
use cargo_metadata::Message;
use regex::Regex;
use std::error::Error;
use std::process::Command;

pub fn run_cargo(
    cargo_command: &str,
    sub_command: &str,
    params: &[&str],
    mut message_handler: impl FnMut(&cargo_metadata::Message) -> Result<(), Box<dyn Error>>,
) -> Result<std::process::ExitStatus, Box<dyn Error>> {
    let mut cmd = Command::new(cargo_command)
        .arg(sub_command)
        .arg("--message-format=json")
        .args(params)
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();

    let reader = std::io::BufReader::new(cmd.stdout.take().unwrap());

    for message in cargo_metadata::Message::parse_stream(reader) {
        message_handler(&message.unwrap())?;
    }

    Ok(cmd.wait()?)
}

pub fn native_library_dependencies(
    cargo_command: &str,
    build_params: &[&str],
    package: &str,
) -> Result<String, Box<dyn Error>> {
    let mut native_library_dependencies = String::new();

    let mut libs_params = vec!["-p", package];
    libs_params.extend(build_params);

    run_cargo(cargo_command, "rustc", &libs_params, |message| {
        match message {
            Message::CompilerMessage(msg) => {
                let message = &msg.message;
                const NATIVE_LIBS_PREFIX: &str = "native-static-libs:";
                if matches!(message.level, DiagnosticLevel::Note)
                    && message.message.starts_with(NATIVE_LIBS_PREFIX)
                {
                    let native_libs = message.message[NATIVE_LIBS_PREFIX.len()..].trim();
                    native_library_dependencies.push_str(native_libs.into());
                    native_library_dependencies.push_str(" ");
                }
            }
            _ => (),
        }
        Ok(())
    })?;

    Ok(native_library_dependencies.trim().into())
}

pub struct TestCase {
    pub absolute_path: std::path::PathBuf,
    pub relative_path: std::path::PathBuf,
}

/// Returns a list of all the `.60` files in the `tests/cases` subfolders.
pub fn collect_test_cases() -> std::io::Result<Vec<TestCase>> {
    let mut results = vec![];

    let case_root_dir = format!("{}/../cases", env!("CARGO_MANIFEST_DIR"));

    for entry in std::fs::read_dir(case_root_dir.clone())? {
        let entry = entry?;
        let absolute_path = entry.path();
        if let Some(ext) = absolute_path.extension() {
            if ext == "60" {
                let relative_path =
                    std::path::PathBuf::from(absolute_path.strip_prefix(&case_root_dir).unwrap());

                results.push(TestCase { absolute_path, relative_path });
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
    lazy_static::lazy_static! {
        static ref RX: Regex = Regex::new(r"(?sU)\n```([a-z]+)\n(.+)\n```\n").unwrap();
    }
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
