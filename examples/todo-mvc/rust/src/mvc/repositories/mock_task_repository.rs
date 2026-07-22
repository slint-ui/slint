// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use std::{cell::RefCell, rc::Rc};

use super::traits;
use crate::mvc;

#[derive(Clone)]
pub struct MockTaskRepository {
    tasks: Rc<RefCell<Vec<mvc::TaskModel>>>,
}

impl MockTaskRepository {
    pub fn new(tasks: Vec<mvc::TaskModel>) -> Self {
        Self { tasks: Rc::new(RefCell::new(tasks)) }
    }
}

impl traits::TaskRepository for MockTaskRepository {
    fn task_count(&self) -> usize {
        self.tasks.borrow().len()
    }

    fn get_task(&self, index: usize) -> Option<mvc::TaskModel> {
        self.tasks.borrow().get(index).cloned()
    }

    fn toggle_done(&self, index: usize) -> bool {
        if let Some(task) = self.tasks.borrow_mut().get_mut(index) {
            task.done = !task.done;
            return true;
        }

        false
    }

    fn remove_task(&self, index: usize) -> bool {
        if index < self.tasks.borrow().len() {
            self.tasks.borrow_mut().remove(index);
            return true;
        }

        false
    }

    fn push_task(&self, task: mvc::TaskModel) -> bool {
        self.tasks.borrow_mut().push(task);
        true
    }
}
