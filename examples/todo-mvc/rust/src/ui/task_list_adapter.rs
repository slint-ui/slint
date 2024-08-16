// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use chrono::DateTime;
use slint::*;
use std::rc::Rc;

const DATE_TIME_FMT: &str = "%a, %b %d, %Y %H:%M";

use crate::{
    mvc::{TaskListController, TaskListControllerCallbacks, TaskModel},
    ui,
};

pub fn create_controller_callbacks(view_handle: &ui::MainWindow) -> TaskListControllerCallbacks {
    TaskListControllerCallbacks {
        on_refresh: Box::new({
            let view_handle = view_handle.as_weak();

            move |task_model| {
                let Some(view) = view_handle.upgrade() else {
                    return;
                };
                ui::TaskListAdapter::get(&view)
                    .set_tasks(Rc::new(MapModel::new(task_model, map_task_to_item)).into());
            }
        }),
        on_show_create_task: Box::new({
            let view_handle = view_handle.as_weak();

            move || {
                let Some(view) = view_handle.upgrade() else {
                    return;
                };
                ui::NavigationAdapter::get(&view).invoke_next_page();
            }
        }),
    }
}

pub fn initialize_adapter(view_handle: &ui::MainWindow, controller: Rc<TaskListController>) {
    let adapter = ui::TaskListAdapter::get(view_handle);

    adapter.on_toggle_task_checked({
        let controller = controller.clone();

        move |index| {
            controller.toggle_done(index as usize);
        }
    });

    adapter.on_remove_task({
        let controller = controller.clone();

        move |index| {
            controller.remove_task(index as usize);
        }
    });

    adapter.on_show_create_task({
        let controller = controller.clone();

        move || {
            controller.show_create_task();
        }
    });
}

// maps a TaskModel (data) to a SelectionItem (ui)
fn map_task_to_item(task: TaskModel) -> ui::SelectionListViewItem {
    ui::SelectionListViewItem {
        text: task.title.into(),
        checked: task.done,
        description: DateTime::from_timestamp_millis(task.due_date_time)
            .unwrap()
            .format(DATE_TIME_FMT)
            .to_string()
            .into(),
    }
}
