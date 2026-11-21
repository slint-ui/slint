// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::editor_preview::{LspToPreviews, Result};
use i_slint_live_preview::protocol::{LspToPreviewMessage, PreviewTarget};
use std::rc::Rc;

pub async fn apply(
    server_notifier: crate::ServerNotifier,
    to_preview: Rc<LspToPreviews>,
    origin: PreviewTarget,
    label: Option<String>,
    edit: lsp_types::WorkspaceEdit,
    notify_on_completion: bool,
) {
    let result: Result<()> = async {
        let response = server_notifier
            .send_request::<lsp_types::request::ApplyWorkspaceEdit>(
                lsp_types::ApplyWorkspaceEditParams { label, edit },
            )?
            .await?;
        if response.applied {
            Ok(())
        } else {
            Err(response.failure_reason.unwrap_or("The editor rejected the edit".into()).into())
        }
    }
    .await;
    if notify_on_completion {
        to_preview.send_to_local_target(
            origin,
            &LspToPreviewMessage::WorkspaceEditFinished { applied: result.is_ok() },
        );
    }
    if let Err(error) = result {
        tracing::warn!("Failed to apply preview edit: {error}");
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use crate::editor_preview::LspToPreview;
    use i_slint_live_preview::protocol::PreviewToLspMessage;
    use std::{
        cell::RefCell,
        collections::HashMap,
        task::{Context, Poll, Waker},
    };

    struct Capture(PreviewTarget, Rc<RefCell<Vec<LspToPreviewMessage>>>);

    impl LspToPreview for Capture {
        fn preview_target(&self) -> PreviewTarget {
            self.0
        }
        fn send(&self, message: &LspToPreviewMessage) {
            self.1.borrow_mut().push(message.clone());
        }
    }

    fn check_routing(origin: PreviewTarget, selected: PreviewTarget, notify_on_completion: bool) {
        let child = Rc::new(RefCell::new(Vec::new()));
        let embedded = Rc::new(RefCell::new(Vec::new()));
        let to_preview = LspToPreviews::new(
            HashMap::from([
                (
                    PreviewTarget::ChildProcess,
                    Box::new(Capture(PreviewTarget::ChildProcess, child.clone()))
                        as Box<dyn LspToPreview>,
                ),
                (
                    PreviewTarget::EmbeddedWasm,
                    Box::new(Capture(PreviewTarget::EmbeddedWasm, embedded.clone()))
                        as Box<dyn LspToPreview>,
                ),
            ]),
            selected,
            #[cfg(feature = "preview-remote")]
            |weak| {
                Rc::new(crate::connector::remote::RemoteLspToPreview::new(
                    tokio::sync::mpsc::unbounded_channel().0,
                    weak,
                ))
            },
        )
        .unwrap();
        let (expected, other) = match origin {
            PreviewTarget::ChildProcess => (&child, &embedded),
            PreviewTarget::EmbeddedWasm => (&embedded, &child),
            PreviewTarget::Dummy => unreachable!(),
        };
        let mut ctx = crate::language::test::mock_context();
        ctx.session.replace_preview_transport(0, to_preview.clone());
        let (sender, receiver) = crossbeam_channel::unbounded();
        let queue = crate::OutgoingRequestQueue::default();
        ctx.server_notifier = crate::ServerNotifier::new(sender, queue.clone());
        let mut watcher = crate::FileWatcher::<crate::LspFileWatcherImpl>::start_with_impl(
            |_| {},
            |_| {},
            true,
            |_| Ok(crate::LspFileWatcherImpl {}),
        )
        .unwrap();
        let message = if notify_on_completion {
            PreviewToLspMessage::SendWorkspaceEdit {
                label: None,
                edit: Default::default(),
                notify_on_completion,
            }
        } else {
            serde_json::from_value(serde_json::json!({
                "SendWorkspaceEdit": {"label": null, "edit": {}}
            }))
            .unwrap()
        };
        let mut dispatch = std::pin::pin!(async {
            match origin {
                PreviewTarget::EmbeddedWasm => {
                    crate::handle_notification(
                        lsp_server::Notification::new("slint/preview_to_lsp".into(), message),
                        &mut ctx,
                        &mut watcher,
                    )
                    .await
                }
                _ => crate::handle_preview_to_lsp_message(message, origin, &mut ctx).await,
            }
        });
        assert!(matches!(
            dispatch.as_mut().poll(&mut Context::from_waker(Waker::noop())),
            Poll::Pending
        ));
        let lsp_server::Message::Request(request) = receiver.try_recv().unwrap() else {
            panic!("Expected workspace/applyEdit");
        };
        assert_eq!(request.method, "workspace/applyEdit");
        assert!(other.borrow().is_empty());
        assert!(expected.borrow().is_empty());
        to_preview
            .set_local_target(match selected {
                PreviewTarget::ChildProcess => PreviewTarget::EmbeddedWasm,
                _ => PreviewTarget::ChildProcess,
            })
            .unwrap();
        assert!(crate::complete_request(
            &queue,
            lsp_server::Response::new_ok(request.id, serde_json::json!({"applied": false}))
        ));
        i_slint_compiler::poll_ready(dispatch).unwrap();
        assert!(other.borrow().is_empty());
        if notify_on_completion {
            assert!(matches!(
                expected.borrow().as_slice(),
                [LspToPreviewMessage::WorkspaceEditFinished { applied: false }]
            ));
        } else {
            assert!(expected.borrow().is_empty());
        }
    }

    #[test]
    fn responses_follow_the_sender_before_and_after_target_changes() {
        for origin in [PreviewTarget::ChildProcess, PreviewTarget::EmbeddedWasm] {
            for selected in [PreviewTarget::ChildProcess, PreviewTarget::EmbeddedWasm] {
                check_routing(origin, selected, true);
            }
        }
    }

    #[test]
    fn old_previews_receive_no_new_messages() {
        check_routing(PreviewTarget::EmbeddedWasm, PreviewTarget::ChildProcess, false);
    }
}
