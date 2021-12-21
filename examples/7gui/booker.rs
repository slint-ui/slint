// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

use chrono::NaiveDate;
use sixtyfps::SharedString;

sixtyfps::sixtyfps!(import { Booker } from "booker.60";);

pub fn main() {
    let booker = Booker::new();
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

    booker.run();
}
