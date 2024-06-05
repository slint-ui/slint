// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use slint::*;

use crate::{
    controllers::{CreateTaskController, TaskListController},
    repositories::traits::{DateTimeRepository, TaskRepository},
    ui,
};

// one place to implement connection between adapter (view) and controller
pub fn connect_create_task_controller<R: DateTimeRepository + Clone + 'static>(
    view_handle: &Weak<ui::MainWindow>,
    controller: CreateTaskController<R>,
) {
    controller.on_back({
        let view_handle = view_handle.clone();

        move || {
            view_handle.upgrade().unwrap().global::<ui::NavigationAdapter>().invoke_previous_page();
        }
    });
}

// one place to implement connection between adapter (view) and controller
pub fn connect_task_list_controller<R: TaskRepository + Clone + 'static>(
    view_handle: &Weak<ui::MainWindow>,
    controller: TaskListController<R>,
) {
    controller.on_show_create_task({
        let view_handle = view_handle.clone();

        move || {
            view_handle.upgrade().unwrap().global::<ui::NavigationAdapter>().invoke_next_page();
        }
    });
}
