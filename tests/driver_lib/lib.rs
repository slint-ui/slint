use cargo_metadata::diagnostic::DiagnosticLevel;
use cargo_metadata::Message;
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
    libs_params.extend(["--", "--print=native-static-libs"].iter());

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
