// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::time::Duration;

use anyhow::{Context as _, Result};
use i_slint_springboard::{
    ClientCommand, ClientRequest, EventEnvelope, ProtocolErrorCode, RequestDecodeError, RequestId,
    ResponseEnvelope, ResponsePayload, SPRINGBOARD_PROTOCOL_VERSION, ServerEvent, ServerMessage,
    SessionError, decode_request,
};
use tokio::io::{
    AsyncBufReadExt as _, AsyncRead, AsyncWrite, AsyncWriteExt as _, BufReader, BufWriter,
};

use crate::session_driver::ProjectSessionController;

pub async fn serve(controller: ProjectSessionController) -> Result<()> {
    serve_stream(tokio::io::stdin(), tokio::io::stdout(), controller).await
}

async fn serve_stream<Reader, Writer>(
    reader: Reader,
    writer: Writer,
    mut controller: ProjectSessionController,
) -> Result<()>
where
    Reader: AsyncRead + Unpin,
    Writer: AsyncWrite + Unpin,
{
    let mut lines = BufReader::new(reader).lines();
    let mut writer = BufWriter::new(writer);
    let mut handshaken = false;
    let mut tick = tokio::time::interval(Duration::from_millis(100));

    loop {
        tokio::select! {
            line = lines.next_line() => {
                let Some(line) = line.context("Failed reading a Springboard request")? else {
                    controller.shutdown().await?;
                    return Ok(());
                };
                let request = match decode_request(&line) {
                    Ok(request) => request,
                    Err(error) => {
                        write_decode_error(&mut writer, error).await?;
                        continue;
                    }
                };
                if !handle_request(&mut writer, &mut controller, &mut handshaken, request).await? {
                    return Ok(());
                }
            }
            _ = tick.tick() => {
                controller.poll()?;
                if handshaken {
                    for event in controller.take_events() {
                        write_message(
                            &mut writer,
                            &ServerMessage::Event(EventEnvelope::new(event.into())),
                        )
                        .await?;
                    }
                }
            }
        }
    }
}

async fn handle_request<Writer>(
    writer: &mut Writer,
    controller: &mut ProjectSessionController,
    handshaken: &mut bool,
    request: ClientRequest,
) -> Result<bool>
where
    Writer: AsyncWrite + Unpin,
{
    if request.protocol_version != SPRINGBOARD_PROTOCOL_VERSION {
        write_response(
            writer,
            request.request_id,
            ResponsePayload::Error {
                code: ProtocolErrorCode::VersionMismatch,
                message: format!(
                    "Springboard protocol {} is required; the client requested {}",
                    SPRINGBOARD_PROTOCOL_VERSION, request.protocol_version
                ),
            },
        )
        .await?;
        return Ok(true);
    }

    if !*handshaken && !matches!(request.command, ClientCommand::Handshake { .. }) {
        write_response(
            writer,
            request.request_id,
            ResponsePayload::Error {
                code: ProtocolErrorCode::HandshakeRequired,
                message: "Handshake before sending Springboard commands".into(),
            },
        )
        .await?;
        return Ok(true);
    }

    match request.command {
        ClientCommand::Handshake { .. } => {
            *handshaken = true;
            write_response(writer, request.request_id, ResponsePayload::Ok).await?;
            write_message(
                writer,
                &ServerMessage::Event(EventEnvelope::new(ServerEvent::Snapshot {
                    snapshot: controller.snapshot(),
                })),
            )
            .await?;
        }
        ClientCommand::Snapshot => {
            write_response(
                writer,
                request.request_id,
                ResponsePayload::Snapshot { snapshot: controller.snapshot() },
            )
            .await?;
        }
        ClientCommand::Launch { device_id } => {
            let response = operation_response(controller.launch(&device_id).await);
            write_response(writer, request.request_id, response).await?;
            write_pending_events(writer, controller).await?;
        }
        ClientCommand::Stop { device_id } => {
            let response = operation_response(controller.stop(&device_id).await);
            write_response(writer, request.request_id, response).await?;
            write_pending_events(writer, controller).await?;
        }
        ClientCommand::Refresh { device_id } => {
            let response = operation_response(controller.refresh(&device_id));
            write_response(writer, request.request_id, response).await?;
            write_pending_events(writer, controller).await?;
        }
        ClientCommand::AddManualDevice { .. } => {
            write_response(
                writer,
                request.request_id,
                ResponsePayload::Error {
                    code: ProtocolErrorCode::Internal,
                    message: "Manual remote devices require remote viewer support".into(),
                },
            )
            .await?;
        }
        ClientCommand::Shutdown => {
            controller.shutdown().await?;
            write_response(writer, request.request_id, ResponsePayload::Ok).await?;
            write_message(writer, &ServerMessage::Event(EventEnvelope::new(ServerEvent::Shutdown)))
                .await?;
            return Ok(false);
        }
    }
    Ok(true)
}

