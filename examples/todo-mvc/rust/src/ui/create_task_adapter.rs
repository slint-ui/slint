// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use slint::*;

use crate::{
    mvc::{
        {CreateTaskController, TaskListController}, {DateModel, TimeModel},
    },
    ui,
};

// a helper function to make adapter and controller connection a little bit easier
fn connect_with_controller(
    view_handle: &ui::MainWindow,
    controller: &CreateTaskController,
    connect_adapter_controller: impl FnOnce(ui::CreateTaskAdapter, CreateTaskController) + 'static,
) {
    connect_adapter_controller(view_handle.global::<ui::CreateTaskAdapter>(), controller.clone());
}

// a helper function to make adapter and controller connection a little bit easier
fn connect_with_task_list_controller(
    view_handle: &ui::MainWindow,
    controller: &TaskListController,
    connect_adapter_controller: impl FnOnce(ui::CreateTaskAdapter, TaskListController) + 'static,
) {
    connect_adapter_controller(view_handle.global::<ui::CreateTaskAdapter>(), controller.clone());
}

// one place to implement connection between adapter (view) and controller
pub fn connect(view_handle: &ui::MainWindow, controller: CreateTaskController) {
    connect_with_controller(view_handle, &controller, {
        move |adapter, controller| {
            adapter.on_back(move || {
                controller.back();
            })
        }
    });

    connect_with_controller(view_handle, &controller, {
        move |adapter, controller| {
            adapter.on_current_date(move || map_date_model_to_date(controller.current_date()))
        }
    });

    connect_with_controller(view_handle, &controller, {
        move |adapter, controller| {
            adapter.on_current_time(move || map_time_model_to_time(controller.current_time()))
        }
    });

    connect_with_controller(view_handle, &controller, {
        move |adapter, controller| {
            adapter.on_date_string(move |date| {
                controller.date_string(map_date_to_date_model(date)).into()
            })
        }
    });

    connect_with_controller(view_handle, &controller, {
        move |adapter, controller| {
            adapter.on_time_string(move |time| {
                controller.time_string(map_time_to_time_model(time)).into()
            })
        }
    });

    connect_with_controller(view_handle, &controller, {
        move |adapter, controller| {
            adapter.on_time_stamp(move |date, time| {
                controller
                    .time_stamp(map_date_to_date_model(date), map_time_to_time_model(time))
                    .into()
            })
        }
    });
}

pub fn connect_task_list_controller(view_handle: &ui::MainWindow, controller: TaskListController) {
    connect_with_task_list_controller(view_handle, &controller, {
        move |adapter, controller| {
            adapter.on_create(move |title, time_stamp| {
                controller.create_task(title.as_str(), time_stamp as i64)
            })
        }
    });
}

fn map_time_model_to_time(time_model: TimeModel) -> ui::Time {
    ui::Time {
        hour: time_model.hour as i32,
        minute: time_model.minute as i32,
        second: time_model.second as i32,
    }
}

fn map_time_to_time_model(time: ui::Time) -> TimeModel {
    TimeModel { hour: time.hour as u32, minute: time.minute as u32, second: time.second as u32 }
}

fn map_date_model_to_date(date_model: DateModel) -> ui::Date {
    ui::Date { year: date_model.year, month: date_model.month as i32, day: date_model.day as i32 }
}

fn map_date_to_date_model(date: ui::Date) -> DateModel {
    DateModel { year: date.year, month: date.month as u32, day: date.day as u32 }
}
