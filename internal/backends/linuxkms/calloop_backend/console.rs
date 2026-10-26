// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore CLOEXEC errno evdev fbcon GETSTATE KDGKBMODE KDSETMODE KDSKBMODE logind NOCTTY seatd TCFLSH TCIFLUSH
// cSpell: ignore acqsig EBUSY frsig relsig SETMODE sigaction SETMASK syscalls waitpid waitv

//! Reading keyboards through evdev doesn't stop the kernel from also feeding the keys to the
//! active virtual console, where they end up in the shell once the application exits.
//! The text console also draws into the legacy framebuffer unless it's in graphics mode.
//! libseat switches the console keyboard off and the console to graphics mode,
//! so this is only needed without it.

use std::fs::{File, OpenOptions};
use std::os::fd::AsRawFd;
use std::os::unix::fs::OpenOptionsExt;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

use nix::{fcntl::OFlag, libc};

const VT_SETMODE: u32 = 0x5602;
const VT_GETSTATE: u32 = 0x5603;
const KDSETMODE: u32 = 0x4B3A;
const KDGKBMODE: u32 = 0x4B44;
const KDSKBMODE: u32 = 0x4B45;
const KD_TEXT: i32 = 0x00;
const KD_GRAPHICS: i32 = 0x01;
const K_OFF: i32 = 0x04;
const VT_AUTO: i8 = 0;
const VT_PROCESS: i8 = 1;
const UNCHANGED: i32 = -1;
const SIGNALS: [i32; 3] = [libc::SIGINT, libc::SIGTERM, libc::SIGHUP];

#[repr(C)]
#[allow(non_camel_case_types)]
struct vt_stat {
    v_active: u16,
    v_signal: u16,
    v_state: u16,
}

#[repr(C)]
#[allow(non_camel_case_types)]
struct vt_mode {
    mode: i8,
    waitv: i8,
    relsig: i16,
    acqsig: i16,
    frsig: i16,
}

impl vt_mode {
    fn new(mode: i8) -> Self {
        Self { mode, waitv: 0, relsig: 0, acqsig: 0, frsig: 0 }
    }
}

nix::ioctl_read_bad!(vt_getstate, VT_GETSTATE, vt_stat);
nix::ioctl_write_ptr_bad!(vt_setmode, VT_SETMODE, vt_mode);
nix::ioctl_write_int_bad!(kdsetmode, KDSETMODE);
nix::ioctl_read_bad!(kdgkbmode, KDGKBMODE, i32);
nix::ioctl_write_int_bad!(kdskbmode, KDSKBMODE);
nix::ioctl_write_int_bad!(tcflsh, libc::TCFLSH);

static LOCKED: AtomicBool = AtomicBool::new(false);
static TTY_FD: AtomicI32 = AtomicI32::new(-1);
static KEYBOARD_MODE: AtomicI32 = AtomicI32::new(UNCHANGED);
static GRAPHICS_MODE: AtomicBool = AtomicBool::new(false);
static GRAPHICS_MODE_REQUESTED: AtomicBool = AtomicBool::new(false);
static PROCESS_SWITCHING: AtomicBool = AtomicBool::new(false);

pub struct Console {
    tty: File,
    handlers: [Option<libc::sigaction>; 3],
    owns_console: bool,
}

impl Console {
    /// Returns `None` on systems without virtual consoles.
    pub fn acquire(disable_keyboard: bool) -> Result<Option<Self>, String> {
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
        let console = Self::prepare(tty, disable_keyboard)
            .map_err(|e| format!("Could not prepare {tty_path}: {e}"))?;
        enter_graphics_mode();
        Ok(Some(console))
    }

    fn prepare(tty: File, disable_keyboard: bool) -> Result<Self, nix::errno::Errno> {
        let handle_signals = std::env::var_os("SLINT_KMS_NO_SIGNAL_HANDLER").is_none();
        let mut console = Self { tty, handlers: [None; 3], owns_console: false };
        let lock = ConsoleLock::new()?;
        if TTY_FD.load(Ordering::Relaxed) != -1 {
            return Err(nix::errno::Errno::EBUSY);
        }

        let fd = console.tty.as_raw_fd();
        let mut keyboard_mode = UNCHANGED;
        if disable_keyboard {
            unsafe { kdgkbmode(fd, &mut keyboard_mode)? };
        }
        TTY_FD.store(fd, Ordering::Relaxed);
        KEYBOARD_MODE.store(keyboard_mode, Ordering::Relaxed);
        console.owns_console = true;
        if handle_signals {
            console.install_handlers()?;
        }
        if disable_keyboard {
            unsafe { kdskbmode(fd, K_OFF)? };
        }
        drop(lock);
        Ok(console)
    }

