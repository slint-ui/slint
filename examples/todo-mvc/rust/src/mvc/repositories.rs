// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

mod mock_task_repository;
pub use mock_task_repository::*;

use crate::mvc::models::TaskModel;

pub mod traits;

pub fn task_repo() -> impl traits::TaskRepository + Clone {
    MockTaskRepository::new(vec![
        TaskModel { title: "Learn Rust".into(), done: true, due_date_time: 1717686537151 },
        TaskModel { title: "Learn Slint".into(), done: true, due_date_time: 1717686537151 },
        TaskModel {
            title: "Create project with Rust and Slint".into(),
            done: true,
            due_date_time: 1717686537151,
        },
    ])
}
