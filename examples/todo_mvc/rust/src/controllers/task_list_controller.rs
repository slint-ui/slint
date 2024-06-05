// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use slint::*;
use std::rc::Rc;

use crate::models::TaskModel;
use crate::repositories::traits::TaskRepository;
use crate::Callback;

#[derive(Clone)]
pub struct TaskListController<R: TaskRepository> {
    repo: R,
    tasks: Rc<VecModel<TaskModel>>,
    show_create_task_callback: Rc<Callback<(), ()>>,
}

impl<R: TaskRepository> TaskListController<R> {
    pub fn new(repo: R) -> Self {
        let tasks = Rc::new(VecModel::default());
        tasks.extend_from_slice(repo.tasks().as_slice());

        Self { repo, tasks, show_create_task_callback: Rc::new(Callback::default()) }
    }

    pub fn tasks(&self) -> ModelRc<TaskModel> {
        self.tasks.clone().into()
    }

    pub fn toggle_task_checked(&self, index: usize) {
        self.tasks.set_row_data(index, self.repo.toggle(index));
    }

    pub fn remove_task(&self, index: usize) {
        if !self.repo.remove(index) {
            return;
        }

        self.tasks.remove(index);
    }

    pub fn show_create_task(&self) {
        self.show_create_task_callback.invoke(&());
    }

    pub fn on_show_create_task(&self, mut callback: impl FnMut() + 'static) {
        self.show_create_task_callback.on(move |()| {
            callback();
        });
    }

    pub fn add_task(&self, title: &str, due_date: i64) {
        self.tasks.push(self.repo.add_task(title, due_date));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repositories::MockTaskRepository;
    use std::cell::Cell;

    fn test_controller() -> TaskListController<MockTaskRepository> {
        TaskListController::new(MockTaskRepository::new(vec![
            TaskModel { title: "Item 1".into(), due_date: 1, checked: true },
            TaskModel { title: "Item 2".into(), due_date: 1, checked: false },
        ]))
    }

    #[test]
    fn test_tasks() {
        let controller = test_controller();
        let tasks = controller.tasks();

        assert_eq!(tasks.row_count(), 2);
        assert_eq!(
            tasks.row_data(0),
            Some(TaskModel { title: "Item 1".into(), due_date: 1, checked: true },)
        );
        assert_eq!(
            tasks.row_data(1),
            Some(TaskModel { title: "Item 2".into(), due_date: 1, checked: false },)
        );
    }

    #[test]
    fn test_toggle_task_checked() {
        let controller = test_controller();
        let tasks = controller.tasks();

        assert!(tasks.row_data(0).unwrap().checked);
        controller.toggle_task_checked(0);
        assert!(!tasks.row_data(0).unwrap().checked);
    }

    #[test]
    fn test_remove_task() {
        let controller = test_controller();
        let tasks = controller.tasks();

        assert_eq!(tasks.row_count(), 2);
        controller.remove_task(0);
        assert_eq!(tasks.row_count(), 1);

        assert_eq!(
            tasks.row_data(0),
            Some(TaskModel { title: "Item 2".into(), due_date: 1, checked: false },)
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
        let tasks = controller.tasks();

        assert_eq!(tasks.row_count(), 2);
        controller.add_task("Item 3", 3);
        assert_eq!(tasks.row_count(), 3);

        assert_eq!(
            tasks.row_data(2),
            Some(TaskModel { title: "Item 3".into(), due_date: 3, checked: false },)
        );
    }
}
