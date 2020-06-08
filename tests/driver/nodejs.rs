use std::error::Error;
use std::{fs::File, io::Write, path::PathBuf};

pub fn test(testcase: &test_driver_lib::TestCase) -> Result<(), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;

    let mut sixtyfpsdir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    sixtyfpsdir.pop(); // driver
    sixtyfpsdir.pop(); // tests
    sixtyfpsdir.push("api/sixtyfps-node");

    {
        let mut package_json = File::create(dir.path().join("package.json"))?;
        write!(
            package_json,
            r#"
{{
    "name": "sixtyfps_test_{testcase}",
    "version": "0.1.0",
    "main": "main.js",
    "dependencies": {{
        "sixtyfps": "{sixtyfpsdir}"
    }},
    "scripts": {{
        "start": "node ."
    }}
}}"#,
            testcase = testcase.relative_path.file_stem().unwrap().to_string_lossy(),
            sixtyfpsdir = sixtyfpsdir.to_string_lossy(),
        )?;
    }
    {
        let mut main_js = File::create(dir.path().join("main.js"))?;
        write!(
            main_js,
            r#"
const assert = require('assert').strict;
require("sixtyfps");
let sixtyfps = require("{path}");

"#,
            path = testcase.absolute_path.to_string_lossy()
        )?;
        let source = std::fs::read_to_string(&testcase.absolute_path)?;
        for x in test_driver_lib::extract_test_functions(&source).filter(|x| x.language_id == "js")
        {
            write!(main_js, "{{\n    {}\n}}\n", x.source.replace("\n", "\n    "))?;
        }
    }

    let install_output = std::process::Command::new("npm")
        .arg("install")
        .current_dir(dir.path())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .map_err(|err| format!("Could not launch npm install: {}", err))?;

    if !install_output.status.success() {
        print!("{}", String::from_utf8_lossy(install_output.stderr.as_ref()));
        return Err("npm install failed!".to_owned().into());
    }

    let install_output = std::process::Command::new("npm")
        .arg("start")
        .current_dir(dir.path())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .map_err(|err| format!("Could not launch npm start: {}", err))?;

    if !install_output.status.success() {
        print!("{}", String::from_utf8_lossy(install_output.stderr.as_ref()));
        return Err(String::from_utf8_lossy(install_output.stderr.as_ref()).to_owned().into());
    }

    Ok(())
}
