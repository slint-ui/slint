// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![allow(unsafe_code)]

use core::ffi::{c_char, c_int, c_uchar, c_void};

const FC_MATCH_PATTERN: c_int = 0;
const FC_RESULT_MATCH: c_int = 0;

#[repr(C)]
struct FcFontSet {
    nfont: c_int,
    sfont: c_int,
    fonts: *mut *mut c_void,
}

// This is duplicated in the slint-compiler's glyph embedding code
pub fn find_families(requested_family: &str) -> Result<Vec<String>, libloading::Error> {
    unsafe {
        let fontconfig = libloading::Library::new("libfontconfig.so.1")?;
        let fc_init_load_config_and_fonts: libloading::Symbol<
            unsafe extern "C" fn() -> *mut c_void,
        > = fontconfig
            .get(b"FcInitLoadConfigAndFonts")
            .expect("Unable to find FcInitLoadConfigAndFonts");
        let fc_name_parse: libloading::Symbol<unsafe extern "C" fn(*const c_uchar) -> *mut c_void> =
            fontconfig.get(b"FcNameParse").expect("Unable to find FcNameParse");
        let fc_config_substitute: libloading::Symbol<
            unsafe extern "C" fn(
                *mut c_void, // FcConfig *
                *mut c_void, // FcPattern *
                c_int,       // FcMatchKind
            ) -> c_int,
        > = fontconfig.get(b"FcConfigSubstitute").expect("Unable to find FcConfigSubstitute");
        let fc_default_substitute: libloading::Symbol<unsafe extern "C" fn(*mut c_void)> =
            fontconfig.get(b"FcDefaultSubstitute").expect("Unable to find FcDefaultSubstitute");
        let fc_font_sort: libloading::Symbol<
            unsafe extern "C" fn(
                *mut c_void,      // FcConfig *
                *mut c_void,      // FcPattern *
                c_int,            // FcBool trim
                *mut *mut c_void, // FcCharSet **
                *mut c_int,       // FcResult *
            ) -> *mut FcFontSet,
        > = fontconfig.get(b"FcFontSort").expect("Unable to find FcFontSort");
        let fc_pattern_get_string: libloading::Symbol<
            unsafe extern "C" fn(
                *mut c_void,       // FcPattern *
                *const c_char,     // const char *object
                c_int,             // int n
                *mut *mut c_uchar, // FcChar8 **s
            ) -> c_int,
        > = fontconfig.get(b"FcPatternGetString").expect("Unable to find FcPatternGetString");
        let fc_font_set_destroy: libloading::Symbol<unsafe extern "C" fn(*mut FcFontSet)> =
            fontconfig.get(b"FcFontSetDestroy").expect("Unable to find FcFontSetDestroy");
        let fc_pattern_destroy: libloading::Symbol<
            unsafe extern "C" fn(
                *mut c_void, // FcPattern *
            ),
        > = fontconfig.get(b"FcPatternDestroy").expect("Unable to find FcPatternDestroy");
        let fc_config_destroy: libloading::Symbol<
            unsafe extern "C" fn(
                *mut c_void, // FcConfig *
            ),
        > = fontconfig.get(b"FcConfigDestroy").expect("Unable to find FcConfigDestroy");

        let config = fc_init_load_config_and_fonts();
        let family_cstr = std::ffi::CString::new(requested_family).unwrap();
        let pattern = fc_name_parse(family_cstr.as_ptr() as *mut c_uchar);
        fc_config_substitute(std::ptr::null_mut(), pattern, FC_MATCH_PATTERN);
        fc_default_substitute(pattern);
        let mut sort_result = FC_RESULT_MATCH;
        let result_set = fc_font_sort(config, pattern, 1, std::ptr::null_mut(), &mut sort_result);

        let mut families = Vec::new();
        for idx in 0..(*result_set).nfont {
            let mut raw_family_name = std::ptr::null_mut();
            if fc_pattern_get_string(
                *(*result_set).fonts.offset(idx as isize),
                b"family\0".as_ptr() as *const c_char,
                0,
                &mut raw_family_name,
            ) != FC_RESULT_MATCH
            {
                continue;
            }

            if raw_family_name.is_null() {
                continue;
            }
            if let Some(family_name) = std::ffi::CStr::from_ptr(raw_family_name as *const c_char)
                .to_str()
                .ok()
                .map(|raw_family_name| raw_family_name.to_owned())
            {
                families.push(family_name)
            }
        }

        fc_font_set_destroy(result_set);
        fc_pattern_destroy(pattern);
        fc_config_destroy(config);
        Ok(families)
    }
}
