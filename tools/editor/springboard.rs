// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use i_slint_springboard::{
    ClientCommand, ClientRequest, RequestId, SPRINGBOARD_PROTOCOL_VERSION, ServerMessage,
};
use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader};

type Result<T> = i_slint_editor_preview::Result<T>;

#[derive(Debug, Eq, PartialEq)]
struct SpringboardCommand {
    executable: PathBuf,
    arguments: Vec<OsString>,
    working_directory: PathBuf,
}

fn sibling_springboard_executable(editor_executable: &Path) -> Result<PathBuf> {
    let Some(directory) = editor_executable.parent() else {
        return Err("Failed to determine the Visual Editor executable directory".into());
    };
    Ok(directory.join(format!("slint-springboard{}", std::env::consts::EXE_SUFFIX)))
}

fn springboard_command(
    editor_executable: &Path,
    project_root: &Path,
    executable_override: Option<&Path>,
) -> Result<SpringboardCommand> {
    let executable = match executable_override {
        Some(executable) => executable.to_path_buf(),
        None => sibling_springboard_executable(editor_executable)?,
    };
    Ok(SpringboardCommand {
        executable,
        arguments: vec![
            OsString::from("serve"),
            OsString::from("--stdio"),
            project_root.as_os_str().to_owned(),
        ],
        working_directory: project_root.to_owned(),
    })
}

#[derive(Debug)]
pub enum SpringboardHostEvent {
    Message(ServerMessage),
    Error(String),
    Closed,
}

#[derive(Debug)]
struct GenerationEvent {
    generation: u64,
    event: SpringboardHostEvent,
}

pub struct SpringboardProcess {
    child: Option<tokio::process::Child>,
    input: Option<tokio::process::ChildStdin>,
    reader: Option<tokio::task::JoinHandle<()>>,
    events_sender: tokio::sync::mpsc::UnboundedSender<GenerationEvent>,
    events_receiver: tokio::sync::mpsc::UnboundedReceiver<GenerationEvent>,
    generation: u64,
    project_root: Option<PathBuf>,
    manifest_present: bool,
    launch_failed: bool,
    executable_override: Option<PathBuf>,
}

impl Default for SpringboardProcess {
    fn default() -> Self {
        let (events_sender, events_receiver) = tokio::sync::mpsc::unbounded_channel();
        Self {
            child: None,
            input: None,
            reader: None,
            events_sender,
            events_receiver,
            generation: 0,
            project_root: None,
            manifest_present: false,
            launch_failed: false,
            executable_override: None,
        }
    }
}

impl SpringboardProcess {
    pub async fn ensure_project(&mut self, project_root: &Path) -> Result<()> {
        let project_root = std::fs::canonicalize(project_root).map_err(|error| {
            format!("Failed to resolve Springboard project {}: {error}", project_root.display())
        })?;
        let manifest_present =
            i_slint_springboard::project::load_project_run_target(&project_root)?.is_some();
        let same_configuration = self.project_root.as_ref() == Some(&project_root)
            && self.manifest_present == manifest_present;
        if same_configuration && (self.child.is_some() || self.launch_failed || !manifest_present) {
            return Ok(());
        }

        self.stop().await?;
        self.project_root = Some(project_root.clone());
        self.manifest_present = manifest_present;
        self.launch_failed = false;
        if !manifest_present {
            return Ok(());
        }

        let result = self.start(&project_root).await;
        if result.is_err() {
            self.launch_failed = true;
        }
        result
    }

    async fn start(&mut self, project_root: &Path) -> Result<()> {
        let editor_executable = std::env::current_exe()
            .map_err(|error| format!("Failed to locate the Visual Editor executable: {error}"))?;
        let command = springboard_command(
            &editor_executable,
            project_root,
            self.executable_override.as_deref(),
        )?;
        if !command.executable.is_file() {
            return Err(format!(
                "Could not find Springboard at {}. Build it with `cargo build -p slint-springboard` or reinstall the Visual Editor.",
                command.executable.display()
            )
            .into());
        }

        let mut process = tokio::process::Command::new(&command.executable);
        process
            .args(&command.arguments)
            .current_dir(&command.working_directory)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true);
        let mut child = process.spawn().map_err(|error| {
            format!("Failed to start Springboard at {}: {error}", command.executable.display())
        })?;
        let mut input = child.stdin.take().ok_or("Failed to open Springboard stdin")?;
        let output = child.stdout.take().ok_or("Failed to open Springboard stdout")?;

