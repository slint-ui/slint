// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use std::rc::Rc;

use slint::{Model, ModelNotify, ModelTracker};

use super::TaskModel;
use crate::mvc::traits::TaskRepository;

#[derive(Clone)]
pub struct TaskListModel {
    repo: Rc<dyn TaskRepository>,
    notify: Rc<ModelNotify>,
}

impl TaskListModel {
    pub fn new(repo: impl TaskRepository + 'static) -> Self {
        Self { repo: Rc::new(repo), notify: Rc::new(Default::default()) }
    }

    pub fn toggle_done(&self, index: usize) {
        if !self.repo.toggle_done(index) {
            return;
        }

        self.notify.row_changed(index)
    }

    pub fn remove_task(&self, index: usize) {
        if !self.repo.remove_task(index) {
            return;
        }

        self.notify.row_removed(index, 1)
    }

    pub fn push_task(&self, task: TaskModel) {
        if !self.repo.push_task(task) {
            return;
        }

        self.notify.row_added(self.row_count() - 1, 1);
    }
}

impl Model for TaskListModel {
    type Data = TaskModel;

    fn row_count(&self) -> usize {
        self.repo.task_count()
    }

    fn row_data(&self, row: usize) -> Option<Self::Data> {
        self.repo.get_task(row)
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        self.notify.as_ref()
    }
}
