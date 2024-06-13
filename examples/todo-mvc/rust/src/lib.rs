// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

pub mod mvc;
pub mod ui;

mod callback;
pub use callback::*;
pub use slint::*;

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub fn main() {
    let main_window = init();

    main_window.run().unwrap();
}

fn init() -> ui::MainWindow {
    let view_handle = ui::MainWindow::new().unwrap();

    let task_list_controller = mvc::TaskListController::new(mvc::task_repo());
    ui::task_list_adapter::connect(&view_handle, task_list_controller.clone());
    ui::navigation_adapter::connect_task_list_controller(
        &view_handle,
        task_list_controller.clone(),
    );

    let create_task_controller = mvc::CreateTaskController::new(mvc::date_time_repo());
    ui::create_task_adapter::connect(&view_handle, create_task_controller.clone());
    ui::navigation_adapter::connect_create_task_controller(&view_handle, create_task_controller);
    ui::create_task_adapter::connect_task_list_controller(&view_handle, task_list_controller);

    view_handle
}

// FIXME: android example
