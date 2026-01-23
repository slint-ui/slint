use futures_util::{SinkExt as _, StreamExt as _, stream::SplitSink};
use serde::Serialize;
use slint::JoinHandle;
use tokio_tungstenite_wasm::{Message, WebSocketStream};

pub struct Connection {
    task_handle: Option<JoinHandle<()>>,
    sink: SplitSink<WebSocketStream, Message>,
}

impl Connection {
    pub async fn connect(host: &str, port: u16) -> anyhow::Result<Self> {
        let stream = tokio_tungstenite_wasm::connect(format!("ws://{host}:{port}/")).await?;
        let (sink, mut receiver) = stream.split();

        let task_handle = Some(slint::spawn_local(async move {
            while let Some(msg) = receiver.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        // Handle incoming text messages
                        eprintln!("Received text message: {text}");
                    }
                    Ok(Message::Binary(bin)) => {
                        // Handle incoming binary messages
                        match postcard::from_bytes::<lsp_protocol::LspToPreviewMessage>(&bin) {
                            Ok(message) => {
                                // Process the data
                            }
                            Err(err) => {
                                eprintln!("Failed to deserialize message: {err}");
                            }
                        }
                    }
                    Ok(Message::Close(_)) => {
                        // Handle connection close
                        break;
                    }
                    Err(err) => {
                        eprintln!("WebSocket error: {err}");
                        break;
                    }
                }
            }
        })?);

        Ok(Self { task_handle, sink })
    }

    pub async fn send(&mut self, data: impl Serialize) -> anyhow::Result<()> {
        let data = postcard::to_allocvec(&data)?;
        self.sink.send(Message::Binary(data.into())).await?;

        Ok(())
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        if let Some(task_handle) = self.task_handle.take() {
            task_handle.abort();
        }
    }
}