fn operation_response(result: Result<()>) -> ResponsePayload {
    match result {
        Ok(()) => ResponsePayload::Ok,
        Err(error) => {
            let code = match error.downcast_ref::<SessionError>() {
                Some(SessionError::UnknownDevice(_)) => ProtocolErrorCode::UnknownDevice,
                Some(SessionError::TargetLimitReached { .. }) => {
                    ProtocolErrorCode::TargetLimitReached
                }
                _ => ProtocolErrorCode::Internal,
            };
            ResponsePayload::Error { code, message: error.to_string() }
        }
    }
}

async fn write_pending_events<Writer>(
    writer: &mut Writer,
    controller: &mut ProjectSessionController,
) -> Result<()>
where
    Writer: AsyncWrite + Unpin,
{
    for event in controller.take_events() {
        write_message(writer, &ServerMessage::Event(EventEnvelope::new(event.into()))).await?;
    }
    Ok(())
}

async fn write_decode_error<Writer>(writer: &mut Writer, error: RequestDecodeError) -> Result<()>
where
    Writer: AsyncWrite + Unpin,
{
    if let Some(request_id) = error.request_id {
        write_response(
            writer,
            request_id,
            ResponsePayload::Error { code: error.code, message: error.message },
        )
        .await
    } else {
        write_message(
            writer,
            &ServerMessage::Event(EventEnvelope::new(ServerEvent::Error {
                device_id: None,
                message: error.message,
            })),
        )
        .await
    }
}

async fn write_response<Writer>(
    writer: &mut Writer,
    request_id: RequestId,
    response: ResponsePayload,
) -> Result<()>
where
    Writer: AsyncWrite + Unpin,
{
    write_message(writer, &ServerMessage::Response(ResponseEnvelope::new(request_id, response)))
        .await
}

