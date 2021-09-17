/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2021 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2021 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

use fontconfig::fontconfig;

pub fn find_families(requested_family: &str) -> Vec<String> {
    unsafe {
        let config = fontconfig::FcInitLoadConfigAndFonts();
        let family_cstr = std::ffi::CString::new(requested_family).unwrap();
        let pattern = fontconfig::FcNameParse(family_cstr.as_ptr() as *mut libc::c_uchar);
        fontconfig::FcConfigSubstitute(std::ptr::null_mut(), pattern, fontconfig::FcMatchPattern);
        fontconfig::FcDefaultSubstitute(pattern);
        let mut sort_result = fontconfig::FcResultMatch;
        let result_set =
            fontconfig::FcFontSort(config, pattern, 1, std::ptr::null_mut(), &mut sort_result);

        let mut families = Vec::new();
        for idx in 0..(*result_set).nfont {
            let mut raw_family_name = std::ptr::null_mut();
            if fontconfig::FcPatternGetString(
                *(*result_set).fonts.offset(idx as isize),
                b"family\0".as_ptr() as *const libc::c_char,
                0,
                &mut raw_family_name,
            ) != fontconfig::FcResultMatch
            {
                continue;
            }

            if raw_family_name.is_null() {
                continue;
            }
            if let Some(family_name) =
                std::ffi::CStr::from_ptr(raw_family_name as *const libc::c_char)
                    .to_str()
                    .ok()
                    .map(|raw_family_name| raw_family_name.to_owned())
            {
                families.push(family_name)
            }
        }

        fontconfig::FcFontSetDestroy(result_set);
        fontconfig::FcPatternDestroy(pattern);
        fontconfig::FcConfigDestroy(config);
        families
    }
}
