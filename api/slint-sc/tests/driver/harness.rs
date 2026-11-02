// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Software-3.0

//! Helpers available to the `` ```rust `` test code of the Slint SC test cases.
//! The driver includes this module in every generated test program.

#![allow(dead_code, unused_macros)]

#[cfg(not(target_os = "none"))]
extern crate std;

#[cfg(target_os = "none")]
use slint_sc_test_sys::semihosting::{fs::File, io::Error as IoError, io::Write as _};
#[cfg(target_os = "none")]
slint_sc_test_sys::entry!(crate::main);
#[cfg(not(target_os = "none"))]
use std::{fs::File, io::Error as IoError, io::Write as _};

/// Render the component and write the screenshot to a file in the current
/// directory, for the driver to compare against the PNG reference afterwards.
/// The optional second argument distinguishes multiple screenshots of the same
/// test (e.g. `screenshot!(x, after_click)`).
macro_rules! screenshot {
    ($component:expr) => {
        crate::harness::save_screenshot(
            |buf| $component.render_rgb8(buf),
            concat!(test_name!(), ".ppm\0"),
        )?
    };
    ($component:expr, $state:ident) => {
        crate::harness::save_screenshot(
            |buf| $component.render_rgb8(buf),
            concat!(test_name!(), "-", stringify!($state), ".ppm\0"),
        )?
    };
}

/// The error type of the test's `main`.
#[derive(Debug)]
pub enum Error {
    Render(slint_sc::RenderError),
    SaveScreenshot(IoError),
}

impl From<slint_sc::RenderError> for Error {
    fn from(error: slint_sc::RenderError) -> Self {
        Self::Render(error)
    }
}

const WIDTH: u32 = 64;
const HEIGHT: u32 = 64;

/// The size every test case creates its component with.
pub const WINDOW_SIZE: slint_sc::Size = slint_sc::Size::new(WIDTH, HEIGHT);

/// Save the screenshot as the file `name`, which ends with a NUL for semihosting.
pub fn save_screenshot(
    render: impl FnOnce(&mut [u8]) -> Result<(), slint_sc::RenderError>,
    name: &str,
) -> Result<(), Error> {
    let mut buffer = [0u8; (WIDTH * HEIGHT * 3) as usize];
    render(&mut buffer)?;
    let name = core::ffi::CStr::from_bytes_with_nul(name.as_bytes()).unwrap();
    let save = || -> Result<(), IoError> {
        #[cfg(target_os = "none")]
        let mut file = File::create(name)?;
        #[cfg(not(target_os = "none"))]
        let mut file = File::create(name.to_str().unwrap())?;
        write!(file, "P6\n{WIDTH} {HEIGHT}\n255\n")?;
        file.write_all(&buffer)
    };
    save().map_err(Error::SaveScreenshot)
}