async fn write_message<Writer>(writer: &mut Writer, message: &ServerMessage) -> Result<()>
where
    Writer: AsyncWrite + Unpin,
{
    let mut json = serde_json::to_vec(message)?;
    json.push(b'\n');
    writer.write_all(&json).await?;
    writer.flush().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use i_slint_springboard::{
        DeviceStateStore, ProjectSnapshot, ResponsePayload, ServerEvent, project::ProjectRunTarget,
    };
    use tokio::io::{AsyncWriteExt as _, DuplexStream, duplex};

    use super::*;
    use crate::session_driver::ViewerChildCommand;

    fn controller(directory: &tempfile::TempDir) -> ProjectSessionController {
        ProjectSessionController::new(
            ProjectRunTarget {
                project_root: directory.path().into(),
                manifest_path: directory.path().join("slint.toml"),
                entry_file: directory.path().join("main.slint"),
                component: "App".into(),
            },
            DeviceStateStore::new(directory.path().join("config/devices.json")),
            ViewerChildCommand::current_executable().unwrap(),
        )
    }

    async fn start_server(
        directory: &tempfile::TempDir,
    ) -> (
        tokio::io::Lines<BufReader<tokio::io::ReadHalf<DuplexStream>>>,
        tokio::io::WriteHalf<DuplexStream>,
        tokio::task::JoinHandle<Result<()>>,
    ) {
        let (client, server) = duplex(64 * 1024);
        let (client_reader, client_writer) = tokio::io::split(client);
        let controller = controller(directory);
        let task = tokio::spawn(async move {
            let (server_reader, server_writer) = tokio::io::split(server);
            serve_stream(server_reader, server_writer, controller).await
        });
        (BufReader::new(client_reader).lines(), client_writer, task)
    }

    async fn send(writer: &mut tokio::io::WriteHalf<DuplexStream>, request: &str) {
        writer.write_all(request.as_bytes()).await.unwrap();
        writer.write_all(b"\n").await.unwrap();
    }

    async fn receive(
        lines: &mut tokio::io::Lines<BufReader<tokio::io::ReadHalf<DuplexStream>>>,
    ) -> ServerMessage {
        serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap()
    }

    async fn handshake(
        lines: &mut tokio::io::Lines<BufReader<tokio::io::ReadHalf<DuplexStream>>>,
        writer: &mut tokio::io::WriteHalf<DuplexStream>,
    ) -> ProjectSnapshot {
        send(
            writer,
            r#"{"protocol_version":1,"request_id":1,"command":"handshake","client_name":"test"}"#,
        )
        .await;
        assert!(matches!(
            receive(lines).await,
            ServerMessage::Response(ResponseEnvelope { response: ResponsePayload::Ok, .. })
        ));
        let ServerMessage::Event(EventEnvelope {
            event: ServerEvent::Snapshot { snapshot }, ..
        }) = receive(lines).await
        else {
            panic!("expected initial snapshot")
        };
        snapshot
    }

    #[tokio::test]
    async fn handshake_sends_an_initial_snapshot_and_eof_stops_the_session() {
        let directory = tempfile::tempdir().unwrap();
        let (mut lines, mut writer, task) = start_server(&directory).await;

        let snapshot = handshake(&mut lines, &mut writer).await;

        assert_eq!(snapshot.project_root, PathBuf::from(directory.path()));
        assert_eq!(snapshot.devices.len(), 1);
        drop(writer);
        drop(lines);
        task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn unknown_commands_return_correlated_errors() {
        let directory = tempfile::tempdir().unwrap();
        let (mut lines, mut writer, task) = start_server(&directory).await;
        handshake(&mut lines, &mut writer).await;

        send(&mut writer, r#"{"protocol_version":1,"request_id":9,"command":"explode"}"#).await;

        assert!(matches!(
            receive(&mut lines).await,
            ServerMessage::Response(ResponseEnvelope {
                request_id: RequestId(9),
                response: ResponsePayload::Error { code: ProtocolErrorCode::UnknownCommand, .. },
                ..
            })
        ));
        drop(writer);
        drop(lines);
        task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn version_mismatches_are_reported_before_handshake() {
        let directory = tempfile::tempdir().unwrap();
        let (mut lines, mut writer, task) = start_server(&directory).await;

        send(
            &mut writer,
            r#"{"protocol_version":99,"request_id":3,"command":"handshake","client_name":"old"}"#,
        )
        .await;

        assert!(matches!(
            receive(&mut lines).await,
            ServerMessage::Response(ResponseEnvelope {
                request_id: RequestId(3),
                response: ResponsePayload::Error { code: ProtocolErrorCode::VersionMismatch, .. },
                ..
            })
        ));
        drop(writer);
        drop(lines);
        task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn abrupt_client_exit_stops_the_server() {
        let directory = tempfile::tempdir().unwrap();
        let (client, server) = duplex(1024);
        let controller = controller(&directory);
        let task = tokio::spawn(async move {
            let (reader, writer) = tokio::io::split(server);
            serve_stream(reader, writer, controller).await
        });

        drop(client);

        tokio::time::timeout(Duration::from_secs(1), task).await.unwrap().unwrap().unwrap();
    }
}
