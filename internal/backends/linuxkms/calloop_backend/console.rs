// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore CLOEXEC errno evdev GETSTATE KDGKBMODE KDSKBMODE NOCTTY TCFLSH TCIFLUSH

//! Reading keyboards through evdev doesn't stop the kernel from also feeding the keys to the
//! active virtual console, where they end up in the shell once the application exits.
//! libseat switches the console keyboard off, so this is only needed without it.

use std::fs::{File, OpenOptions};
use std::os::fd::AsRawFd;
use std::os::unix::fs::OpenOptionsExt;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

use nix::{fcntl::OFlag, libc};

const VT_GETSTATE: u32 = 0x5603;
const KDGKBMODE: u32 = 0x4B44;
const KDSKBMODE: u32 = 0x4B45;
const K_OFF: i32 = 0x04;
const SIGNALS: [i32; 3] = [libc::SIGINT, libc::SIGTERM, libc::SIGHUP];

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
nix::ioctl_write_int_bad!(tcflsh, libc::TCFLSH);

static LOCKED: AtomicBool = AtomicBool::new(false);
static TTY_FD: AtomicI32 = AtomicI32::new(-1);
static ORIGINAL_MODE: AtomicI32 = AtomicI32::new(0);

pub struct ConsoleKeyboard {
    tty: File,
    original_mode: i32,
    handlers: [Option<libc::sigaction>; 3],
    owns_console: bool,
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
        Self::disable(tty).map(Some).map_err(|e| format!("Could not disable {tty_path}: {e}"))
    }

    fn disable(tty: File) -> Result<Self, nix::errno::Errno> {
        let handle_signals = std::env::var_os("SLINT_KMS_NO_SIGNAL_HANDLER").is_none();
        let mut keyboard = Self { tty, original_mode: 0, handlers: [None; 3], owns_console: false };
        let lock = ConsoleLock::new()?;
        if TTY_FD.load(Ordering::Relaxed) != -1 {
            return Err(nix::errno::Errno::EBUSY);
        }

        unsafe { kdgkbmode(keyboard.tty.as_raw_fd(), &mut keyboard.original_mode)? };
        TTY_FD.store(keyboard.tty.as_raw_fd(), Ordering::Relaxed);
        ORIGINAL_MODE.store(keyboard.original_mode, Ordering::Relaxed);
        keyboard.owns_console = true;
        if handle_signals {
            keyboard.install_handlers()?;
        }
        unsafe { kdskbmode(keyboard.tty.as_raw_fd(), K_OFF)? };
        drop(lock);
        Ok(keyboard)
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

impl Drop for ConsoleKeyboard {
    fn drop(&mut self) {
        if !self.owns_console {
            return;
        }
        // Only valid, fixed signal numbers are used, so blocking them can't fail on Linux.
        let Ok(lock) = ConsoleLock::new() else { return };
        let result = restore_keyboard(self.tty.as_raw_fd(), self.original_mode);
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
            eprintln!("Warning: Could not restore the console keyboard mode: {errno}");
        }
    }
}

fn restore_keyboard(fd: i32, mode: i32) -> Result<(), nix::errno::Errno> {
    unsafe {
        // Discard type-ahead so it doesn't run in the shell after restoration.
        let _ = tcflsh(fd, libc::TCIFLUSH);
        kdskbmode(fd, mode)?;
    }
    Ok(())
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
        let _ = restore_keyboard(fd, ORIGINAL_MODE.load(Ordering::Relaxed));
    }

    // Re-raise on this thread so waitpid reports termination by the original signal.
    // Keep the lock until termination so another thread can't disable the keyboard again.
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
