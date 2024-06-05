// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use crate::models::{DateModel, TimeModel};

use super::traits;

#[derive(Clone)]
pub struct MockDateTimeRepository {
    current_date: DateModel,
    current_time: TimeModel,
    time_stamp: i32,
}

impl MockDateTimeRepository {
    pub fn new(current_date: DateModel, current_time: TimeModel, time_stamp: i32) -> Self {
        Self { current_date, current_time, time_stamp }
    }
}

impl traits::DateTimeRepository for MockDateTimeRepository {
    fn current_date(&self) -> DateModel {
        self.current_date
    }

    fn current_time(&self) -> TimeModel {
        self.current_time
    }

    fn date_to_string(&self, date: DateModel) -> String {
        format!("{}/{}/{}", date.year, date.month, date.day)
    }

    fn time_to_string(&self, time: TimeModel) -> String {
        format!("{}:{}", time.hour, time.minute)
    }

    fn time_stamp(&self, _date: DateModel, _time: TimeModel) -> i32 {
        self.time_stamp
    }
}
