// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::cell::{Cell, RefCell};
use std::fs::OpenOptions;
use std::io::Write;
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
    tty_fd: Option<std::fs::File>,
    original_tty_mode: Option<i32>,
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

    fn detect_pixel_format(vinfo: &fb_var_screeninfo) -> Result<drm::buffer::DrmFourcc, String> {
        if vinfo.grayscale != 0 {
            return Err("Grayscale framebuffers are not supported".to_string());
        }

        if vinfo.nonstd != 0 {
            return Err("Non-standard pixel formats are not supported".to_string());
        }

        match vinfo.bits_per_pixel {
            32 => Self::detect_32bpp_format(vinfo),
            24 => Self::detect_24bpp_format(vinfo),
            16 => Self::detect_16bpp_format(vinfo),
            15 => Err("15-bit framebuffers are not supported".to_string()),
            8 => Err("8-bit framebuffers are not supported".to_string()),
            _ => Err(format!("Unsupported bits per pixel: {}", vinfo.bits_per_pixel)),
        }
    }

    fn detect_32bpp_format(vinfo: &fb_var_screeninfo) -> Result<drm::buffer::DrmFourcc, String> {
        match (
            vinfo.red.offset,
            vinfo.red.length,
            vinfo.green.offset,
            vinfo.green.length,
            vinfo.blue.offset,
            vinfo.blue.length,
            vinfo.transp.length,
        ) {
            // ARGB8888: A(24-31) R(16-23) G(8-15) B(0-7)
            (16, 8, 8, 8, 0, 8, 8) if vinfo.transp.offset == 24 => {
                Ok(drm::buffer::DrmFourcc::Argb8888)
            }

            // XRGB8888: X(24-31) R(16-23) G(8-15) B(0-7)
            (16, 8, 8, 8, 0, 8, _) => Ok(drm::buffer::DrmFourcc::Xrgb8888),

            // ABGR8888: A(24-31) B(16-23) G(8-15) R(0-7)
            (0, 8, 8, 8, 16, 8, 8) if vinfo.transp.offset == 24 => {
                Ok(drm::buffer::DrmFourcc::Abgr8888)
            }

            // XBGR8888: X(24-31) B(16-23) G(8-15) R(0-7)
            (0, 8, 8, 8, 16, 8, _) => Ok(drm::buffer::DrmFourcc::Xbgr8888),

            // RGBA8888: R(24-31) G(16-23) B(8-15) A(0-7)
            (24, 8, 16, 8, 8, 8, 8) if vinfo.transp.offset == 0 => {
                Ok(drm::buffer::DrmFourcc::Rgba8888)
            }

            // BGRA8888: B(24-31) G(16-23) R(8-15) A(0-7)
            (8, 8, 16, 8, 24, 8, 8) if vinfo.transp.offset == 0 => {
                Ok(drm::buffer::DrmFourcc::Bgra8888)
            }

            _ => Err(format!(
                "Unsupported 32-bit format: R({}/{}), G({}/{}), B({}/{}), A({}/{})",
                vinfo.red.offset,
                vinfo.red.length,
                vinfo.green.offset,
                vinfo.green.length,
                vinfo.blue.offset,
                vinfo.blue.length,
                vinfo.transp.offset,
                vinfo.transp.length
            )),
        }
    }

    fn detect_24bpp_format(vinfo: &fb_var_screeninfo) -> Result<drm::buffer::DrmFourcc, String> {
        match (
            vinfo.red.offset,
            vinfo.red.length,
            vinfo.green.offset,
            vinfo.green.length,
            vinfo.blue.offset,
            vinfo.blue.length,
        ) {
            // RGB888: R(16-23) G(8-15) B(0-7)
            (16, 8, 8, 8, 0, 8) => Ok(drm::buffer::DrmFourcc::Rgb888),

            // BGR888: B(16-23) G(8-15) R(0-7)
            (0, 8, 8, 8, 16, 8) => Ok(drm::buffer::DrmFourcc::Bgr888),

            _ => Err(format!(
                "Unsupported 24-bit format: R({}/{}), G({}/{}), B({}/{})",
                vinfo.red.offset,
                vinfo.red.length,
                vinfo.green.offset,
                vinfo.green.length,
                vinfo.blue.offset,
                vinfo.blue.length
            )),
        }
    }

    fn detect_16bpp_format(vinfo: &fb_var_screeninfo) -> Result<drm::buffer::DrmFourcc, String> {
        match (
            vinfo.red.offset,
            vinfo.red.length,
            vinfo.green.offset,
            vinfo.green.length,
            vinfo.blue.offset,
            vinfo.blue.length,
        ) {
            // RGB565: R(11-15) G(5-10) B(0-4)
            (11, 5, 5, 6, 0, 5) => Ok(drm::buffer::DrmFourcc::Rgb565),

            // BGR565: B(11-15) G(5-10) R(0-4)
            (0, 5, 5, 6, 11, 5) => Ok(drm::buffer::DrmFourcc::Bgr565),

            _ => Err(format!(
                "Unsupported 16-bit format: R({}/{}), G({}/{}), B({}/{})",
                vinfo.red.offset,
                vinfo.red.length,
                vinfo.green.offset,
                vinfo.green.length,
                vinfo.blue.offset,
                vinfo.blue.length
            )),
        }
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

        let format = Self::detect_pixel_format(&vinfo)
            .map_err(|e| PlatformError::Other(format!("Error detecting pixel format: {}", e)))?;

        let bpp = match vinfo.bits_per_pixel {
            32 => 4,
            24 => 3,
            16 => 2,
            _ => {
                return Err(format!("Unsupported bits per pixel: {}", vinfo.bits_per_pixel).into());
            }
        };

        println!("Detected framebuffer format: {:?} ({}bpp)", format, vinfo.bits_per_pixel);
        println!(
            "Color channels - R: {}/{}, G: {}/{}, B: {}/{}, A: {}/{}",
            vinfo.red.offset,
            vinfo.red.length,
            vinfo.green.offset,
            vinfo.green.length,
            vinfo.blue.offset,
            vinfo.blue.length,
            vinfo.transp.offset,
            vinfo.transp.length
        );

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

        // Try to hide cursor by setting graphics mode on tty
        let (tty_fd, original_tty_mode) = match Self::setup_graphics_mode() {
            Ok((tty, mode)) => (tty, Some(mode)),
            Err(e) => {
                eprintln!("Warning: Could not set graphics mode: {}", e);
                (None, None)
            }
        };

        let back_buffer = RefCell::new(vec![0u8; size_bytes].into_boxed_slice());

        Ok(Arc::new(Self {
            fb: RefCell::new(fb),
            back_buffer,
            width,
            height,
            presenter: crate::display::noop_presenter::NoopPresenter::new(),
            first_frame: Cell::new(true),
            format,
            tty_fd,
            original_tty_mode,
        }))
    }

    fn setup_graphics_mode() -> Result<(Option<std::fs::File>, i32), String> {
        // Open control FD for VT query
        let control = OpenOptions::new()
            .read(true)
            .open("/dev/console")
            .map_err(|e| format!("Could not open /dev/console: {e}"))?;
        let ctl_fd = control.as_raw_fd();

        // Query active VT
        let mut state: vt_stat = unsafe { std::mem::zeroed() };
        if unsafe { nix::libc::ioctl(ctl_fd, VT_GETSTATE, &mut state) } < 0 {
            return Err("VT_GETSTATE ioctl failed".into());
        }
        let tty_path = format!("/dev/tty{}", state.v_active);

        // Open VT device
        let mut tty = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&tty_path)
            .map_err(|e| format!("Could not open {}: {e}", tty_path))?;

        // Save old mode
        let mut old_mode: i32 = 0;
        if unsafe { nix::libc::ioctl(tty.as_raw_fd(), KDGETMODE, &mut old_mode) } < 0 {
            return Err("KDGETMODE ioctl failed".into());
        }

        // Switch to graphics mode
        if unsafe { nix::libc::ioctl(tty.as_raw_fd(), KDSETMODE, KD_GRAPHICS) } < 0 {
            return Err("KDSETMODE KD_GRAPHICS ioctl failed".into());
        }

        // Hide cursor fallback
        let _ = tty.write_all(b"\x1b[?25l");
        let _ = tty.flush();

        Ok((Some(tty), old_mode))
    }

    fn restore_original_mode(&self) {
        if let Some(ref tty) = self.tty_fd {
            use std::os::fd::AsRawFd;

            if let Some(mode) = self.original_tty_mode {
                unsafe {
                    let _ = nix::libc::ioctl(tty.as_raw_fd(), KDSETMODE, mode);
                }
            }

            let mut tty_write = tty;
            let _ = tty_write.write_all(b"\x1b[?25h"); // Show cursor
            let _ = tty_write.flush();
        }
    }
}

