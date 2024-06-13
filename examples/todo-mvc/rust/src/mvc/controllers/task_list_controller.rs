// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use std::rc::Rc;

use slint::Model;
use slint::ModelNotify;
use slint::ModelRc;
use slint::ModelTracker;

use crate::mvc;
use crate::Callback;

#[derive(Clone)]
pub struct TaskListController {
    task_model: TaskModel,
    show_create_task_callback: Rc<Callback<(), ()>>,
}

impl TaskListController {
    pub fn new(repo: impl mvc::traits::TaskRepository + 'static) -> Self {
        Self {
            task_model: TaskModel::new(repo),
            show_create_task_callback: Rc::new(Callback::default()),
        }
    }

    pub fn task_model(&self) -> ModelRc<mvc::TaskModel> {
        ModelRc::new(self.task_model.clone())
    }

    pub fn toggle_done(&self, index: usize) {
        self.task_model.toggle_done(index)
    }

    pub fn remove_task(&self, index: usize) {
        self.task_model.remove_task(index)
    }

    pub fn create_task(&self, title: &str, due_date: i64) {
        self.task_model.push_task(mvc::TaskModel {
            title: title.into(),
            due_date,
            ..Default::default()
        })
    }

    pub fn show_create_task(&self) {
        self.show_create_task_callback.invoke(&());
    }

    pub fn on_show_create_task(&self, mut callback: impl FnMut() + 'static) {
        self.show_create_task_callback.on(move |()| {
            callback();
        });
    }
}

#[derive(Clone)]
struct TaskModel {
    repo: Rc<dyn mvc::traits::TaskRepository>,
    notify: Rc<ModelNotify>,
}

impl TaskModel {
    fn new(repo: impl mvc::traits::TaskRepository + 'static) -> Self {
        Self { repo: Rc::new(repo), notify: Rc::new(Default::default()) }
    }

    fn toggle_done(&self, index: usize) {
        if !self.repo.toggle_done(index) {
            return;
        }

        self.notify.row_changed(index)
    }

    fn remove_task(&self, index: usize) {
        if !self.repo.remove_task(index) {
            return;
        }

        self.notify.row_removed(index, 1)
    }

    fn push_task(&self, task: mvc::TaskModel) {
        if !self.repo.push_task(task) {
            return;
        }

        self.notify.row_added(self.row_count() - 1, 1);
    }
}

impl Model for TaskModel {
    type Data = mvc::TaskModel;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mvc;
    use std::cell::Cell;

    fn test_controller() -> TaskListController {
        TaskListController::new(mvc::MockTaskRepository::new(vec![
            mvc::TaskModel { title: "Item 1".into(), due_date: 1, done: true },
            mvc::TaskModel { title: "Item 2".into(), due_date: 1, done: false },
        ]))
    }

    #[test]
    fn test_tasks() {
        let controller = test_controller();
        let task_model = controller.task_model();

        assert_eq!(task_model.row_count(), 2);
        assert_eq!(
            task_model.row_data(0),
            Some(mvc::TaskModel { title: "Item 1".into(), due_date: 1, done: true },)
        );
        assert_eq!(
            task_model.row_data(1),
            Some(mvc::TaskModel { title: "Item 2".into(), due_date: 1, done: false },)
        );
    }

    #[test]
    fn test_toggle_task_checked() {
        let controller = test_controller();
        let task_model = controller.task_model();

        assert!(task_model.row_data(0).unwrap().done);
        controller.toggle_done(0);
        assert!(!task_model.row_data(0).unwrap().done);
    }

    #[test]
    fn test_remove_task() {
        let controller = test_controller();
        let task_model = controller.task_model();

        assert_eq!(task_model.row_count(), 2);
        controller.remove_task(0);
        assert_eq!(task_model.row_count(), 1);

        assert_eq!(
            task_model.row_data(0),
            Some(mvc::TaskModel { title: "Item 2".into(), due_date: 1, done: false },)
        );
    }

    #[test]
    fn test_show_create_task() {
        let controller = test_controller();

        let callback_invoked = Rc::new(Cell::new(false));

        controller.on_show_create_task({
            let callback_invoked = callback_invoked.clone();

            move || {
                callback_invoked.set(true);
            }
        });

        controller.show_create_task();

        assert!(callback_invoked.get());
    }

    #[test]
    fn test_add_task() {
        let controller = test_controller();
        let task_model = controller.task_model();

        assert_eq!(task_model.row_count(), 2);
        controller.create_task("Item 3", 3);
        assert_eq!(task_model.row_count(), 3);

        assert_eq!(
            task_model.row_data(2),
            Some(mvc::TaskModel { title: "Item 3".into(), due_date: 3, done: false },)
        );
    }
}
