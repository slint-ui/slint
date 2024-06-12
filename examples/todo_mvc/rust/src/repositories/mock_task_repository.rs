// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use std::{cell::RefCell, rc::Rc};

use super::traits;
use crate::models::TaskModel;

#[derive(Clone)]
pub struct MockTaskRepository {
    tasks: Rc<RefCell<Vec<TaskModel>>>,
}

impl MockTaskRepository {
    pub fn new(tasks: Vec<TaskModel>) -> Self {
        Self { tasks: Rc::new(RefCell::new(tasks)) }
    }
}

impl traits::TaskRepository for MockTaskRepository {
    fn tasks(&self) -> Vec<TaskModel> {
        self.tasks.as_ref().borrow().clone()
    }

    fn toggle(&self, index: usize) -> TaskModel {
        let checked = self.tasks.as_ref().borrow()[index].checked;
        self.tasks.as_ref().borrow_mut()[index].checked = !checked;
        self.tasks.as_ref().borrow()[index].clone()
    }

    fn remove(&self, index: usize) -> bool {
        if index < self.tasks.as_ref().borrow().len() {
            self.tasks.as_ref().borrow_mut().remove(index);
            return true;
        }

        false
    }

    fn add_task(&self, title: &str, due_date: i64) -> TaskModel {
        let task = TaskModel { title: title.to_string(), due_date, ..Default::default() };

        self.tasks.as_ref().borrow_mut().push(task.clone());

        task
    }
}
