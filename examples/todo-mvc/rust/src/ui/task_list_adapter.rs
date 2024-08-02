// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use chrono::DateTime;
use slint::*;
use std::rc::Rc;

use crate::{
    mvc::{TaskListController, TaskListControllerCallbacks, TaskModel},
    ui,
};

pub fn create_controller_callbacks(view_handle: &ui::MainWindow) -> TaskListControllerCallbacks {
    TaskListControllerCallbacks {
        on_refresh: Box::new({
            let view_handle = view_handle.as_weak();

            move |task_model| {
                ui::TaskListAdapter::get(&view_handle.unwrap())
                    .set_tasks(Rc::new(MapModel::new(task_model, map_task_to_item)).into());
            }
        }),
        on_show_create_task: Box::new({
            let view_handle = view_handle.as_weak();

            move || {
                ui::NavigationAdapter::get(&view_handle.unwrap()).invoke_next_page();
            }
        }),
    }
}

pub fn initialize_adapter(view_handle: &ui::MainWindow, controller: Rc<TaskListController>) {
    ui::TaskListAdapter::get(view_handle).on_toggle_task_checked({
        let controller = controller.clone();

        move |index| {
            controller.toggle_done(index as usize);
        }
    });

    ui::TaskListAdapter::get(view_handle).on_remove_task({
        let controller = controller.clone();

        move |index| {
            controller.remove_task(index as usize);
        }
    });

    ui::TaskListAdapter::get(view_handle).on_show_create_task({
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
        description: DateTime::from_timestamp_millis(task.due_date)
            .unwrap()
            // example: Thu, Jun 6, 2024 16:29
            .format("%a, %b %d, %Y %H:%M")
            .to_string()
            .into(),
    }
}