        let handshake = ClientRequest {
            protocol_version: SPRINGBOARD_PROTOCOL_VERSION,
            request_id: RequestId(1),
            command: ClientCommand::Handshake { client_name: "slint-editor".into() },
        };
        let mut line = serde_json::to_vec(&handshake)?;
        line.push(b'\n');
        input.write_all(&line).await?;
        input.flush().await?;

        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        let events = self.events_sender.clone();
        self.reader = Some(tokio::task::spawn_local(async move {
            let mut lines = BufReader::new(output).lines();
            loop {
                match lines.next_line().await {
                    Ok(Some(line)) => match serde_json::from_str::<ServerMessage>(&line) {
                        Ok(message) => {
                            if events
                                .send(GenerationEvent {
                                    generation,
                                    event: SpringboardHostEvent::Message(message),
                                })
                                .is_err()
                            {
                                break;
                            }
                        }
                        Err(error) => {
                            let _ = events.send(GenerationEvent {
                                generation,
                                event: SpringboardHostEvent::Error(format!(
                                    "Springboard sent an invalid protocol message: {error}"
                                )),
                            });
                            break;
                        }
                    },
                    Ok(None) => {
                        let _ = events.send(GenerationEvent {
                            generation,
                            event: SpringboardHostEvent::Closed,
                        });
                        break;
                    }
                    Err(error) => {
                        let _ = events.send(GenerationEvent {
                            generation,
                            event: SpringboardHostEvent::Error(format!(
                                "Failed reading Springboard output: {error}"
                            )),
                        });
                        break;
                    }
                }
            }
        }));
        self.input = Some(input);
        self.child = Some(child);
        Ok(())
    }

    pub async fn recv(&mut self) -> Option<SpringboardHostEvent> {
        loop {
            let event = self.events_receiver.recv().await?;
            if event.generation == self.generation {
                return Some(event.event);
            }
        }
    }

    pub async fn stop(&mut self) -> Result<()> {
        self.generation = self.generation.wrapping_add(1);
        if let Some(mut input) = self.input.take() {
            let request = ClientRequest {
                protocol_version: SPRINGBOARD_PROTOCOL_VERSION,
                request_id: RequestId(2),
                command: ClientCommand::Shutdown,
            };
            let mut line = serde_json::to_vec(&request)?;
            line.push(b'\n');
            let _ = input.write_all(&line).await;
            let _ = input.flush().await;
        }
        if let Some(mut child) = self.child.take()
            && child.try_wait()?.is_none()
            && tokio::time::timeout(Duration::from_secs(1), child.wait()).await.is_err()
        {
            child.start_kill()?;
            child.wait().await?;
        }
        if let Some(reader) = self.reader.take() {
            reader.abort();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_springboard_next_to_the_editor() {
        let editor = Path::new("/build/output/slint-editor");

        assert_eq!(
            sibling_springboard_executable(editor).unwrap(),
            Path::new("/build/output")
                .join(format!("slint-springboard{}", std::env::consts::EXE_SUFFIX))
        );
    }

    #[test]
    fn test_override_builds_a_headless_project_command() {
        let command = springboard_command(
            Path::new("/build/output/slint-editor"),
            Path::new("/project with spaces"),
            Some(Path::new("/test/springboard")),
        )
        .unwrap();

        assert_eq!(command.executable, Path::new("/test/springboard"));
        assert_eq!(command.working_directory, Path::new("/project with spaces"));
        assert_eq!(
            command.arguments,
            ["serve", "--stdio", "/project with spaces"].map(OsString::from)
        );
    }
}
