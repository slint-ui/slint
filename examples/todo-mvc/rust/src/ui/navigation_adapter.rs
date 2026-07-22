// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use slint::*;

use crate::{
    mvc::{CreateTaskController, TaskListController},
    ui,
};

// one place to implement connection between adapter (view) and controller
pub fn connect_create_task_controller(
    view_handle: &ui::MainWindow,
    controller: CreateTaskController,
) {
    controller.on_back({
        let view_handle = view_handle.as_weak();

        move || {
            view_handle.unwrap().global::<ui::NavigationAdapter>().invoke_previous_page();
        }
    });
}

// one place to implement connection between adapter (view) and controller
pub fn connect_task_list_controller(view_handle: &ui::MainWindow, controller: TaskListController) {
    controller.on_show_create_task({
        let view_handle = view_handle.as_weak();

        move || {
            view_handle.unwrap().global::<ui::NavigationAdapter>().invoke_next_page();
        }
    });
}
