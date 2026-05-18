// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use std::rc::Rc;

use slint::{DataTransfer, VecModel};

slint::include_modules!();

// What we attach to each `DataTransfer` via `set_user_data`. A clone of
// the `TaskData` plus the row it came from, so `source-column-of` can
// answer the .slint side and `move-task` knows what to remove on a move.
#[derive(Clone)]
struct DragPayload {
    task: TaskData,
    source_column: usize,
    source_index: usize,
}

fn main() -> Result<(), slint::PlatformError> {
    let window = MainWindow::new()?;

    let todo = Rc::new(VecModel::from(vec![
        TaskData { id: 1, title: "Write release notes".into() },
        TaskData { id: 2, title: "Reply to mailing list".into() },
        TaskData { id: 3, title: "Triage open issues".into() },
    ]));
    let doing = Rc::new(VecModel::from(vec![
        TaskData { id: 4, title: "Polish drag-and-drop example".into() },
        TaskData { id: 5, title: "Review kanban PR".into() },
    ]));
    let done =
        Rc::new(VecModel::from(vec![TaskData { id: 6, title: "Set up project skeleton".into() }]));

    window.set_todo(todo.clone().into());
    window.set_doing(doing.clone().into());
    window.set_done(done.clone().into());

    let columns = [todo, doing, done];
    let api = window.global::<Api>();

    api.on_make_data(|task, source_column, source_index| {
        let mut transfer = DataTransfer::default();
        transfer.set_user_data(Rc::new(DragPayload {
            task,
            source_column: source_column as usize,
            source_index: source_index as usize,
        }));
        transfer
    });

    api.on_source_column_of(|data| {
        data.user_data()
            .and_then(|rc| rc.downcast::<DragPayload>().ok())
            .map_or(-1, |p| p.source_column as i32)
    });

    api.on_add_task({
        let columns = columns.clone();
        move |data, target, target_index| {
            let target = target as usize;
            let Some(payload) = data.user_data().and_then(|rc| rc.downcast::<DragPayload>().ok())
            else {
                return;
            };
            if target < columns.len() {
                columns[target].insert(target_index as usize, payload.task.clone());
            }
        }
    });

    api.on_move_task({
        let columns = columns.clone();
        move |data, target, target_index| {
            let target = target as usize;
            let Some(payload) = data.user_data().and_then(|rc| rc.downcast::<DragPayload>().ok())
            else {
                return;
            };
            if target >= columns.len() {
                return;
            }
            let source = payload.source_column;
            let source_index = payload.source_index;
            let mut target_index = target_index as usize;

            if source == target {
                // Same-column reorder. Dropping at the source's own slot, or
                // immediately after it, is a no-op.
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
                // Cross-column move. Source and target are independent models,
                // so order doesn't matter for index stability.
                columns[source].remove(source_index);
                columns[target].insert(target_index, payload.task.clone());
            }
        }
    });

    window.run()
}
