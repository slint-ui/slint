// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use std::rc::Rc;

use slint::*;

use crate::{
    mvc::{
        CreateTaskController, CreateTaskControllerCallbacks, DateModel, TaskListController,
        TimeModel,
    },
    ui,
};

pub fn create_controller_callbacks(view_handle: &ui::MainWindow) -> CreateTaskControllerCallbacks {
    CreateTaskControllerCallbacks {
        on_refresh: Box::new({
            let view_handle = view_handle.as_weak();

            move |create_task_model| {
                ui::CreateTaskAdapter::get(&view_handle.unwrap())
                    .set_title(create_task_model.title.into());
                ui::CreateTaskAdapter::get(&view_handle.unwrap())
                    .set_due_date(map_date_model_to_date(create_task_model.due_data));
                ui::CreateTaskAdapter::get(&view_handle.unwrap())
                    .set_due_time(map_time_model_to_time(create_task_model.due_time));
            }
        }),
        on_back: Box::new({
            let view_handle = view_handle.as_weak();

            move || {
                ui::NavigationAdapter::get(&view_handle.unwrap()).invoke_previous_page();
            }
        }),
    }
}

pub fn initialize_adapter(
    view_handle: &ui::MainWindow,
    create_task_controller: Rc<CreateTaskController>,
    task_list_controller: Rc<TaskListController>,
) {
    ui::CreateTaskAdapter::get(view_handle).on_refresh({
        let create_task_controller = create_task_controller.clone();

        move || {
            create_task_controller.refresh();
        }
    });

    ui::CreateTaskAdapter::get(view_handle).on_back({
        let create_task_controller = create_task_controller.clone();

        move || {
            create_task_controller.back();
        }
    });

    ui::CreateTaskAdapter::get(view_handle).on_date_string({
        let create_task_controller = create_task_controller.clone();

        move |date| create_task_controller.date_string(map_date_to_date_model(date))
    });

    ui::CreateTaskAdapter::get(view_handle).on_time_string({
        let create_task_controller = create_task_controller.clone();

        move |time| create_task_controller.time_string(map_time_to_time_model(time))
    });

    ui::CreateTaskAdapter::get(view_handle).on_time_stamp({
        let create_task_controller = create_task_controller.clone();

        move |date, time| {
            create_task_controller
                .time_stamp(map_date_to_date_model(date), map_time_to_time_model(time))
        }
    });

    ui::CreateTaskAdapter::get(view_handle).on_create({
        move |title, time_stamp| task_list_controller.create_task(title.as_str(), time_stamp as i64)
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