impl Drop for LinuxFBDisplay {
    fn drop(&mut self) {
        self.restore_original_mode();
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

        match self.format {
            drm::buffer::DrmFourcc::Xrgb8888
            | drm::buffer::DrmFourcc::Argb8888
            | drm::buffer::DrmFourcc::Xbgr8888
            | drm::buffer::DrmFourcc::Abgr8888
            | drm::buffer::DrmFourcc::Rgba8888
            | drm::buffer::DrmFourcc::Bgra8888 => {
                // 32-bit formats
                callback(self.back_buffer.borrow_mut().as_mut(), age, self.format)?;
            }
            drm::buffer::DrmFourcc::Rgb888 | drm::buffer::DrmFourcc::Bgr888 => {
                // 24-bit formats
                callback(self.back_buffer.borrow_mut().as_mut(), age, self.format)?;
            }
            drm::buffer::DrmFourcc::Rgb565 | drm::buffer::DrmFourcc::Bgr565 => {
                // 16-bit formats
                callback(self.back_buffer.borrow_mut().as_mut(), age, self.format)?;
            }
            _ => {
                return Err(PlatformError::Other(format!(
                    "Unsupported pixel format: {:?}",
                    self.format
                )));
            }
        }

        let mut fb = self.fb.borrow_mut();
        fb.as_mut().copy_from_slice(&self.back_buffer.borrow());
        Ok(())
    }

    fn as_presenter(self: Arc<Self>) -> Arc<dyn crate::display::Presenter> {
        self.presenter.clone()
    }
}

const KD_GRAPHICS: i32 = 0x01;
const VT_GETSTATE: u64 = 0x5603;
const KDGETMODE: u64 = 0x4B3B;
const KDSETMODE: u64 = 0x4B3A;

#[repr(C)]
#[derive(Debug)]
#[allow(non_camel_case_types)]
struct vt_stat {
    v_active: u16,
    v_signal: u16,
    v_state: u16,
}

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
