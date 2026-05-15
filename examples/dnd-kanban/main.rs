// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use std::rc::Rc;

use slint::{DataTransfer, VecModel};

slint::include_modules!();

// What we attach to each `DataTransfer` via `set_user_data`. A clone of
// the `TaskData` plus where it came from, so the drop handler can move
// it without searching the source column by id.
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

    api.on_can_drop(|data, target| {
        data.user_data()
            .and_then(|rc| rc.downcast::<DragPayload>().ok())
            .is_some_and(|p| p.source_column != target as usize)
    });

    api.on_drop_task({
        let columns = columns.clone();
        move |data, target| {
            let target = target as usize;
            let Some(payload) = data.user_data().and_then(|rc| rc.downcast::<DragPayload>().ok())
            else {
                return;
            };
            if target >= columns.len() || payload.source_column == target {
                return;
            }
            // The clone we put in `user_data` is what we move; we just delete
            // the source row.
            columns[payload.source_column].remove(payload.source_index);
            columns[target].push(payload.task.clone());
        }
    });

    window.run()
}
