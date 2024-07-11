// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use chrono::{DateTime, Datelike};

pub fn get_day_from_datetime(date: DateTime<chrono::offset::Utc>) -> String {
    if date.day() == chrono::offset::Local::now().day() {
        // TODO: translations
        return "Today".to_string();
    }
    date.weekday().to_string()
}
