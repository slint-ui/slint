// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

mod mock_date_time_repository;
pub use mock_date_time_repository::*;

mod mock_task_repository;
pub use mock_task_repository::*;

use crate::mvc::models::{DateModel, TaskModel, TimeModel};

pub mod traits;

pub fn date_time_repo() -> impl traits::DateTimeRepository + Clone {
    MockDateTimeRepository::new(
        DateModel { year: 2024, month: 6, day: 11 },
        TimeModel { hour: 16, minute: 43, second: 0 },
        1718183634,
    )
}

pub fn task_repo() -> impl traits::TaskRepository + Clone {
    MockTaskRepository::new(vec![
        TaskModel { title: "Learn Rust".into(), done: true, due_date: 1717686537151 },
        TaskModel { title: "Learn Slint".into(), done: true, due_date: 1717686537151 },
        TaskModel {
            title: "Create project with Rust and Slint".into(),
            done: true,
            due_date: 1717686537151,
        },
    ])
}
