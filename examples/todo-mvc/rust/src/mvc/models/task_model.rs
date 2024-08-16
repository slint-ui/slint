// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#[derive(Clone, Default, Debug, PartialEq)]
pub struct TaskModel {
    pub title: String,

    // due date time in milliseconds
    pub due_date_time: i64,
    pub done: bool,
}
