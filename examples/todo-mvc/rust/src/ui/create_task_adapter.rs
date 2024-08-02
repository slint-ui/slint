// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use std::rc::Rc;

use chrono::{Datelike, Local, NaiveDate, NaiveDateTime, NaiveTime, Timelike};
use slint::*;

const DATE_FMT: &str = "%a, %b %d %Y";
const TIME_FMT: &str = "%R";

use crate::{
    mvc::{CreateTaskController, TaskListController},
    ui,
};

struct CreateTaskControllerAdapter {
    view_handle: Weak<ui::MainWindow>,
}

impl CreateTaskController for CreateTaskControllerAdapter {
    fn initialize(&self) {
        let Some(view) = self.view_handle.upgrade() else {
            return;
        };
        let adapter = ui::CreateTaskAdapter::get(&view);
        adapter.set_title("".into());

        let now = Local::now();

        // Current local date
        let time = now.time();
        let date = now.date_naive();

        adapter.set_due_date(ui::Date {
            year: date.year(),
            month: date.month() as i32,
            day: date.day() as i32,
        });
        adapter.set_due_time(ui::Time {
            hour: time.hour() as i32,
            minute: time.minute() as i32,
            second: time.second() as i32,
        });
    }

    fn back(&self) {
        let Some(view) = self.view_handle.upgrade() else {
            return;
        };
        ui::NavigationAdapter::get(&view).invoke_previous_page();
    }
}

pub fn new_create_task_controller(view_handle: &ui::MainWindow) -> Rc<dyn CreateTaskController> {
    Rc::new(CreateTaskControllerAdapter { view_handle: view_handle.as_weak() })
}

pub fn initialize_adapter(
    view_handle: &ui::MainWindow,
    create_task_controller: Rc<dyn CreateTaskController>,
    task_list_controller: Rc<TaskListController>,
) {
    let adapter = ui::CreateTaskAdapter::get(&view_handle);

    adapter.on_initialize({
        let create_task_controller = create_task_controller.clone();

        move || {
            create_task_controller.initialize();
        }
    });

    adapter.on_back({
        let create_task_controller = create_task_controller.clone();

        move || {
            create_task_controller.back();
        }
    });

    adapter.on_date_string(|date| map_date_to_string(date));
    adapter.on_time_string(|time| map_time_to_string(time));

    adapter.on_create({
        move |title, date, time| {
            task_list_controller.create_task(title.as_str(), time_stamp(time, date))
        }
    });
}

fn map_date_to_string(date: ui::Date) -> SharedString {
    NaiveDate::from_ymd_opt(date.year, date.month as u32, date.day as u32)
        .unwrap()
        .format(DATE_FMT)
        .to_string()
        .into()
}

fn map_time_to_string(time: ui::Time) -> SharedString {
    NaiveTime::from_hms_opt(time.hour as u32, time.minute as u32, time.second as u32)
        .unwrap()
        .format(TIME_FMT)
        .to_string()
        .into()
}

fn time_stamp(time: ui::Time, date: ui::Date) -> i64 {
    let time =
        NaiveTime::from_hms_opt(time.hour as u32, time.minute as u32, time.second as u32).unwrap();
    let date = NaiveDate::from_ymd_opt(date.year, date.month as u32, date.day as u32).unwrap();

    let date_time = NaiveDateTime::new(date, time);

    date_time.and_utc().timestamp_millis()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_date_to_string() {
        assert_eq!(
            map_date_to_string(ui::Date { day: 5, month: 6, year: 2024 }),
            SharedString::from("Wed, Jun 05 2024")
        );
    }

    #[test]
    fn test_map_time_to_string() {
        assert_eq!(
            map_time_to_string(ui::Time { hour: 11, minute: 30, second: 31 }),
            SharedString::from("11:30")
        );
    }

    #[test]
    fn test_time_stamp() {
        assert_eq!(
            time_stamp(
                ui::Time { hour: 11, minute: 30, second: 31 },
                ui::Date { day: 5, month: 6, year: 2024 }
            ),
            1717587031000
        );
    }
}
