use std::error::Error;
use std::{fs::File, io::Write, path::PathBuf};

lazy_static::lazy_static! {
    static ref NATIVE_LIB: PathBuf = {
        use test_driver_lib::Message;
        let mut res = PathBuf::new();
        test_driver_lib::run_cargo(&env!("CARGO"), "build", &["-p", "sixtyfps-node"], |message| {
            if let Message::CompilerArtifact(artifact) = message {
                if artifact.target.name != "sixtyfps_node_native" {
                    return Ok(());
                }
                assert!(res.as_os_str().is_empty(), "There must be only one target with name 'sixtyfps_node_native'");
                res = artifact.filenames[0].clone();
            };
            Ok(())
        }).expect("Could not run cargo build to extract native node plugin path");
        assert!(!res.as_os_str().is_empty(), "Did not find the native nodejs lib (sixtyfps_node_native)");
        res
    };
}

pub fn test(testcase: &test_driver_lib::TestCase) -> Result<(), Box<dyn Error>> {
    let native_lib = NATIVE_LIB.as_os_str();

    let mut sixtyfpspath = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    sixtyfpspath.pop(); // driver
    sixtyfpspath.pop(); // tests
    sixtyfpspath.push("api/sixtyfps-node/lib/index.js");

    let dir = tempfile::tempdir()?;

    let mut main_js = File::create(dir.path().join("main.js"))?;
    write!(
        main_js,
        r#"
                const assert = require('assert').strict;
                require("{sixtyfpspath}");
                let sixtyfps = require("{path}");
        "#,
        sixtyfpspath = sixtyfpspath.to_string_lossy(),
        path = testcase.absolute_path.to_string_lossy()
    )?;
    let source = std::fs::read_to_string(&testcase.absolute_path)?;
    for x in test_driver_lib::extract_test_functions(&source).filter(|x| x.language_id == "js") {
        write!(main_js, "{{\n    {}\n}}\n", x.source.replace("\n", "\n    "))?;
    }

    let output = std::process::Command::new("node")
        .arg(dir.path().join("main.js"))
        .current_dir(dir.path())
        .env("SIXTYFPS_NODE_NATIVE_LIB", native_lib)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .map_err(|err| format!("Could not launch npm start: {}", err))?;

    if !output.status.success() {
        print!("{}", String::from_utf8_lossy(output.stdout.as_ref()));
        print!("{}", String::from_utf8_lossy(output.stderr.as_ref()));
        return Err(String::from_utf8_lossy(output.stderr.as_ref()).to_owned().into());
    }

    Ok(())
}
