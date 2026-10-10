// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore CLOEXEC errno evdev GETSTATE KDGKBMODE KDSKBMODE NOCTTY TCFLSH TCIFLUSH

//! Reading keyboards through evdev doesn't stop the kernel from also feeding the keys to the
//! active virtual console, where they end up in the shell once the application exits.
//! libseat switches the console keyboard off, so this is only needed without it.

use std::fs::{File, OpenOptions};
use std::os::fd::AsRawFd;
use std::os::unix::fs::OpenOptionsExt;

use nix::fcntl::OFlag;

const VT_GETSTATE: u32 = 0x5603;
const KDGKBMODE: u32 = 0x4B44;
const KDSKBMODE: u32 = 0x4B45;
const K_OFF: i32 = 0x04;

#[repr(C)]
#[allow(non_camel_case_types)]
struct vt_stat {
    v_active: u16,
    v_signal: u16,
    v_state: u16,
}

nix::ioctl_read_bad!(vt_getstate, VT_GETSTATE, vt_stat);
nix::ioctl_read_bad!(kdgkbmode, KDGKBMODE, i32);
nix::ioctl_write_int_bad!(kdskbmode, KDSKBMODE);
nix::ioctl_write_int_bad!(tcflsh, nix::libc::TCFLSH);

/// Switches the keyboard of the active virtual console off and restores it when dropped.
pub struct ConsoleKeyboard {
    tty: File,
    original_mode: i32,
}

impl ConsoleKeyboard {
    /// Returns `None` on systems without virtual consoles.
    pub fn acquire() -> Result<Option<Self>, String> {
        let control = match open_tty("/dev/tty0") {
            Ok(control) => control,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(format!("Could not open /dev/tty0: {e}")),
        };

        let state = unsafe {
            let mut state: vt_stat = std::mem::zeroed();
            vt_getstate(control.as_raw_fd(), &mut state)
                .map_err(|errno| format!("VT_GETSTATE ioctl failed: {errno}"))?;
            state
        };

        let tty_path = format!("/dev/tty{}", state.v_active);
        let tty = open_tty(&tty_path).map_err(|e| format!("Could not open {tty_path}: {e}"))?;

        let original_mode = unsafe {
            let mut mode = 0;
            kdgkbmode(tty.as_raw_fd(), &mut mode)
                .map_err(|errno| format!("KDGKBMODE ioctl failed: {errno}"))?;
            mode
        };

        unsafe {
            kdskbmode(tty.as_raw_fd(), K_OFF)
                .map_err(|errno| format!("KDSKBMODE K_OFF ioctl failed: {errno}"))?;
        }

        Ok(Some(Self { tty, original_mode }))
    }
}

impl Drop for ConsoleKeyboard {
    fn drop(&mut self) {
        let fd = self.tty.as_raw_fd();
        unsafe {
            // Drop whatever was typed ahead before the keyboard was switched off,
            // so that it doesn't run in the shell either.
            let _ = tcflsh(fd, nix::libc::TCIFLUSH);
            if let Err(errno) = kdskbmode(fd, self.original_mode) {
                eprintln!("Warning: Could not restore the console keyboard mode: {errno}");
            }
        }
    }
}

fn open_tty(path: &str) -> std::io::Result<File> {
    OpenOptions::new()
        .custom_flags((OFlag::O_NOCTTY | OFlag::O_CLOEXEC).bits())
        .read(true)
        .write(true)
        .open(path)
}
