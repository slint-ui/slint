// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use std::rc::Rc;

use crate::mvc::{traits::DateTimeRepository, DateModel, TimeModel};
use crate::{mvc, Callback};

#[derive(Clone)]
pub struct CreateTaskController {
    repo: Rc<dyn mvc::traits::DateTimeRepository>,
    back_callback: Rc<Callback<(), ()>>,
}

impl CreateTaskController {
    pub fn new(repo: impl DateTimeRepository + 'static) -> Self {
        Self { repo: Rc::new(repo), back_callback: Rc::new(Callback::default()) }
    }

    pub fn current_date(&self) -> DateModel {
        self.repo.current_date()
    }

    pub fn current_time(&self) -> TimeModel {
        self.repo.current_time()
    }

    pub fn date_string(&self, date_model: DateModel) -> String {
        self.repo.date_to_string(date_model)
    }

    pub fn time_string(&self, time_model: TimeModel) -> String {
        self.repo.time_to_string(time_model)
    }

    pub fn back(&self) {
        self.back_callback.invoke(&());
    }

    pub fn on_back(&self, mut callback: impl FnMut() + 'static) {
        self.back_callback.on(move |()| {
            callback();
        });
    }

    pub fn time_stamp(&self, date_model: DateModel, time_model: TimeModel) -> i32 {
        self.repo.time_stamp(date_model, time_model)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mvc::MockDateTimeRepository;
    use std::cell::Cell;

    fn test_controller() -> CreateTaskController {
        CreateTaskController::new(MockDateTimeRepository::new(
            DateModel { year: 2024, month: 6, day: 12 },
            TimeModel { hour: 13, minute: 30, second: 29 },
            15,
        ))
    }

    #[test]
    fn test_current_date() {
        let controller = test_controller();
        assert_eq!(controller.current_date(), DateModel { year: 2024, month: 6, day: 12 });
    }

    #[test]
    fn test_current_time() {
        let controller = test_controller();
        assert_eq!(controller.current_time(), TimeModel { hour: 13, minute: 30, second: 29 });
    }

    #[test]
    fn test_date_string() {
        let controller = test_controller();
        assert_eq!(
            controller.date_string(DateModel { year: 2020, month: 10, day: 5 }).as_str(),
            "2020/10/5"
        );
    }

    #[test]
    fn test_time_string() {
        let controller = test_controller();
        assert_eq!(
            controller.time_string(TimeModel { hour: 10, minute: 12, second: 55 }).as_str(),
            "10:12"
        );
    }

    #[test]
    fn test_back() {
        let controller = test_controller();

        let callback_invoked = Rc::new(Cell::new(false));

        controller.on_back({
            let callback_invoked = callback_invoked.clone();

            move || {
                callback_invoked.set(true);
            }
        });

        controller.back();

        assert!(callback_invoked.get());
    }

    #[test]
    fn test_time_stamp() {
        let controller = test_controller();

        assert_eq!(controller.time_stamp(DateModel::default(), TimeModel::default()), 15);
    }
}
