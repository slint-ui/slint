use std::error::Error;
use std::fs::File;
use std::process::Command;
use std::{io::Write, path::PathBuf};

pub fn test_macro(testcase: &test_driver_lib::TestCase) -> Result<(), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;

    let mut sixtyfpsdir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    sixtyfpsdir.pop(); // driver
    sixtyfpsdir.pop(); // tests
    sixtyfpsdir.push("api/sixtyfps-rs");

    let mut target_dir = PathBuf::from(env!("OUT_DIR"));
    target_dir.pop(); // out
    target_dir.pop(); // driver-xxxx
    target_dir.pop(); // build
    target_dir.pop(); // debug

    {
        let mut cargo_toml = File::create(dir.path().join("Cargo.toml"))?;
        write!(
            cargo_toml,
            r#"
[package]
name = "testcase_{testcase}"
version = "0.0.0"
edition = "2018"
publish = false

[[bin]]
path = "main.rs"
name = "testcase_{testcase}"

[dependencies]
sixtyfps = {{ path = "{sixtyfpsdir}" }}
"#,
            testcase = testcase.relative_path.file_stem().unwrap().to_string_lossy(),
            sixtyfpsdir = sixtyfpsdir.to_string_lossy(),
        )?;
    }
    {
        let mut main_rs = File::create(dir.path().join("main.rs"))?;
        main_rs.write_all(b"sixtyfps::sixtyfps!{\n")?;
        std::io::copy(&mut File::open(&testcase.absolute_path)?, &mut main_rs)?;
        main_rs.write_all(b"\n}\n")?;
    }
    let output = Command::new(env!("CARGO"))
        .arg("run")
        .arg("--target-dir")
        .arg(target_dir)
        .arg("--manifest-path")
        .arg(dir.path().join("Cargo.toml"))
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?
        .wait_with_output()?;

    print!("{}", String::from_utf8_lossy(output.stderr.as_ref()));

    if !output.status.success() {
        return Err("Cargo exited with error".to_owned().into());
    }

    Ok(())
}

pub fn test_buildrs(testcase: &test_driver_lib::TestCase) -> Result<(), Box<dyn Error>> {
    //TODO
    Ok(())
}
