// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use crate::models::{DateModel, TimeModel};

pub trait DateTimeRepository {
    fn current_date(&self) -> DateModel;
    fn current_time(&self) -> TimeModel;
    fn date_to_string(&self, date: DateModel) -> String;
    fn time_to_string(&self, time: TimeModel) -> String;
    fn time_stamp(&self, date: DateModel, time: TimeModel) -> i32;
}
