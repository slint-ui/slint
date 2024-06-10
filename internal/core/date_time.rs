// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::SharedString;
#[cfg(feature = "std")]
use chrono::Local;
use chrono::{Datelike, NaiveDate};

pub fn use_24_hour_format() -> bool {
    true
}

/// Returns the number of days in a given month
pub fn month_day_count(month: u32, year: i32) -> Option<i32> {
    Some(
        NaiveDate::from_ymd_opt(
            match month {
                12 => year + 1,
                _ => year,
            },
            match month {
                12 => 1,
                _ => month + 1,
            },
            1,
        )?
        .signed_duration_since(NaiveDate::from_ymd_opt(year, month, 1)?)
        .num_days() as i32,
    )
}

pub fn month_offset(month: u32, year: i32) -> i32 {
    if let Some(date) = NaiveDate::from_ymd_opt(year, month, 1) {
        let offset = date.weekday().number_from_monday() as i32;

        // sunday
        if offset >= 7 {
            return 0;
        }

        return offset;
    }

    // The result is only None if month == 0, it should not happen because the function is only
    // used internal and not directly by the user. So it is ok to return 0 on a None result
    0
}

pub fn format_date(format: &str, day: u32, month: u32, year: i32) -> SharedString {
    if let Some(date) = NaiveDate::from_ymd_opt(year, month, day) {
        return crate::format!("{}", date.format(format));
    }

    // Don't panic, this function is used only internal
    SharedString::default()
}

pub fn parse_date(date: &str, format: &str) -> Option<[i32; 3]> {
    NaiveDate::parse_from_str(date, format)
        .ok()
        .map(|date| [date.day() as i32, date.month() as i32, date.year()])
}

#[cfg(feature = "std")]
pub fn date_now() -> [i32; 3] {
    let now = Local::now().date_naive();
    [now.day() as i32, now.month() as i32, now.year()]
}

// display the today date is currently not implemented for no_std
#[cfg(not(feature = "std"))]
pub fn date_now() -> [i32; 3] {
    [-1, -1, -1]
}

#[cfg(feature = "ffi")]
mod ffi {
    #![allow(unsafe_code)]

    use super::*;

    #[no_mangle]
    pub extern "C" fn slint_date_time_use_24_hour_format() -> bool {
        use_24_hour_format()
    }

    #[no_mangle]
    pub extern "C" fn slint_date_time_month_day_count(month: u32, year: i32) -> i32 {
        month_day_count(month, year).unwrap_or(0)
    }

    #[no_mangle]
    pub extern "C" fn slint_date_time_month_offset(month: u32, year: i32) -> i32 {
        month_offset(month, year)
    }

    #[no_mangle]
    pub extern "C" fn slint_date_time_format_date(
        format: &SharedString,
        day: u32,
        month: u32,
        year: i32,
        out: &mut SharedString,
    ) {
        *out = format_date(format, day, month, year)
    }

    #[no_mangle]
    pub extern "C" fn slint_date_time_date_now(d: &mut i32, m: &mut i32, y: &mut i32) {
        [*d, *m, *y] = date_now();
    }

    #[no_mangle]
    pub extern "C" fn slint_date_time_parse_date(
        date: &SharedString,
        format: &SharedString,
        d: &mut i32,
        m: &mut i32,
        y: &mut i32,
    ) -> bool {
        if let Some(x) = parse_date(date, format) {
            [*d, *m, *y] = x;
            true
        } else {
            false
        }
    }
}
