// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use chrono::DateTime;
use slint::*;
use std::rc::Rc;

use crate::{
    mvc::{TaskListController, TaskModel},
    ui,
};

// a helper function to make adapter and controller connection a little bit easier
pub fn connect_with_controller(
    view_handle: &ui::MainWindow,
    controller: &TaskListController,
    connect_adapter_controller: impl FnOnce(ui::TaskListAdapter, TaskListController) + 'static,
) {
    connect_adapter_controller(view_handle.global::<ui::TaskListAdapter>(), controller.clone());
}

// one place to implement connection between adapter (view) and controller
pub fn connect(view_handle: &ui::MainWindow, controller: TaskListController) {
    // sets a mapped list of the task items to the ui
    view_handle
        .global::<ui::TaskListAdapter>()
        .set_tasks(Rc::new(MapModel::new(controller.task_model(), map_task_to_item)).into());

    connect_with_controller(view_handle, &controller, {
        move |adapter, controller| {
            adapter.on_toggle_task_checked(move |index| {
                controller.toggle_done(index as usize);
            })
        }
    });

    connect_with_controller(view_handle, &controller, {
        move |adapter, controller| {
            adapter.on_remove_task(move |index| {
                controller.remove_task(index as usize);
            })
        }
    });

    connect_with_controller(view_handle, &controller, {
        move |adapter: ui::TaskListAdapter, controller| {
            adapter.on_show_create_task(move || {
                controller.show_create_task();
            })
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
