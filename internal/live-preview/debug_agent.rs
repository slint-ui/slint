// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::{collections::HashSet, path::PathBuf, rc::Weak, sync::Arc};

use i_slint_core::textlayout::sharedparley::fontique;
use lsp_types::Url;
use slint_interpreter::ComponentInstance;

use crate::inspector::InspectorOverlay;
use crate::preview_sessions::PreviewSessionEvent;
use crate::protocol::{
    DebuggerToPreviewMessage, PreviewComponent, PreviewToLsp, PreviewToLspMessage,
};

/// The part of a preview that serves the editor's tools,
/// shared by every preview that runs a [`crate::preview_sessions::PreviewSession`].
///
/// It forwards the `debug()` output to the editor, draws the editor's highlight,
/// registers the fonts the editor sends, and answers the debugger's system-testing requests.
/// Pass each [`PreviewSessionEvent`] to [`Self::handle_event()`],
/// and call [`Self::show()`] for each component instance the preview puts on screen.
pub struct DebugAgent {
    inspector: Option<InspectorOverlay>,
    highlight: Option<(Url, u32)>,
    registered_fonts: HashSet<Url>,
    to_editor: Weak<dyn PreviewToLsp>,
}

/// A [`PreviewSessionEvent`] that changes what the preview shows,
/// returned by [`DebugAgent::handle_event()`] for the preview to handle.
pub enum PreviewUpdate {
    ShowPreview { component: PreviewComponent },
    ContentsChanged,
}

impl DebugAgent {
    /// Forwards the `debug()` output to `to_editor` until the agent is dropped.
    /// The agent replaces the log message handler of the global context,
    /// so keep one agent at a time.
    pub fn new(
        to_editor: Weak<dyn PreviewToLsp>,
    ) -> Result<Self, slint_interpreter::PlatformError> {
        let log_to_editor = to_editor.clone();
        i_slint_backend_selector::with_global_context(|context| {
            context.set_log_message_handler(Some(Box::new(move |message| {
                let arguments = message.message_arguments();
                let location = message
                    .location()
                    .map(|location| (PathBuf::from(location.path), location.line, location.column));
                if let Some((file, line, column)) = &location {
                    tracing::info!(
                        "DEBUG {file}:{line}:{column}> {arguments}",
                        file = file.display()
                    );
                } else {
                    tracing::info!("DEBUG> {arguments}");
                }
                let Some(to_editor) = log_to_editor.upgrade() else { return };
                to_editor
                    .send(&PreviewToLspMessage::DebugMessage {
                        location,
                        message: arguments.to_string(),
                    })
                    .ok();
            })))
        })?;
        Ok(Self { inspector: None, highlight: None, registered_fonts: HashSet::new(), to_editor })
    }

    /// Handles `event` for `shown`, the instance on screen,
    /// unless the event changes what the preview shows.
    pub fn handle_event(
        &mut self,
        event: PreviewSessionEvent,
        shown: Option<&ComponentInstance>,
    ) -> Option<PreviewUpdate> {
        match event {
            PreviewSessionEvent::ShowPreview { component } => {
                return Some(PreviewUpdate::ShowPreview { component });
            }
            PreviewSessionEvent::ContentsChanged => return Some(PreviewUpdate::ContentsChanged),
            PreviewSessionEvent::SetUserSettings { .. } => {}
            PreviewSessionEvent::HighlightFromEditor { url, offset } => {
                self.highlight = url.map(|url| (url, offset));
                if let Some(inspector) = &self.inspector {
                    inspector.update(shown, self.highlight.as_ref());
                }
            }
            PreviewSessionEvent::RegisterFont { url, contents } => {
                if !self.registered_fonts.insert(url.clone()) {
                    tracing::debug!("Font {url} already registered, skipping");
                    return None;
                }
                let len = contents.len();
                // Wrap the already-Arc-backed bytes in a Blob without copying.
                let blob = fontique::Blob::new(Arc::new(contents));
                i_slint_backend_selector::with_global_context(|context| {
                    context.font_context().borrow_mut().register_fonts(blob);
                })
                .ok();
                tracing::debug!("Registered font {url} ({len} bytes)");
            }
            PreviewSessionEvent::Debug(DebuggerToPreviewMessage::SystemTestRequest {
                id,
                request,
            }) => {
                crate::system_testing::answer_request(id, request, self.to_editor.clone());
            }
            // #14727
            PreviewSessionEvent::Debug(DebuggerToPreviewMessage::SetInspectMode { .. }) => {}
        }
        None
    }

    /// Creates the inspector overlay in `window`, unless it exists.
    /// Call it before the first [`Self::show()`].
    pub async fn create_overlay(
        &mut self,
        window: &i_slint_core::api::Window,
    ) -> anyhow::Result<()> {
        if self.inspector.is_none() {
            self.inspector = Some(InspectorOverlay::new(window).await?);
        }
        Ok(())
    }

    /// Draws the highlight over `instance`, which the preview just put on screen,
    /// and ends the subscriptions of the debugger, which observed the instance it replaced.
    pub fn show(&self, instance: &ComponentInstance) -> anyhow::Result<()> {
        crate::system_testing::clear_subscriptions(&self.to_editor);
        let inspector =
            self.inspector.as_ref().ok_or_else(|| anyhow::anyhow!("No inspector overlay"))?;
        inspector.attach()?;
        inspector.update(Some(instance), self.highlight.as_ref());
        Ok(())
    }

    /// Removes the highlight and ends the subscriptions of the debugger
    /// before the preview takes its instance off screen.
    pub fn hide(&self) {
        crate::system_testing::clear_subscriptions(&self.to_editor);
        if let Some(inspector) = &self.inspector {
            inspector.detach();
        }
    }

    /// Forgets the highlight and the subscriptions of an editor that disconnected,
    /// and removes the highlight from `shown`, the instance on screen.
    pub fn reset(&mut self, shown: Option<&ComponentInstance>) {
        self.highlight = None;
        crate::system_testing::clear_subscriptions(&self.to_editor);
        if let Some(inspector) = &self.inspector {
            inspector.update(shown, None);
        }
    }
}

impl Drop for DebugAgent {
    fn drop(&mut self) {
        i_slint_backend_selector::with_global_context(|context| {
            context.set_log_message_handler(None);
        })
        .ok();
    }
}
