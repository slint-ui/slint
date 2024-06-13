// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use crate::mvc;

pub trait TaskRepository {
    fn task_count(&self) -> usize;
    fn get_task(&self, index: usize) -> Option<mvc::TaskModel>;
    fn toggle_done(&self, index: usize) -> bool;
    fn remove_task(&self, index: usize) -> bool;
    fn push_task(&self, task: mvc::TaskModel) -> bool;
}
