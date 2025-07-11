// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::cell::{Cell, RefCell};
use std::os::fd::AsRawFd;
use std::sync::Arc;

use i_slint_core::platform::PlatformError;

pub struct LinuxFBDisplay {
    fb: RefCell<memmap2::MmapMut>,
    back_buffer: RefCell<Box<[u8]>>,
    width: u32,
    height: u32,
    presenter: Arc<crate::display::noop_presenter::NoopPresenter>,
    first_frame: Cell<bool>,
    format: drm::buffer::DrmFourcc,
}

impl LinuxFBDisplay {
    pub fn new(
        device_opener: &crate::DeviceOpener,
    ) -> Result<Arc<dyn super::SoftwareBufferDisplay>, PlatformError> {
        let mut fb_errors: Vec<String> = Vec::new();

        for fbnum in 0..10 {
            match Self::new_with_path(
                device_opener,
                std::path::Path::new(&format!("/dev/fb{fbnum}")),
            ) {
                Ok(dsp) => return Ok(dsp),
                Err(e) => fb_errors.push(format!("Error using /dev/fb{fbnum}: {}", e)),
            }
        }

        Err(PlatformError::Other(format!(
            "Could not open any legacy framebuffers.\n{}",
            fb_errors.join("\n")
        )))
    }

    fn new_with_path(
        device_opener: &crate::DeviceOpener,
        path: &std::path::Path,
    ) -> Result<Arc<dyn super::SoftwareBufferDisplay>, PlatformError> {
        let fd = device_opener(path)?;

        let vinfo = unsafe {
            let mut vinfo: fb_var_screeninfo = std::mem::zeroed();
            fbioget_vscreeninfo(fd.as_raw_fd(), &mut vinfo as *mut _)
                .map_err(|errno| format!("Error reading framebuffer variable info: {errno}"))?;
            vinfo
        };

        let finfo = unsafe {
            let mut finfo: fb_fix_screeninfo = std::mem::zeroed();
            fbioget_fscreeninfo(fd.as_raw_fd(), &mut finfo as *mut _)
                .map_err(|errno| format!("Error reading framebuffer fixed info: {errno}"))?;
            finfo
        };

        let format = if vinfo.bits_per_pixel == 32 {
            drm::buffer::DrmFourcc::Xrgb8888
        } else if vinfo.bits_per_pixel == 16 {
            if vinfo.red != RGB565_EXPECTED_RED_CHANNEL
                || vinfo.green != RGB565_EXPECTED_GREEN_CHANNEL
                || vinfo.blue != RGB565_EXPECTED_BLUE_CHANNEL
            {
                return Err(format!("Error using linux framebuffer: 16-bpp framebuffer does not have expected 565 format. Found {}/{}/{}", vinfo.red.length, vinfo.green.length, vinfo.blue.length).into());
            }
            drm::buffer::DrmFourcc::Rgb565
        } else {
            return Err(format!("Error using linux framebuffer: Only 32- and 16-bpp framebuffers are supported right now, found {}", vinfo.bits_per_pixel).into());
        };

        let bpp = vinfo.bits_per_pixel / 8;

        let width = vinfo.xres;
        let height = vinfo.yres;

        if finfo.line_length != width * bpp {
            return Err(format!("Error using linux framebuffer: padded lines are not supported yet (width: {}, bpp: {}, line length: {})", width, bpp, finfo.line_length).into());
        }

        let size_bytes = width as usize * height as usize * bpp as usize;

        let fb = unsafe {
            memmap2::MmapOptions::new()
                .len(size_bytes)
                .map_mut(&fd)
                .map_err(|err| format!("Error mmapping framebuffer: {err}"))?
        };

        //eprintln!("vinfo {:#?}", vinfo);
        //eprintln!("finfo {:#?}", finfo);

        let back_buffer = RefCell::new(vec![0u8; size_bytes].into_boxed_slice());

        Ok(Arc::new(Self {
            fb: RefCell::new(fb),
            back_buffer,
            width,
            height,
            presenter: crate::display::noop_presenter::NoopPresenter::new(),
            first_frame: Cell::new(true),
            format,
        }))
    }
}

impl super::SoftwareBufferDisplay for LinuxFBDisplay {
    fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    fn map_back_buffer(
        &self,
        callback: &mut dyn FnMut(
            &'_ mut [u8],
            u8,
            drm::buffer::DrmFourcc,
        ) -> Result<(), PlatformError>,
    ) -> Result<(), PlatformError> {
        let age = if self.first_frame.get() { 0 } else { 1 };
        self.first_frame.set(false);
        callback(self.back_buffer.borrow_mut().as_mut(), age, self.format)?;

        let mut fb = self.fb.borrow_mut();
        fb.as_mut().copy_from_slice(&self.back_buffer.borrow());
        Ok(())
    }

    fn as_presenter(self: Arc<Self>) -> Arc<dyn crate::display::Presenter> {
        self.presenter.clone()
    }
}

const RGB565_EXPECTED_RED_CHANNEL: fb_bitfield =
    fb_bitfield { offset: 11, length: 5, msb_right: 0 };

const RGB565_EXPECTED_GREEN_CHANNEL: fb_bitfield =
    fb_bitfield { offset: 5, length: 6, msb_right: 0 };

const RGB565_EXPECTED_BLUE_CHANNEL: fb_bitfield =
    fb_bitfield { offset: 0, length: 5, msb_right: 0 };

const FBIOGET_VSCREENINFO: u32 = 0x4600;

#[repr(C)]
#[derive(Debug, PartialEq)]
#[allow(non_camel_case_types)]

pub struct fb_bitfield {
    offset: u32,
    length: u32,
    msb_right: u32,
}

#[repr(C)]
#[derive(Debug)]
#[allow(non_camel_case_types)]
pub struct fb_var_screeninfo {
    xres: u32,
    yres: u32,
    xres_virtual: u32,
    yres_virtual: u32,
    xoffset: u32,
    yoffset: u32,
    bits_per_pixel: u32,
    grayscale: u32,
    red: fb_bitfield,
    green: fb_bitfield,
    blue: fb_bitfield,
    transp: fb_bitfield,

    nonstd: u32,

    activate: u32,

    height: u32,
    width: u32,

    accel_flags: u32,

    pixclock: u32,
    left_margin: u32,
    right_margin: u32,
    upper_margin: u32,
    lower_margin: u32,
    hsync_len: u32,
    vsync_len: u32,
    sync: u32,
    vmode: u32,
    rotate: u32,
    colorspace: u32,
    reserved: [u32; 4],
}

nix::ioctl_read_bad!(fbioget_vscreeninfo, FBIOGET_VSCREENINFO, fb_var_screeninfo);

const FBIOGET_FSCREENINFO: u32 = 0x4602;

#[repr(C)]
#[derive(Debug)]
#[allow(non_camel_case_types)]
pub struct fb_fix_screeninfo {
    id: [u8; 16],
    smem_start: std::ffi::c_ulong,

    smem_len: u32,
    r#type: u32,
    type_aux: u32,
    visual: u32,
    xpanstep: u16,
    ypanstep: u16,
    ywrapstep: u16,
    line_length: u32,
    mmio_start: std::ffi::c_ulong,

    mmio_len: u32,
    accel: u32,

    capabilities: u16,
    reserved: [u16; 2],
}

nix::ioctl_read_bad!(fbioget_fscreeninfo, FBIOGET_FSCREENINFO, fb_fix_screeninfo);
