// Copyright © 2026 Klarälvdalens Datakonsult AB, a KDAB Group company <info@kdab.com>, author Nathan Collins <nathan.collins@kdab.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The Slint Debugger window (#14681).

use std::cell::OnceCell;

use i_slint_editor_preview as editor_preview;
use slint::ComponentHandle;

use crate::EditorToSessionMessage;
use crate::preview::ui::{DebuggerUi, EditorUi, Preview};

pub fn connect(
    editor_ui: &EditorUi,
    session: crossbeam_channel::Sender<EditorToSessionMessage>,
    open: bool,
) {
    let debugger = DebuggerWindow::new(session);
    editor_ui.on_show_debugger(move || {
        if let Err(err) = debugger.show() {
            tracing::warn!("Failed to show the Slint Debugger window: {err}");
        }
    });
    editor_ui.set_debugger_available(true);
    if open {
        editor_ui.invoke_show_debugger();
    }
}

struct DebuggerWindow {
    ui: OnceCell<DebuggerUi>,
    session: crossbeam_channel::Sender<EditorToSessionMessage>,
}

impl DebuggerWindow {
    fn new(session: crossbeam_channel::Sender<EditorToSessionMessage>) -> Self {
        Self { ui: OnceCell::new(), session }
    }

    fn show(&self) -> Result<&DebuggerUi, slint::PlatformError> {
        use slint::winit_030::WinitWindowAccessor;

        let ui = match self.ui.get() {
            Some(ui) => ui,
            None => {
                let ui = create(&self.session)?;
                self.ui.get_or_init(|| ui)
            }
        };
        ui.show()?;
        ui.window().with_winit_window(|window| window.focus_window());
        Ok(ui)
    }
}

fn create(
    session: &crossbeam_channel::Sender<EditorToSessionMessage>,
) -> Result<DebuggerUi, slint::PlatformError> {
    let debugger = DebuggerUi::new()?;

    let run_sender = session.clone();
    debugger.global::<Preview>().on_run(move || {
        run_sender.send(EditorToSessionMessage::RunPreview).ok();
    });

    let springboard = debugger.global::<editor_preview::springboard_ui::Springboard>();
    let springboard = <editor_preview::springboard_ui::Springboard as slint::Global<
        '_,
        DebuggerUi,
    >>::as_weak(&springboard);
    session.send(EditorToSessionMessage::AttachSpringboardUi(springboard)).ok();

    // TODO: Enable starting only once the debugger knows what to run.
    debugger.set_can_start(true);
    // TODO: Report busy from the live preview client.
    debugger.set_busy(false);

    #[cfg(target_os = "macos")]
    debugger.on_perform_native_window_zoom(crate::preview::macos_titlebar::setup(&debugger));

    Ok(debugger)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attached_uis(
        receiver: &crossbeam_channel::Receiver<EditorToSessionMessage>,
    ) -> Vec<slint::Weak<editor_preview::springboard_ui::Springboard<'static>>> {
        receiver
            .try_iter()
            .filter_map(|message| match message {
                EditorToSessionMessage::AttachSpringboardUi(global) => Some(global),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn show_debugger_creates_one_window_and_reuses_it() {
        i_slint_backend_testing::init_no_event_loop();
        let editor_ui = crate::preview::ui::create_ui().unwrap();
        assert!(!editor_ui.get_debugger_available());
        let (sender, receiver) = crossbeam_channel::unbounded();

        connect(&editor_ui, sender, false);
        assert!(editor_ui.get_debugger_available());
        assert!(attached_uis(&receiver).is_empty());

        editor_ui.invoke_show_debugger();
        assert_eq!(attached_uis(&receiver).len(), 1);

        editor_ui.invoke_show_debugger();
        assert!(attached_uis(&receiver).is_empty());
    }
}
