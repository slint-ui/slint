// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use crate::mvc;

pub trait DateTimeRepository {
    fn current_date(&self) -> mvc::DateModel;
    fn current_time(&self) -> mvc::TimeModel;
    fn date_to_string(&self, date: mvc::DateModel) -> String;
    fn time_to_string(&self, time: mvc::TimeModel) -> String;
    fn time_stamp(&self, date: mvc::DateModel, time: mvc::TimeModel) -> i32;
}
