//! This test is trying to compile all the *.60 files in the sub directories and check that compilation
//! errors are properly reported
//!
//! The compiler can have comments like this:
//! ```
//!  // ^error{some_regexp}
//! ```

#[test]
fn main() -> std::io::Result<()> {
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
    let (res, mut diag) = sixtyfps_compiler::parser::parse(&source);
    diag.current_path = path.to_path_buf();
    let mut tr = sixtyfps_compiler::typeregister::TypeRegister::builtin();
    let doc = sixtyfps_compiler::object_tree::Document::from_node(res, &mut diag, &mut tr);
    if !diag.has_error() {
        sixtyfps_compiler::expression_tree::resolve_expressions(&doc, &mut diag, &mut tr);
    }

    //let mut errors = std::collections::HashSet::from_iter(diag.inner.into_iter());
    let mut success = true;

    // Find expexted errors in the file.
    let re = regex::Regex::new(r"\n *//[^\n]*(\^)error\{([^\n}]*)\}\n").unwrap();
    for m in re.captures_iter(&source) {
        let line_begin_offset = m.get(0).unwrap().start();
        let column = m.get(1).unwrap().start() - line_begin_offset;
        let rx = m.get(1).unwrap().as_str();
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
                println!("{:?}: Error not found ad offset {}: {:?}", path, offset, rx);
            }
        }
    }

    if !diag.inner.is_empty() {
        println!("{:?}: Unexptected errors: {:#?}", path, diag.inner);

        #[cfg(feature = "display-diagnostics")]
        diag.print(source);

        success = false;
    }

    Ok(success)
}
