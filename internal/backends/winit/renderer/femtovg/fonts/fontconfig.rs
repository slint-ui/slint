// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use fontconfig_sys as ffi;
use fontconfig_sys::ffi_dispatch;

#[cfg(feature = "fontconfig-dlopen")]
use ffi::statics::LIB;
#[cfg(not(feature = "fontconfig-dlopen"))]
use ffi::*;

// This is duplicated in the slint-compiler's glyph embedding code
pub fn find_families(requested_family: &str) -> Vec<String> {
    unsafe {
        let config = ffi_dispatch!(feature = "fontconfig-dlopen", LIB, FcInitLoadConfigAndFonts,);
        let family_cstr = std::ffi::CString::new(requested_family).unwrap();
        let pattern = ffi_dispatch!(
            feature = "fontconfig-dlopen",
            LIB,
            FcNameParse,
            family_cstr.as_ptr() as *mut libc::c_uchar
        );
        ffi_dispatch!(
            feature = "fontconfig-dlopen",
            LIB,
            FcConfigSubstitute,
            std::ptr::null_mut(),
            pattern,
            ffi::FcMatchPattern
        );
        ffi_dispatch!(feature = "fontconfig-dlopen", LIB, FcDefaultSubstitute, pattern);
        let mut sort_result = ffi::FcResultMatch;
        let result_set = ffi_dispatch!(
            feature = "fontconfig-dlopen",
            LIB,
            FcFontSort,
            config,
            pattern,
            1,
            std::ptr::null_mut(),
            &mut sort_result
        );

        let mut families = Vec::new();
        for idx in 0..(*result_set).nfont {
            let mut raw_family_name = std::ptr::null_mut();
            if ffi_dispatch!(
                feature = "fontconfig-dlopen",
                LIB,
                FcPatternGetString,
                *(*result_set).fonts.offset(idx as isize),
                b"family\0".as_ptr() as *const libc::c_char,
                0,
                &mut raw_family_name
            ) != ffi::FcResultMatch
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

        ffi_dispatch!(feature = "fontconfig-dlopen", LIB, FcFontSetDestroy, result_set);
        ffi_dispatch!(feature = "fontconfig-dlopen", LIB, FcPatternDestroy, pattern);
        ffi_dispatch!(feature = "fontconfig-dlopen", LIB, FcConfigDestroy, config);
        families
    }
}
