//! This test is trying to compile all the *.60 files in the sub directories and check that compilation
//! errors are properly reported
//!
//! The compiler can have comments like this:
//! ```
//!  // ^error{some_regexp}
//! ```

#[test]
fn main() -> std::io::Result<()> {
    if let Some(specific_test) =
        std::env::args().skip(1).skip_while(|arg| arg.starts_with("--") || arg == "main").next()
    {
        let mut path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("tests");
        path.push(specific_test);
        assert!(process_file(&path)?);
        return Ok(());
    }
    let mut success = true;
    for entry in std::fs::read_dir(format!("{}/tests", env!("CARGO_MANIFEST_DIR")))? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            for test_entry in path.read_dir()? {
                let test_entry = test_entry?;
                let path = test_entry.path();
                if let Some(ext) = path.extension() {
                    if ext == "60" {
                        success &= process_file(&path)?;
                    }
                }
            }
        }
    }
    assert!(success);
    Ok(())
}

fn process_file(path: &std::path::Path) -> std::io::Result<bool> {
    let source = std::fs::read_to_string(&path)?;
    std::panic::catch_unwind(|| process_file_source(path, source, false)).unwrap_or_else(|err| {
        println!("Panic while processing {}: {:?}", path.display(), err);
        Ok(false)
    })
}

fn process_file_source(
    path: &std::path::Path,
    source: String,
    silent: bool,
) -> std::io::Result<bool> {
    let (res, mut diag) = sixtyfps_compilerlib::parser::parse(&source);
    diag.current_path = path.to_path_buf();
    let mut tr = sixtyfps_compilerlib::typeregister::TypeRegister::builtin();
    if !diag.has_error() {
        let doc =
            sixtyfps_compilerlib::object_tree::Document::from_node(res.into(), &mut diag, &mut tr);
        if !diag.has_error() {
            let compiler_config = sixtyfps_compilerlib::CompilerConfiguration::default();
            sixtyfps_compilerlib::run_passes(&doc, &mut diag, &mut tr, &compiler_config);
        }
    }

    //let mut errors = std::collections::HashSet::from_iter(diag.inner.into_iter());
    let mut success = true;

    // Find expected errors in the file.
    let re = regex::Regex::new(r"\n *//[^\n]*(\^)error\{([^\n}]*)\}\n").unwrap();
    for m in re.captures_iter(&source) {
        let line_begin_offset = m.get(0).unwrap().start();
        let column = m.get(1).unwrap().start() - line_begin_offset;
        let rx = m.get(2).unwrap().as_str();
        let r = match regex::Regex::new(&rx) {
            Err(e) => {
                eprintln!("{:?}: Invalid regexp {:?} : {:?}", path, rx, e);
                return Ok(false);
            }
            Ok(r) => r,
        };
        let offset = source[..line_begin_offset].rfind('\n').unwrap_or(0) + column;

        match diag.inner.iter().position(|e| e.span.offset == offset && r.is_match(&e.message)) {
            Some(idx) => {
                diag.inner.remove(idx);
            }
            None => {
                success = false;
                println!("{:?}: Error not found at offset {}: {:?}", path, offset, rx);
            }
        }
    }

    if !diag.inner.is_empty() {
        println!("{:?}: Unexptected errors: {:#?}", path, diag.inner);

        if !silent {
            #[cfg(feature = "display-diagnostics")]
            diag.print(source);
        }

        success = false;
    }

    Ok(success)
}

#[test]
/// Test that this actually fail when it should
fn self_test() -> std::io::Result<()> {
    let fake_path = std::path::Path::new("fake.60");
    let process = |str: &str| process_file_source(&fake_path, str.into(), true);

    // this should succeed
    assert!(process(
        r#"
Foo := Rectangle { x: 0; }
    "#
    )?);

    // unless we expected an error
    assert!(!process(
        r#"
Foo := Rectangle { x: 0; }
//     ^error{i want an error}
    "#
    )?);

    // An error should fail
    assert!(!process(
        r#"
Foo := Rectangle foo { x:0; }
    "#
    )?);

    // An error with the proper comment should pass
    assert!(process(
        r#"
Foo := Rectangle foo { x:0; }
//               ^error{expected LBrace}
    "#
    )?);

    // But not if it is at the wrong position
    assert!(!process(
        r#"
Foo := Rectangle foo { x:0; }
//             ^error{expected LBrace}
    "#
    )?);

    // or the wrong line
    assert!(!process(
        r#"
Foo := Rectangle foo { x:0; }

//               ^error{expected LBrace}
    "#
    )?);

    // or the wrong message
    assert!(!process(
        r#"
Foo := Rectangle foo { x:0; }
//               ^error{foo_bar}
    "#
    )?);

    Ok(())
}
