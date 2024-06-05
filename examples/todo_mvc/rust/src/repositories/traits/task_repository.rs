// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use crate::models::TaskModel;

pub trait TaskRepository {
    fn tasks(&self) -> Vec<TaskModel>;
    fn toggle(&self, index: usize) -> TaskModel;
    fn remove(&self, index: usize) -> bool;
    fn add_task(&self, title: &str, due_date: i64) -> TaskModel;
}
