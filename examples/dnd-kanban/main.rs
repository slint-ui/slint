// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use std::rc::Rc;

use slint::language::{DragAction, DropEvent};
use slint::{DataTransfer, VecModel};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

slint::include_modules!();

// What we attach to each `DataTransfer` via `set_user_data`. A clone of the
// `TaskData` plus the row it came from, so `can-drop` recognizes our own
// payloads and `dropped` knows what to remove on a move.
#[derive(Clone)]
struct DragPayload {
    task: TaskData,
    source_column: usize,
    source_index: usize,
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
fn main() -> Result<(), slint::PlatformError> {
    // This provides better error messages in debug mode.
    // It's disabled in release mode so it doesn't bloat up the file size.
    #[cfg(all(debug_assertions, target_arch = "wasm32"))]
    console_error_panic_hook::set_once();

    let window = MainWindow::new()?;

    let todo = Rc::new(VecModel::from(vec![
        TaskData { title: "Write release notes".into() },
        TaskData { title: "Reply to mailing list".into() },
        TaskData { title: "Triage open issues".into() },
    ]));
    let doing = Rc::new(VecModel::from(vec![
        TaskData { title: "Polish drag-and-drop example".into() },
        TaskData { title: "Review kanban PR".into() },
    ]));
    let done = Rc::new(VecModel::from(vec![TaskData { title: "Set up project skeleton".into() }]));

    window.set_todo(todo.clone().into());
    window.set_doing(doing.clone().into());
    window.set_done(done.clone().into());

    let columns = [todo, doing, done];
    let api = window.global::<Api>();

    api.on_make_data(|task, source_column, source_index| {
        let mut transfer = DataTransfer::default();
        // Plain text lets the card drop onto other apps via a native drag.
        transfer.set_plain_text(task.title.clone());
        // The in-app payload carries the full move info. It can't cross a native drag boundary,
        // but Slint restores it for drops back onto this window.
        transfer.set_user_data(Rc::new(DragPayload {
            task,
            source_column: source_column as usize,
            source_index: source_index as usize,
        }));
        transfer
    });

    api.on_can_drop(|event: DropEvent, _target, _target_index| -> DragAction {
        if event.data.user_data().and_then(|rc| rc.downcast::<DragPayload>().ok()).is_some() {
            // Our own card: accept whatever modifier the user is holding.
            return event.proposed_action;
        }
        if event.data.has_plain_text() {
            // External plain text drop: always treated as a copy.
            return DragAction::Copy;
        }
        DragAction::None
    });

    api.on_dropped({
        let columns = columns.clone();
        move |event: DropEvent, target, target_index| {
            let target = target as usize;
            if target >= columns.len() {
                return;
            }
            let target_index = target_index as usize;

            if let Some(payload) =
                event.data.user_data().and_then(|rc| rc.downcast::<DragPayload>().ok())
            {
                if event.proposed_action != DragAction::Move {
                    // Anything that isn't an explicit move is treated as a copy.
                    columns[target].insert(target_index, payload.task.clone());
                    return;
                }
                let source = payload.source_column;
                let source_index = payload.source_index;
                let mut target_index = target_index;

                if source == target {
                    // Same-column reorder. Dropping at the source's own slot,
                    // or immediately after it, is a no-op.
                    if target_index == source_index || target_index == source_index + 1 {
                        return;
                    }
                    // Removing the source shifts later rows up by one, so the
                    // target index needs to be decremented in that case.
                    let task = columns[source].remove(source_index);
                    if target_index > source_index {
                        target_index -= 1;
                    }
                    columns[target].insert(target_index, task);
                } else {
                    // Cross-column move. Source and target are independent
                    // models, so order doesn't matter for index stability.
                    columns[source].remove(source_index);
                    columns[target].insert(target_index, payload.task.clone());
                }
            } else if let Ok(text) = event.data.plain_text() {
                columns[target].insert(target_index, TaskData { title: text });
            }
        }
    });

    window.run()
}
