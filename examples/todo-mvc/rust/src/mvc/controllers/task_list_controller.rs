// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use std::rc::Rc;

use crate::mvc::{models::TaskListModel, traits::TaskRepository, TaskModel};

pub struct TaskListControllerCallbacks {
    pub on_refresh: Box<dyn Fn(TaskListModel)>,
    pub on_show_create_task: Box<dyn Fn()>,
}

pub struct TaskListController {
    model: TaskListModel,
    callbacks: TaskListControllerCallbacks,
}

impl TaskListController {
    pub fn new(
        repo: impl TaskRepository + 'static,
        callbacks: TaskListControllerCallbacks,
    ) -> Rc<Self> {
        let controller = Rc::new(Self { model: TaskListModel::new(repo), callbacks });
        controller.refresh();
        controller
    }

    pub fn toggle_done(&self, index: usize) {
        self.model.toggle_done(index)
    }

    pub fn remove_task(&self, index: usize) {
        self.model.remove_task(index)
    }

    pub fn create_task(&self, title: &str, due_date_time: i64) {
        self.model.push_task(TaskModel { title: title.into(), due_date_time, ..Default::default() })
    }

    pub fn show_create_task(&self) {
        (self.callbacks.on_show_create_task)();
    }

    pub fn model(&self) -> TaskListModel {
        self.model.clone()
    }

    fn refresh(&self) {
        (self.callbacks.on_refresh)(self.model.clone());
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;
    use crate::mvc;
    use ::slint::Model;

    fn test_controller(callbacks: TaskListControllerCallbacks) -> Rc<TaskListController> {
        TaskListController::new(
            mvc::MockTaskRepository::new(vec![
                mvc::TaskModel { title: "Item 1".into(), due_date_time: 1, done: true },
                mvc::TaskModel { title: "Item 2".into(), due_date_time: 1, done: false },
            ]),
            callbacks,
        )
    }

    #[test]
    fn test_toggle_task_checked() {
        let controller = test_controller(TaskListControllerCallbacks {
            on_refresh: Box::new(|_| {}),
            on_show_create_task: Box::new(|| {}),
        });
        let model = controller.model();

        assert!(model.row_data(0).unwrap().done);
        controller.toggle_done(0);
        assert!(!model.row_data(0).unwrap().done);
    }

    #[test]
    fn test_remove_task() {
        let controller = test_controller(TaskListControllerCallbacks {
            on_refresh: Box::new(|_| {}),
            on_show_create_task: Box::new(|| {}),
        });
        let model = controller.model();

        assert_eq!(model.row_count(), 2);
        controller.remove_task(0);
        assert_eq!(model.row_count(), 1);

        assert_eq!(
            model.row_data(0),
            Some(mvc::TaskModel { title: "Item 2".into(), due_date_time: 1, done: false },)
        );
    }

    #[test]
    fn test_add_task() {
        let controller = test_controller(TaskListControllerCallbacks {
            on_refresh: Box::new(|_| {}),
            on_show_create_task: Box::new(|| {}),
        });
        let model = controller.model();

        assert_eq!(model.row_count(), 2);
        controller.create_task("Item 3", 3);
        assert_eq!(model.row_count(), 3);

        assert_eq!(
            model.row_data(2),
            Some(mvc::TaskModel { title: "Item 3".into(), due_date_time: 3, done: false },)
        );
    }

    #[test]
    fn test_show_create_task() {
        let callback_invoked = Rc::new(Cell::new(false));

        let controller = test_controller(TaskListControllerCallbacks {
            on_refresh: Box::new(|_| {}),
            on_show_create_task: Box::new({
                let callback_invoked = callback_invoked.clone();

                move || {
                    callback_invoked.set(true);
                }
            }),
        });

        controller.show_create_task();

        assert!(callback_invoked.get());
    }
}