    fn install_handlers(&mut self) -> Result<(), nix::errno::Errno> {
        for (signal, saved) in SIGNALS.into_iter().zip(&mut self.handlers) {
            let mut previous: libc::sigaction = unsafe { std::mem::zeroed() };
            nix::errno::Errno::result(unsafe {
                libc::sigaction(signal, std::ptr::null(), &mut previous)
            })?;
            if previous.sa_sigaction != libc::SIG_DFL {
                continue;
            }
            let mut action: libc::sigaction = unsafe { std::mem::zeroed() };
            action.sa_sigaction = handle_signal as *const () as usize;
            action.sa_mask = signal_set();
            nix::errno::Errno::result(unsafe {
                libc::sigaction(signal, &action, std::ptr::null_mut())
            })?;
            *saved = Some(previous);
        }
        Ok(())
    }
}

impl Drop for Console {
    fn drop(&mut self) {
        if !self.owns_console {
            return;
        }
        // Only valid, fixed signal numbers are used, so blocking them can't fail on Linux.
        let Ok(lock) = ConsoleLock::new() else { return };
        let result = restore(self.tty.as_raw_fd());
        TTY_FD.store(-1, Ordering::Relaxed);
        for (signal, saved) in SIGNALS.into_iter().zip(&self.handlers) {
            if let Some(saved) = saved {
                let mut current: libc::sigaction = unsafe { std::mem::zeroed() };
                unsafe {
                    if libc::sigaction(signal, std::ptr::null(), &mut current) == 0
                        && current.sa_sigaction == handle_signal as *const () as usize
                    {
                        libc::sigaction(signal, saved, std::ptr::null_mut());
                    }
                }
            }
        }
        drop(lock);
        if let Err(errno) = result {
            eprintln!("Warning: Could not restore the console mode: {errno}");
        }
    }
}

/// Acquires the [`Console`] once the keyboard or a display needs it,
/// which may be after the event loop started.
pub struct LazyConsole {
    disable_keyboard: bool,
    /// `None` until acquiring was attempted.
    console: Option<Option<Console>>,
}

impl LazyConsole {
    pub fn new(disable_keyboard: bool) -> Self {
        let mut console = Self { disable_keyboard, console: None };
        console.update();
        console
    }

    pub fn update(&mut self) {
        let needed = self.disable_keyboard || GRAPHICS_MODE_REQUESTED.load(Ordering::Relaxed);
        if self.console.is_some() || !needed {
            return;
        }
        self.console = Some(Console::acquire(self.disable_keyboard).unwrap_or_else(|e| {
            eprintln!("Warning: Could not prepare the virtual console: {e}");
            None
        }));
    }
}

/// Switches the console to graphics mode while the event loop runs,
/// so that fbcon doesn't draw over the legacy framebuffer.
#[cfg_attr(not(any(enable_skia, feature = "renderer-software")), allow(dead_code))]
pub fn request_graphics_mode() {
    GRAPHICS_MODE_REQUESTED.store(true, Ordering::Relaxed);
    enter_graphics_mode();
}

fn enter_graphics_mode() {
    if !GRAPHICS_MODE_REQUESTED.load(Ordering::Relaxed) {
        return;
    }
    let Ok(lock) = ConsoleLock::new() else { return };
    let fd = TTY_FD.load(Ordering::Relaxed);
    let result = if fd == -1 || GRAPHICS_MODE.load(Ordering::Relaxed) {
        Ok(Ok(0))
    } else {
        GRAPHICS_MODE.store(true, Ordering::Relaxed);
        unsafe { kdsetmode(fd, KD_GRAPHICS) }.map(|_| reset_on_switch_after_exit(fd))
    };
    drop(lock);
    match result {
        Err(errno) => eprintln!("Warning: Could not switch the console to graphics mode: {errno}"),
        Ok(Err(errno)) => {
            eprintln!("Warning: Could not set up the console to reset after a crash: {errno}")
        }
        Ok(Ok(_)) => {}
    }
}

