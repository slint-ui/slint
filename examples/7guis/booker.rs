// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use chrono::NaiveDate;
use slint::SharedString;

slint::slint!(import { Booker } from "booker.slint";);

pub fn main() {
    let booker = Booker::new().unwrap();
    booker.on_validate_date(|date: SharedString| {
        NaiveDate::parse_from_str(date.as_str(), "%d.%m.%Y").is_ok()
    });
    booker.on_compare_date(|date1: SharedString, date2: SharedString| {
        let date1 = match NaiveDate::parse_from_str(date1.as_str(), "%d.%m.%Y") {
            Err(_) => return false,
            Ok(x) => x,
        };
        let date2 = match NaiveDate::parse_from_str(date2.as_str(), "%d.%m.%Y") {
            Err(_) => return false,
            Ok(x) => x,
        };
        date1 <= date2
    });

    booker.run().unwrap();
}
