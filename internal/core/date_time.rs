// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use alloc::rc::Rc;
use alloc::string::ToString;
use chrono::{Datelike, NaiveDate};

#[cfg(feature = "std")]
use chrono::Local;

use crate::{
    model::{ModelRc, VecModel},
    SharedString,
};

pub fn use_24_hour_format() -> bool {
    true
}

fn month_day_count(year: i32, month: u32) -> Option<i64> {
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
        .num_days(),
    )
}

pub fn month_for_date(month: u32, year: i32) -> ModelRc<ModelRc<i32>> {
    let days = Rc::new(VecModel::default());

    // The result is only None if month == 0, it should not happen because the function is only
    // used internal and not directly by the user. So it is ok to return an empty list if it
    // is none
    if let Some(count) = month_day_count(year, month) {
        for d in 1..(count + 1) {
            let day = Rc::new(VecModel::from_slice(&[d as i32, month as i32, year]));
            days.push(day.into());
        }
    }

    days.into()
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

pub fn format_date(format: &SharedString, day: u32, month: u32, year: i32) -> SharedString {
    if let Some(date) = NaiveDate::from_ymd_opt(year, month, day) {
        return date.format(format.as_str()).to_string().into();
    }

    // Don't panic, this function is used only internal
    SharedString::default()
}

pub fn valid_date(date: &str, format: &str) -> bool {
    NaiveDate::parse_from_str(date, format).is_ok()
}

pub fn parse_date(date: &str, format: &str) -> ModelRc<i32> {
    let date_model = Rc::new(VecModel::default());

    if let Ok(date) = NaiveDate::parse_from_str(date, format) {
        date_model.push(date.day() as i32);
        date_model.push(date.month() as i32);
        date_model.push(date.year());
    }

    date_model.into()
}

#[cfg(feature = "std")]
pub fn date_now() -> ModelRc<i32> {
    let now = Local::now().date_naive();
    Rc::new(VecModel::from_slice(&[now.day() as i32, now.month() as i32, now.year()])).into()
}

// display the today date is currently not implemented for no_std
#[cfg(not(feature = "std"))]
pub fn date_now() -> ModelRc<i32> {
    Rc::new(VecModel::from_slice(&[-1, -1, -1])).into()
}

pub fn week_days_short() -> ModelRc<SharedString> {
    let format = SharedString::from("%a");
    Rc::new(VecModel::from_slice(&[
        SharedString::from(&format_date(&format, 26, 5, 2024).as_str()[0..1]),
        SharedString::from(&format_date(&format, 27, 5, 2024).as_str()[0..1]),
        SharedString::from(&format_date(&format, 28, 5, 2024).as_str()[0..1]),
        SharedString::from(&format_date(&format, 29, 5, 2024).as_str()[0..1]),
        SharedString::from(&format_date(&format, 30, 5, 2024).as_str()[0..1]),
        SharedString::from(&format_date(&format, 31, 5, 2024).as_str()[0..1]),
        SharedString::from(&format_date(&format, 1, 6, 2024).as_str()[0..1]),
    ]))
    .into()
}

#[cfg(feature = "ffi")]
mod ffi {
    #![allow(unsafe_code)]

    /// Perform the translation and formatting.
    #[no_mangle]
    pub extern "C" fn slint_use_24_hour_format() -> bool {
        super::use_24_hour_format()
    }
}
