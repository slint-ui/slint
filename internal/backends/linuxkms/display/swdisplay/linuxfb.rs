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
    line_length: u32,
    bpp: u32,
    presenter: Arc<crate::display::noop_presenter::NoopPresenter>,
    first_frame: Cell<bool>,
    format: drm::buffer::DrmFourcc,
    tty_fd: Option<std::fs::File>,
    original_tty_mode: Option<i32>,
}

impl LinuxFBDisplay {
    pub fn new(
        device_opener: &crate::DeviceOpener,
        renderer_formats: &[drm::buffer::DrmFourcc],
    ) -> Result<Arc<dyn super::SoftwareBufferDisplay>, PlatformError> {
        let mut fb_errors: Vec<String> = Vec::new();

        for fbnum in 0..10 {
            match Self::new_with_path(
                device_opener,
                std::path::Path::new(&format!("/dev/fb{fbnum}")),
                renderer_formats,
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
        renderer_formats: &[drm::buffer::DrmFourcc],
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

        let mut available_formats = Vec::new();

        if vinfo.bits_per_pixel == 32 {
            match (vinfo.red.offset, vinfo.green.offset, vinfo.blue.offset, vinfo.transp.offset) {
                // XRGB8888 / ARGB8888 - Red(16), Green(8), Blue(0)
                (16, 8, 0, _) => {
                    if vinfo.transp.length > 0 && vinfo.transp.offset == 24 {
                        available_formats.push(drm::buffer::DrmFourcc::Argb8888);
                    } else {
                        available_formats.push(drm::buffer::DrmFourcc::Xrgb8888);
                    }
                }
                // BGRX8888 / BGRA8888 - Blue(16), Green(8), Red(0)
                (0, 8, 16, _) => {
                    if vinfo.transp.length > 0 && vinfo.transp.offset == 24 {
                        available_formats.push(drm::buffer::DrmFourcc::Bgra8888);
                    } else {
                        available_formats.push(drm::buffer::DrmFourcc::Bgrx8888);
                    }
                }
                // RGBA8888 - Red(24), Green(16), Blue(8), Alpha(0)
                (24, 16, 8, 0) if vinfo.transp.length > 0 => {
                    available_formats.push(drm::buffer::DrmFourcc::Rgba8888);
                }
                _ => {}
            }
        } else if vinfo.bits_per_pixel == 16 {
            match (
                vinfo.red.offset,
                vinfo.red.length,
                vinfo.green.offset,
                vinfo.green.length,
                vinfo.blue.offset,
                vinfo.blue.length,
            ) {
                // RGB565: R(11-15)5, G(5-10)6, B(0-4)5
                (11, 5, 5, 6, 0, 5) => {
                    available_formats.push(drm::buffer::DrmFourcc::Rgb565);
                }
                // BGR565: B(11-15)5, G(5-10)6, R(0-4)5
                (0, 5, 5, 6, 11, 5) => {
                    available_formats.push(drm::buffer::DrmFourcc::Bgr565);
                }
                _ => {}
            }
        }

        if available_formats.is_empty() {
            return Err(format!(
                "Unsupported framebuffer format: {}-bpp with RGB layout r:{}/{} g:{}/{} b:{}/{}",
                vinfo.bits_per_pixel,
                vinfo.red.offset,
                vinfo.red.length,
                vinfo.green.offset,
                vinfo.green.length,
                vinfo.blue.offset,
                vinfo.blue.length
            )
            .into());
        }

        let format = super::negotiate_format(renderer_formats, &available_formats)
            .ok_or_else(|| PlatformError::Other(
                format!("No compatible format found for LinuxFB. Renderer supports: {:?}, FB supports: {:?}",
                        renderer_formats, available_formats).into()))?;

        let bpp = vinfo.bits_per_pixel / 8;

        let width = vinfo.xres;
        let height = vinfo.yres;
        let line_length = finfo.line_length;

        let min_line_length = width * bpp;
        if line_length < min_line_length {
            return Err(format!(
                "Error using linux framebuffer: line length ({}) is less than minimum required ({})",
                line_length, min_line_length
            ).into());
        }

        let fb_size_bytes = line_length as usize * height as usize;

        let back_buffer_size_bytes = width as usize * height as usize * bpp as usize;

        let fb = unsafe {
            memmap2::MmapOptions::new()
                .len(fb_size_bytes)
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

        let back_buffer = RefCell::new(vec![0u8; back_buffer_size_bytes].into_boxed_slice());

        Ok(Arc::new(Self {
            fb: RefCell::new(fb),
            back_buffer,
            width,
            height,
            line_length,
            bpp,
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
        let state = unsafe {
            let mut state: vt_stat = std::mem::zeroed();
            vt_getstate(ctl_fd, &mut state as *mut _)
                .map_err(|errno| format!("VT_GETSTATE ioctl failed: {errno}"))?;
            state
        };

        let tty_path = format!("/dev/tty{}", state.v_active);

        // Open VT device
        let mut tty = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&tty_path)
            .map_err(|e| format!("Could not open {}: {e}", tty_path))?;

        // Save old mode
        let old_mode = unsafe {
            let mut old_mode: u32 = 0;
            kdgetmode(tty.as_raw_fd(), &mut old_mode as *mut _)
                .map_err(|errno| format!("KDGETMODE ioctl failed: {errno}"))?;
            old_mode as i32
        };

        // Switch to graphics mode

        unsafe {
            kdsetmode(tty.as_raw_fd(), KD_GRAPHICS)
                .map_err(|errno| format!("KDSETMODE KD_GRAPHICS ioctl failed: {errno}"))?;
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
                    let _ = kdsetmode(tty.as_raw_fd(), mode);
                }
            }

            let mut tty_write = tty;
            let _ = tty_write.write_all(b"\x1b[?25h"); // Show cursor
            let _ = tty_write.flush();
        }
    }

    fn copy_to_framebuffer(&self) {
        let back_buffer = self.back_buffer.borrow();
        let mut fb = self.fb.borrow_mut();

        let pixel_row_size = self.width as usize * self.bpp as usize;
        let line_length = self.line_length as usize;

        if line_length == pixel_row_size {
            fb.as_mut().copy_from_slice(&back_buffer);
        } else {
            for y in 0..self.height as usize {
                let back_buffer_offset = y * pixel_row_size;
                let fb_offset = y * line_length;

                let back_row =
                    &back_buffer[back_buffer_offset..back_buffer_offset + pixel_row_size];
                let fb_row = &mut fb.as_mut()[fb_offset..fb_offset + pixel_row_size];

                fb_row.copy_from_slice(back_row);
            }
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

        callback(self.back_buffer.borrow_mut().as_mut(), age, self.format)?;

        self.copy_to_framebuffer();

        Ok(())
    }

    fn as_presenter(self: Arc<Self>) -> Arc<dyn crate::display::Presenter> {
        self.presenter.clone()
    }
}

const KD_GRAPHICS: i32 = 0x01;
const VT_GETSTATE: u32 = 0x5603;
const KDGETMODE: u32 = 0x4B3B;
const KDSETMODE: u32 = 0x4B3A;

#[repr(C)]
#[derive(Debug)]
#[allow(non_camel_case_types)]
struct vt_stat {
    v_active: u16,
    v_signal: u16,
    v_state: u16,
}

nix::ioctl_read_bad!(kdgetmode, KDGETMODE, u32);
nix::ioctl_write_int_bad!(kdsetmode, KDSETMODE);
nix::ioctl_read_bad!(vt_getstate, VT_GETSTATE, vt_stat);

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
