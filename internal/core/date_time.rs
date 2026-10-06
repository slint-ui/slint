// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#[cfg(not(feature = "date-time-stubs"))]
use crate::SharedString;
#[cfg(all(feature = "std", not(feature = "date-time-stubs")))]
use chrono::Local;
#[cfg(not(feature = "date-time-stubs"))]
use chrono::{Datelike, NaiveDate};

// With `date-time-stubs`, chrono leaves the binary: the calendar arithmetic is done here, and
// formatting, parsing and today's date are not available.
#[cfg(feature = "date-time-stubs")]
mod stubs {
    use crate::SharedString;

    fn is_leap_year(year: i32) -> bool {
        (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
    }

    pub fn month_day_count(month: u32, year: i32) -> Option<i32> {
        Some(match month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 if is_leap_year(year) => 29,
            2 => 28,
            _ => return None,
        })
    }

    /// The weekday of the first of the month, 1 for Monday to 6 for Saturday, 0 for Sunday.
    pub fn month_offset(month: u32, year: i32) -> i32 {
        if !(1..=12).contains(&month) {
            return 0;
        }
        // Sakamoto's method for the 1st of the month: 0 for Sunday.
        const T: [i32; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
        let y = if month < 3 { year - 1 } else { year };
        (y + y.div_euclid(4) - y.div_euclid(100) + y.div_euclid(400) + T[month as usize - 1] + 1)
            .rem_euclid(7)
    }

    pub fn format_date(_format: &str, _day: u32, _month: u32, _year: i32) -> SharedString {
        SharedString::default()
    }

    pub fn parse_date(_date: &str, _format: &str) -> Option<[i32; 3]> {
        None
    }

    pub fn date_now() -> [i32; 3] {
        [-1, -1, -1]
    }
}
#[cfg(feature = "date-time-stubs")]
pub use stubs::*;

#[cfg(all(test, feature = "date-time-stubs"))]
#[test]
fn stubs_match_chrono() {
    use chrono::{Datelike, NaiveDate};
    for year in 1600..2400 {
        for month in 1..=12 {
            let first = NaiveDate::from_ymd_opt(year, month, 1).unwrap();
            let next = if month == 12 {
                NaiveDate::from_ymd_opt(year + 1, 1, 1)
            } else {
                NaiveDate::from_ymd_opt(year, month + 1, 1)
            }
            .unwrap();
            let days = next.signed_duration_since(first).num_days() as i32;
            assert_eq!(month_day_count(month, year), Some(days), "{year}-{month}");
            let offset = first.weekday().number_from_monday() as i32 % 7;
            assert_eq!(month_offset(month, year), offset, "{year}-{month}");
        }
    }
    assert_eq!(month_day_count(0, 2000), None);
    assert_eq!(month_offset(13, 2000), 0);
}

pub fn use_24_hour_format() -> bool {
    true
}

#[cfg(not(feature = "date-time-stubs"))]
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

#[cfg(not(feature = "date-time-stubs"))]
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

#[cfg(not(feature = "date-time-stubs"))]
pub fn format_date(format: &str, day: u32, month: u32, year: i32) -> SharedString {
    if let Some(date) = NaiveDate::from_ymd_opt(year, month, day) {
        return crate::format!("{}", date.format(format));
    }

    // Don't panic, this function is used only internal
    SharedString::default()
}

#[cfg(not(feature = "date-time-stubs"))]
pub fn parse_date(date: &str, format: &str) -> Option<[i32; 3]> {
    NaiveDate::parse_from_str(date, format)
        .ok()
        .map(|date| [date.day() as i32, date.month() as i32, date.year()])
}

#[cfg(all(feature = "std", not(feature = "date-time-stubs")))]
pub fn date_now() -> [i32; 3] {
    let now = Local::now().date_naive();
    [now.day() as i32, now.month() as i32, now.year()]
}

// display the today date is currently not implemented for no_std
#[cfg(all(not(feature = "std"), not(feature = "date-time-stubs")))]
pub fn date_now() -> [i32; 3] {
    [-1, -1, -1]
}

#[cfg(feature = "ffi")]
mod ffi {
    #![allow(unsafe_code)]

    use super::*;

    #[unsafe(no_mangle)]
    pub extern "C" fn slint_date_time_use_24_hour_format() -> bool {
        use_24_hour_format()
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn slint_date_time_month_day_count(month: u32, year: i32) -> i32 {
        month_day_count(month, year).unwrap_or(0)
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn slint_date_time_month_offset(month: u32, year: i32) -> i32 {
        month_offset(month, year)
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn slint_date_time_format_date(
        format: &SharedString,
        day: u32,
        month: u32,
        year: i32,
        out: &mut SharedString,
    ) {
        *out = format_date(format, day, month, year)
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn slint_date_time_date_now(d: &mut i32, m: &mut i32, y: &mut i32) {
        [*d, *m, *y] = date_now();
    }

    #[unsafe(no_mangle)]
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
