// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

const DISCOVERY_DIRECTORY: &str = "slint-visual-editor-instances";
const RPC_TIMEOUT: Duration = Duration::from_secs(30);
static INSTANCE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EditorAnnotation {
    pub id: String,
    pub text: String,
    pub file: PathBuf,
    pub range: SourceRange,
    pub component: Option<String>,
    pub element_type: String,
    pub element_id: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AnnotationAuthor {
    User,
    Codex,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AnnotationMessage {
    pub id: String,
    pub text: String,
    pub author: AnnotationAuthor,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SourceRange {
    pub start: SourcePosition,
    pub end: SourcePosition,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SourcePosition {
    pub line: u32,
    pub character: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatProvider {
    Codex,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ChatRegistration {
    pub provider: ChatProvider,
    pub thread_id: String,
    pub display_name: String,
    pub cli_path: PathBuf,
}

impl ChatRegistration {
    pub fn validate(&self) -> Result<(), String> {
        if self.thread_id.trim().is_empty() || self.display_name.trim().is_empty() {
            return Err("threadId and displayName must not be empty".into());
        }
        if !self.cli_path.is_absolute() {
            return Err("cliPath must be an absolute path".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum EditorRequest {
    Ping,
    RegisterChat {
        project_root: PathBuf,
        chat: ChatRegistration,
    },
    CanvasScreenshot {
        project_root: PathBuf,
    },
    ReplyAnnotation {
        project_root: PathBuf,
        annotation_id: String,
        text: String,
        provider: ChatProvider,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EditorResponse {
    Pong,
    ChatRegistered { chat: ChatRegistration },
    CanvasScreenshot { png_base64: String },
    AnnotationReplied { annotation_id: String, message_id: String },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EditorDiscovery {
    pub instance_id: String,
    pub project_root: PathBuf,
    pub endpoint: SocketAddr,
    pub token: String,
}

#[derive(Deserialize, Serialize)]
struct RpcRequest {
    token: String,
    working_directory: PathBuf,
    request: EditorRequest,
}

pub struct EditorServer {
    discovery: EditorDiscovery,
    discovery_path: PathBuf,
    project_root: Arc<Mutex<Option<PathBuf>>>,
    stopped: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl EditorServer {
    pub fn start(
        callback: impl Fn(EditorRequest) -> Result<EditorResponse, String> + Send + 'static,
    ) -> io::Result<Self> {
        Self::in_directory(discovery_directory(), callback)
    }

    fn in_directory(
        directory: PathBuf,
        callback: impl Fn(EditorRequest) -> Result<EditorResponse, String> + Send + 'static,
    ) -> io::Result<Self> {
        fs::create_dir_all(&directory)?;
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))?;
        listener.set_nonblocking(true)?;
        let mut token_bytes = [0; 32];
        getrandom::fill(&mut token_bytes).map_err(|error| io::Error::other(error.to_string()))?;
        let token = token_bytes.iter().map(|byte| format!("{byte:02x}")).collect::<String>();
        let instance_id = format!(
            "{}-{}-{}",
            std::process::id(),
            INSTANCE_SEQUENCE.fetch_add(1, Ordering::Relaxed),
            &token[..12]
        );
        let discovery = EditorDiscovery {
            instance_id: instance_id.clone(),
            project_root: PathBuf::new(),
            endpoint: listener.local_addr()?,
            token: token.clone(),
        };
        let project_root = Arc::new(Mutex::new(None));
        let stopped = Arc::new(AtomicBool::new(false));
        let server_project = project_root.clone();
        let server_stopped = stopped.clone();
        let thread = thread::spawn(move || {
            while !server_stopped.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let result =
                            receive_request(&mut stream, &token, &server_project, &callback);
                        let _ = write_json_line(&mut stream, &result);
                    }
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(25));
                    }
                    Err(_) => break,
                }
            }
        });
        Ok(Self {
            discovery,
            discovery_path: directory.join(format!("{instance_id}.json")),
            project_root,
            stopped,
            thread: Some(thread),
        })
    }

    pub fn update_project(&self, project_root: Option<&Path>) -> io::Result<()> {
        let project_root = project_root
            .map(|project_root| canonical_directory(project_root).map_err(io::Error::other))
            .transpose()?;
        *self.project_root.lock().unwrap() = project_root.clone();
        let Some(project_root) = project_root else {
            return remove_if_present(&self.discovery_path);
        };
        let discovery = EditorDiscovery { project_root, ..self.discovery.clone() };
        let staged_path = self.discovery_path.with_extension("tmp");
        fs::write(&staged_path, serde_json::to_vec(&discovery).map_err(io::Error::other)?)?;
        if let Err(error) = fs::rename(&staged_path, &self.discovery_path) {
            if error.kind() != io::ErrorKind::AlreadyExists {
                return Err(error);
            }
            remove_if_present(&self.discovery_path)?;
            fs::rename(staged_path, &self.discovery_path)?;
        }
        Ok(())
    }
}

impl Drop for EditorServer {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Relaxed);
        let _ = remove_if_present(&self.discovery_path);
        let _ = remove_if_present(&self.discovery_path.with_extension("tmp"));
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn receive_request(
    stream: &mut TcpStream,
    token: &str,
    project_root: &Mutex<Option<PathBuf>>,
    callback: &impl Fn(EditorRequest) -> Result<EditorResponse, String>,
) -> Result<EditorResponse, String> {
    stream.set_read_timeout(Some(RPC_TIMEOUT)).map_err(|error| error.to_string())?;
    stream.set_write_timeout(Some(RPC_TIMEOUT)).map_err(|error| error.to_string())?;
    let request: RpcRequest = read_json_line(stream)?;
    if request.token != token {
        return Err("Invalid editor connection token".into());
    }
    let working_directory = canonical_directory(&request.working_directory)?;
    let project_root = project_root.lock().unwrap().clone().ok_or("Editor has no open project")?;
    ensure_scope(&working_directory, &project_root)?;
    let requested_root = match &request.request {
        EditorRequest::Ping => return Ok(EditorResponse::Pong),
        EditorRequest::RegisterChat { project_root, chat } => {
            chat.validate()?;
            project_root
        }
        EditorRequest::CanvasScreenshot { project_root }
        | EditorRequest::ReplyAnnotation { project_root, .. } => project_root,
    };
    if canonical_directory(requested_root)? != project_root {
        return Err("Editor project changed; discover editors again".into());
    }
    callback(request.request)
}

pub fn discover_editors(working_directory: &Path) -> Result<Vec<EditorDiscovery>, String> {
    discover_editors_in(&discovery_directory(), working_directory)
}

fn discover_editors_in(
    directory: &Path,
    working_directory: &Path,
) -> Result<Vec<EditorDiscovery>, String> {
    let working_directory = canonical_directory(working_directory)?;
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("Could not discover editors: {error}")),
    };
    let mut editors = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|extension| extension == "json"))
        .filter_map(|entry| fs::read(entry.path()).ok())
        .filter_map(|contents| serde_json::from_slice::<EditorDiscovery>(&contents).ok())
        .filter(|editor| {
            canonical_directory(&editor.project_root)
                .is_ok_and(|project_root| project_root.starts_with(&working_directory))
        })
        .filter(|editor| {
            editor_rpc(editor, &working_directory, EditorRequest::Ping) == Ok(EditorResponse::Pong)
        })
        .collect::<Vec<_>>();
    editors.sort_by(|first, second| first.instance_id.cmp(&second.instance_id));
    Ok(editors)
}

pub fn select_editor(
    editors: &[EditorDiscovery],
    instance_id: Option<&str>,
) -> Result<EditorDiscovery, String> {
    match instance_id {
        Some(instance_id) => {
            editors.iter().find(|editor| editor.instance_id == instance_id).cloned().ok_or_else(
                || format!("No editor within workingDirectory has instanceId {instance_id}"),
            )
        }
        None => match editors {
            [editor] => Ok(editor.clone()),
            [] => Err("No running editor within workingDirectory".into()),
            _ => {
                Err("Multiple editors match; supply instanceId from discover_visual_editors".into())
            }
        },
    }
}

pub fn editor_rpc(
    editor: &EditorDiscovery,
    working_directory: &Path,
    request: EditorRequest,
) -> Result<EditorResponse, String> {
    let working_directory = canonical_directory(working_directory)?;
    let project_root = canonical_directory(&editor.project_root)?;
    ensure_scope(&working_directory, &project_root)?;
    if !editor.endpoint.ip().is_loopback() {
        return Err("Editor endpoint must use loopback".into());
    }
    let mut stream = TcpStream::connect_timeout(&editor.endpoint, RPC_TIMEOUT)
        .map_err(|error| format!("Could not connect to editor: {error}"))?;
    stream.set_read_timeout(Some(RPC_TIMEOUT)).map_err(|error| error.to_string())?;
    stream.set_write_timeout(Some(RPC_TIMEOUT)).map_err(|error| error.to_string())?;
    write_json_line(
        &mut stream,
        &RpcRequest { token: editor.token.clone(), working_directory, request },
    )
    .map_err(|error| error.to_string())?;
    read_json_line(&mut stream)?
}

pub fn canonical_directory(directory: &Path) -> Result<PathBuf, String> {
    if !directory.is_absolute() {
        return Err("Directory must be an absolute path".into());
    }
    let directory =
        fs::canonicalize(directory).map_err(|error| format!("Invalid directory: {error}"))?;
    if !directory.is_dir() {
        return Err("Path must be a directory".into());
    }
    Ok(directory)
}

fn ensure_scope(working_directory: &Path, project_root: &Path) -> Result<(), String> {
    if !project_root.starts_with(working_directory) {
        return Err("Editor project is outside workingDirectory".into());
    }
    Ok(())
}

fn discovery_directory() -> PathBuf {
    std::env::temp_dir().join(DISCOVERY_DIRECTORY)
}

fn write_json_line(writer: &mut impl Write, value: &impl Serialize) -> io::Result<()> {
    serde_json::to_writer(&mut *writer, value).map_err(io::Error::other)?;
    writer.write_all(b"\n")?;
    writer.flush()
}

fn read_json_line<Value: serde::de::DeserializeOwned>(
    reader: &mut impl io::Read,
) -> Result<Value, String> {
    let mut line = String::new();
    BufReader::new(reader)
        .read_line(&mut line)
        .map_err(|error| format!("Editor request failed: {error}"))?;
    serde_json::from_str(&line).map_err(|error| format!("Invalid editor response: {error}"))
}

fn remove_if_present(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn start_editor(directory: &Path, project_root: &Path) -> EditorServer {
        let server = EditorServer::in_directory(directory.into(), |request| match request {
            EditorRequest::RegisterChat { chat, .. } => Ok(EditorResponse::ChatRegistered { chat }),
            EditorRequest::CanvasScreenshot { .. } => {
                Ok(EditorResponse::CanvasScreenshot { png_base64: "test-png".into() })
            }
            EditorRequest::ReplyAnnotation { annotation_id, .. } => {
                Ok(EditorResponse::AnnotationReplied { annotation_id, message_id: "2".into() })
            }
            EditorRequest::Ping => unreachable!(),
        })
        .unwrap();
        server.update_project(Some(project_root)).unwrap();
        server
    }

    fn chat() -> ChatRegistration {
        ChatRegistration {
            provider: ChatProvider::Codex,
            thread_id: "test-thread".into(),
            display_name: "Test Chat".into(),
            cli_path: std::env::current_exe().unwrap(),
        }
    }

    #[test]
    fn discovers_empty_editors_only_within_scope() {
        let directory = tempfile::tempdir().unwrap();
        let workspace = directory.path().join("workspace");
        let child = workspace.join("child");
        let sibling = directory.path().join("workspace-sibling");
        for project_root in [&workspace, &child, &sibling] {
            fs::create_dir_all(project_root).unwrap();
        }
        let editors = [&workspace, &child, &sibling]
            .map(|project_root| start_editor(directory.path(), project_root));
        let matches = discover_editors_in(directory.path(), &workspace).unwrap();
        assert_eq!(matches.len(), 2);
        assert!(matches.iter().all(|editor| editor.project_root.starts_with(&workspace)));
        let sibling_id = &editors[2].discovery.instance_id;
        assert!(select_editor(&matches, Some(sibling_id)).is_err());
        assert!(select_editor(&matches, None).unwrap_err().contains("Multiple editors"));
        assert_eq!(select_editor(&matches, Some(&matches[0].instance_id)).unwrap(), matches[0]);
    }

    #[test]
    fn registers_a_chat_and_returns_the_editor_reply() {
        let directory = tempfile::tempdir().unwrap();
        let server = start_editor(directory.path(), directory.path());
        let editor = discover_editors_in(directory.path(), directory.path()).unwrap().remove(0);
        let registered_chat = chat();
        let response = editor_rpc(
            &editor,
            directory.path(),
            EditorRequest::RegisterChat {
                project_root: directory.path().into(),
                chat: registered_chat.clone(),
            },
        )
        .unwrap();
        assert_eq!(response, EditorResponse::ChatRegistered { chat: registered_chat });
        drop(server);
        assert!(discover_editors_in(directory.path(), directory.path()).unwrap().is_empty());
    }

    #[test]
    fn rejects_wrong_token_out_of_scope_and_changed_projects() {
        let directory = tempfile::tempdir().unwrap();
        let first = directory.path().join("first");
        let second = directory.path().join("second");
        fs::create_dir(&first).unwrap();
        fs::create_dir(&second).unwrap();
        let server = start_editor(directory.path(), &first);
        let editor = discover_editors_in(directory.path(), &first).unwrap().remove(0);
        let mut invalid_token = editor.clone();
        invalid_token.token = "wrong".into();
        assert!(
            editor_rpc(&invalid_token, &first, EditorRequest::Ping).unwrap_err().contains("token")
        );
        assert!(editor_rpc(&editor, &second, EditorRequest::Ping).unwrap_err().contains("outside"));
        server.update_project(Some(&second)).unwrap();
        assert!(editor_rpc(&editor, &first, EditorRequest::Ping).unwrap_err().contains("outside"));
        assert!(
            editor_rpc(
                &editor,
                directory.path(),
                EditorRequest::RegisterChat { project_root: first, chat: chat() }
            )
            .unwrap_err()
            .contains("changed")
        );
    }

    #[test]
    fn returns_canvas_screenshot_and_rejects_changed_projects() {
        let directory = tempfile::tempdir().unwrap();
        let server = start_editor(directory.path(), directory.path());
        let editor = discover_editors_in(directory.path(), directory.path()).unwrap().remove(0);
        let request = EditorRequest::CanvasScreenshot { project_root: directory.path().into() };
        assert_eq!(
            editor_rpc(&editor, directory.path(), request.clone()).unwrap(),
            EditorResponse::CanvasScreenshot { png_base64: "test-png".into() }
        );
        let child = directory.path().join("child");
        fs::create_dir(&child).unwrap();
        server.update_project(Some(&child)).unwrap();
        assert!(editor_rpc(&editor, directory.path(), request).unwrap_err().contains("changed"));
    }

    #[test]
    fn sends_annotation_replies_with_project_scope_checks() {
        let directory = tempfile::tempdir().unwrap();
        let server = start_editor(directory.path(), directory.path());
        let editor = discover_editors_in(directory.path(), directory.path()).unwrap().remove(0);
        let request = EditorRequest::ReplyAnnotation {
            project_root: directory.path().into(),
            annotation_id: "1".into(),
            text: "Adjusted the radius.".into(),
            provider: ChatProvider::Codex,
        };
        assert_eq!(
            editor_rpc(&editor, directory.path(), request.clone()).unwrap(),
            EditorResponse::AnnotationReplied { annotation_id: "1".into(), message_id: "2".into() }
        );
        let child = directory.path().join("child");
        fs::create_dir(&child).unwrap();
        server.update_project(Some(&child)).unwrap();
        assert!(editor_rpc(&editor, directory.path(), request).unwrap_err().contains("changed"));
    }

    #[test]
    fn returns_registration_errors() {
        let directory = tempfile::tempdir().unwrap();
        let server = EditorServer::in_directory(directory.path().into(), |_| {
            Err("Registration failed".into())
        })
        .unwrap();
        server.update_project(Some(directory.path())).unwrap();
        let editor = discover_editors_in(directory.path(), directory.path()).unwrap().remove(0);
        let mut relative_cli = chat();
        relative_cli.cli_path = "codex".into();
        for (registered_chat, expected_error) in
            [(relative_cli, "absolute"), (chat(), "Registration failed")]
        {
            let error = editor_rpc(
                &editor,
                directory.path(),
                EditorRequest::RegisterChat {
                    project_root: directory.path().into(),
                    chat: registered_chat,
                },
            )
            .unwrap_err();
            assert!(error.contains(expected_error), "{error}");
        }
    }

    #[test]
    fn rejects_relative_and_missing_directories() {
        for directory in [Path::new("relative"), Path::new("/slint-editor-mcp-missing-directory")] {
            assert!(discover_editors(directory).is_err());
        }
    }

    #[cfg(unix)]
    #[test]
    fn canonicalizes_symlinks_before_scope_filtering() {
        let directory = tempfile::tempdir().unwrap();
        let workspace = directory.path().join("workspace");
        let outside = directory.path().join("outside");
        fs::create_dir(&workspace).unwrap();
        fs::create_dir(&outside).unwrap();
        let linked = workspace.join("linked");
        std::os::unix::fs::symlink(&outside, &linked).unwrap();
        let _server = start_editor(directory.path(), &linked);
        assert!(discover_editors_in(directory.path(), &workspace).unwrap().is_empty());
        assert_eq!(discover_editors_in(directory.path(), &outside).unwrap().len(), 1);
    }
}
