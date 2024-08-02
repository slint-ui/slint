// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use mvc::TaskListController;
use slint::ComponentHandle;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

pub mod mvc;
pub mod ui;

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub fn main() {
    let main_window = init();

    main_window.run().unwrap();
}

fn init() -> ui::MainWindow {
    let view_handle = ui::MainWindow::new().unwrap();

    let task_list_controller = TaskListController::new(
        mvc::task_repo(),
        ui::task_list_adapter::create_controller_callbacks(&view_handle),
    );

    ui::task_list_adapter::initialize_adapter(&view_handle, task_list_controller.clone());

    let create_task_controller = ui::create_task_adapter::new_create_task_controller(&view_handle);

    ui::create_task_adapter::initialize_adapter(
        &view_handle,
        create_task_controller,
        task_list_controller,
    );

    view_handle
}

// FIXME: android example