/// Covers exits that skip [`restore`], such as `SIGKILL`.
/// In `VT_PROCESS` mode, the kernel sends `relsig` to the owner before a console switch.
/// Signal 0 only checks that the owner exists, so switches stay refused while it runs.
/// Once it's gone, the next switch resets the console to text mode and `VT_AUTO`.
/// Setting the mode unconditionally also takes over from a killed earlier run.
fn reset_on_switch_after_exit(fd: i32) -> Result<i32, nix::errno::Errno> {
    PROCESS_SWITCHING.store(true, Ordering::Relaxed);
    unsafe { vt_setmode(fd, &vt_mode::new(VT_PROCESS)) }
}

fn restore(fd: i32) -> Result<(), nix::errno::Errno> {
    let switching = if PROCESS_SWITCHING.swap(false, Ordering::Relaxed) {
        unsafe { vt_setmode(fd, &vt_mode::new(VT_AUTO)) }
    } else {
        Ok(0)
    };
    // Like logind and seatd, restore text mode rather than the previous mode,
    // which a killed process may have left in graphics mode.
    let display = if GRAPHICS_MODE.swap(false, Ordering::Relaxed) {
        unsafe { kdsetmode(fd, KD_TEXT) }
    } else {
        Ok(0)
    };
    let keyboard_mode = KEYBOARD_MODE.swap(UNCHANGED, Ordering::Relaxed);
    let keyboard = match keyboard_mode {
        UNCHANGED => Ok(0),
        mode => unsafe {
            // Discard type-ahead so it doesn't run in the shell after restoration.
            let _ = tcflsh(fd, libc::TCIFLUSH);
            kdskbmode(fd, mode)
        },
    };
    switching.and(display).and(keyboard).map(drop)
}

fn signal_set() -> libc::sigset_t {
    unsafe {
        let mut signals = std::mem::zeroed();
        libc::sigemptyset(&mut signals);
        for signal in SIGNALS {
            libc::sigaddset(&mut signals, signal);
        }
        signals
    }
}

fn lock_console() {
    while LOCKED.compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
        std::hint::spin_loop();
    }
}

// A handler can run on any thread. Block it on the lock owner to prevent reentrant deadlock.
// While locked, use only atomics and syscalls: another thread may hold an allocator or stdio lock.
struct ConsoleLock(libc::sigset_t);

impl ConsoleLock {
    fn new() -> Result<Self, nix::errno::Errno> {
        let mut previous = unsafe { std::mem::zeroed() };
        let error = unsafe { libc::pthread_sigmask(libc::SIG_BLOCK, &signal_set(), &mut previous) };
        if error != 0 {
            return Err(nix::errno::Errno::from_raw(error));
        }
        lock_console();
        Ok(Self(previous))
    }
}

impl Drop for ConsoleLock {
    fn drop(&mut self) {
        LOCKED.store(false, Ordering::Release);
        unsafe { libc::pthread_sigmask(libc::SIG_SETMASK, &self.0, std::ptr::null_mut()) };
    }
}

extern "C" fn handle_signal(signal: i32) {
    lock_console();
    let fd = TTY_FD.load(Ordering::Relaxed);
    if fd != -1 {
        let _ = restore(fd);
    }

    // Re-raise on this thread so waitpid reports termination by the original signal.
    // Keep the lock until termination so another thread can't change the console again.
    unsafe {
        let mut action: libc::sigaction = std::mem::zeroed();
        action.sa_sigaction = libc::SIG_DFL;
        libc::sigemptyset(&mut action.sa_mask);
        libc::sigaction(signal, &action, std::ptr::null_mut());
        libc::raise(signal);
        let mut signals = std::mem::zeroed();
        libc::sigemptyset(&mut signals);
        libc::sigaddset(&mut signals, signal);
        libc::pthread_sigmask(libc::SIG_UNBLOCK, &signals, std::ptr::null_mut());
        libc::_exit(128 + signal);
    }
}

fn open_tty(path: &str) -> std::io::Result<File> {
    OpenOptions::new()
        .custom_flags((OFlag::O_NOCTTY | OFlag::O_CLOEXEC).bits())
        .read(true)
        .write(true)
        .open(path)
}

#[cfg(test)]
mod tests;
