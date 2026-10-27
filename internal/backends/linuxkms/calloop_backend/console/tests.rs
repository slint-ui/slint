// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore getmode getstate KDGETMODE sigaction kdgetmode kdgkbmode kdsetmode kdskbmode nocapture setmode

use super::*;
use std::os::unix::process::ExitStatusExt;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

// Set SLINT_TEST_CONSOLE=/dev/tty1 only in a disposable VM with tty1 active.
static HANDLED: AtomicBool = AtomicBool::new(false);
static CONSOLE_IN_USE: std::sync::Mutex<()> = std::sync::Mutex::new(());

nix::ioctl_read_bad!(kdgetmode, 0x4B3B, i32);
nix::ioctl_read_bad!(vt_getmode, 0x5601, vt_mode);

extern "C" fn application_handler(_: i32) {
    HANDLED.store(true, Ordering::Relaxed);
}

fn action(signal: i32) -> libc::sigaction {
    let mut action = unsafe { std::mem::zeroed() };
    assert_eq!(unsafe { libc::sigaction(signal, std::ptr::null(), &mut action) }, 0);
    action
}

fn set_handler(signal: i32, handler: usize) {
    let mut action: libc::sigaction = unsafe { std::mem::zeroed() };
    action.sa_sigaction = handler;
    unsafe {
        libc::sigemptyset(&mut action.sa_mask);
        assert_eq!(libc::sigaction(signal, &action, std::ptr::null_mut()), 0);
    }
}

fn console() -> Console {
    if std::env::var_os("SLINT_TEST_CONSOLE").is_some() {
        request_graphics_mode();
        return Console::acquire(true).unwrap().expect("a virtual console is required");
    }

    // Host tests exercise process-wide signal ownership without touching a virtual console.
    let mut console =
        Console { tty: open_tty("/dev/null").unwrap(), handlers: [None; 3], owns_console: true };
    let handle_signals = std::env::var_os("SLINT_KMS_NO_SIGNAL_HANDLER").is_none();
    let _lock = ConsoleLock::new().unwrap();
    assert_eq!(TTY_FD.load(Ordering::Relaxed), -1);
    TTY_FD.store(console.tty.as_raw_fd(), Ordering::Relaxed);
    KEYBOARD_MODE.store(0, Ordering::Relaxed);
    if handle_signals {
        console.install_handlers().unwrap();
    }
    console
}

fn assert_prepared(console: &Console) {
    if std::env::var_os("SLINT_TEST_CONSOLE").is_some() {
        assert_eq!(console_mode(&console.tty), (K_OFF, KD_GRAPHICS, VT_PROCESS));
    }
}

/// Returns the keyboard, display, and switching modes.
fn console_mode(tty: &File) -> (i32, i32, i8) {
    let (mut keyboard, mut display, mut switching) = (0, 0, vt_mode::new(VT_AUTO));
    unsafe {
        kdgkbmode(tty.as_raw_fd(), &mut keyboard).unwrap();
        kdgetmode(tty.as_raw_fd(), &mut display).unwrap();
        vt_getmode(tty.as_raw_fd(), &mut switching).unwrap();
    }
    (keyboard, display, switching.mode)
}

fn set_console_mode(tty: &File, (keyboard, display, switching): (i32, i32, i8)) {
    unsafe {
        kdskbmode(tty.as_raw_fd(), keyboard).unwrap();
        kdsetmode(tty.as_raw_fd(), display).unwrap();
        vt_setmode(tty.as_raw_fd(), &vt_mode::new(switching)).unwrap();
    }
}

fn active_console(control: &File) -> u16 {
    let mut state: vt_stat = unsafe { std::mem::zeroed() };
    unsafe { vt_getstate(control.as_raw_fd(), &mut state).unwrap() };
    state.v_active
}

/// Returns whether the switch to `console` completed within three seconds.
fn activate(control: &File, console: u16) -> bool {
    nix::ioctl_write_int_bad!(vt_activate, 0x5606);
    unsafe { vt_activate(control.as_raw_fd(), console.into()).unwrap() };
    let deadline = Instant::now() + Duration::from_secs(3);
    while active_console(control) != console {
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    true
}

fn child(scenario: &str, signal: i32) -> Command {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", "calloop_backend::console::tests::signal_child", "--nocapture"])
        .env("SLINT_CONSOLE_TEST_CHILD", scenario)
        .env("SLINT_CONSOLE_TEST_SIGNAL", signal.to_string())
        .env_remove("SLINT_KMS_NO_SIGNAL_HANDLER")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
}

#[test]
fn signal_child() {
    let Ok(scenario) = std::env::var("SLINT_CONSOLE_TEST_CHILD") else { return };
    let signal: i32 = std::env::var("SLINT_CONSOLE_TEST_SIGNAL").unwrap().parse().unwrap();
    for signal in SIGNALS {
        set_handler(signal, libc::SIG_DFL);
    }
    if scenario == "ignored" {
        set_handler(signal, libc::SIG_IGN);
    } else if scenario == "handled" {
        set_handler(signal, application_handler as *const () as usize);
    }
    if scenario == "normal" {
        let mut previous = action(signal);
        previous.sa_flags = libc::SA_RESTART;
        unsafe {
            libc::sigaddset(&mut previous.sa_mask, libc::SIGUSR1);
            assert_eq!(libc::sigaction(signal, &previous, std::ptr::null_mut()), 0);
        }
    }
    if scenario == "mask" {
        let mut blocked = signal_set();
        unsafe {
            libc::sigaddset(&mut blocked, libc::SIGUSR1);
            assert_eq!(libc::pthread_sigmask(libc::SIG_BLOCK, &blocked, std::ptr::null_mut()), 0);
        }
    }
    let console = console();
    assert_prepared(&console);
    match scenario.as_str() {
        "signal" => {
            unsafe { libc::raise(signal) };
            panic!("signal didn't terminate");
        }
        "worker_signal" => {
            std::thread::spawn(move || unsafe { libc::raise(signal) }).join().unwrap();
            panic!("signal didn't terminate");
        }
        "blocked_signal" => {
            let lock = ConsoleLock::new().unwrap();
            unsafe { libc::raise(signal) };
            assert_prepared(&console);
            drop(lock);
            panic!("pending signal didn't terminate");
        }
        "contended_signal" => {
            let start = std::sync::Arc::new(AtomicBool::new(false));
            let started = start.clone();
            let worker = std::thread::spawn(move || {
                while !started.load(Ordering::Acquire) {
                    std::hint::spin_loop();
                }
                unsafe { libc::raise(signal) };
            });
            let lock = ConsoleLock::new().unwrap();
            start.store(true, Ordering::Release);
            // Give the worker a chance to enter the handler while the console is locked.
            for _ in 0..100_000 {
                std::hint::spin_loop();
            }
            drop(lock);
            worker.join().unwrap();
            panic!("signal didn't terminate");
        }
        "ignored" | "handled" => {
            let handler = action(signal).sa_sigaction;
            unsafe { libc::raise(signal) };
            assert_eq!(HANDLED.load(Ordering::Relaxed), scenario == "handled");
            assert_prepared(&console);
            drop(console);
            assert_eq!(action(signal).sa_sigaction, handler);
        }
        "opt_out" => {
            for signal in SIGNALS {
                assert_eq!(action(signal).sa_sigaction, libc::SIG_DFL);
            }
            drop(console);
        }
        "replacement" => {
            set_handler(signal, application_handler as *const () as usize);
            drop(console);
            unsafe { libc::raise(signal) };
            assert!(HANDLED.load(Ordering::Relaxed));
        }
        "unwind" => {
            assert!(
                std::panic::catch_unwind(move || {
                    let _console = console;
                    panic!("unwind out of event loop");
                })
                .is_err()
            );
        }
        "caught_panic" => {
            assert!(std::panic::catch_unwind(|| panic!("caught in callback")).is_err());
            assert_prepared(&console);
            drop(console);
        }
        "worker_panic" => {
            assert!(std::thread::spawn(|| panic!("worker panic")).join().is_err());
            assert_prepared(&console);
            drop(console);
        }
        "killed" => {
            unsafe { libc::raise(libc::SIGKILL) };
            panic!("SIGKILL didn't terminate");
        }
        "after_drop" => {
            drop(console);
            assert_eq!(action(signal).sa_sigaction, libc::SIG_DFL);
            unsafe { libc::raise(signal) };
            panic!("default disposition wasn't restored");
        }
        "normal" => {
            drop(console);
            for signal in SIGNALS {
                assert_eq!(action(signal).sa_sigaction, libc::SIG_DFL);
            }
            let restored = action(signal);
            assert_ne!(restored.sa_flags & libc::SA_RESTART, 0);
            assert_eq!(unsafe { libc::sigismember(&restored.sa_mask, libc::SIGUSR1) }, 1);
            drop(self::console());
        }
        "mask" => {
            drop(console);
            let mut mask = unsafe { std::mem::zeroed() };
            unsafe {
                assert_eq!(libc::pthread_sigmask(libc::SIG_BLOCK, std::ptr::null(), &mut mask), 0);
                for signal in SIGNALS.into_iter().chain([libc::SIGUSR1]) {
                    assert_eq!(libc::sigismember(&mask, signal), 1);
                }
            }
        }
        _ => panic!("unknown scenario"),
    }
    assert_eq!(TTY_FD.load(Ordering::Relaxed), -1);
}

#[test]
fn signal_lifecycle() {
    let _console = CONSOLE_IN_USE.lock().unwrap_or_else(|e| e.into_inner());
    let console = std::env::var_os("SLINT_TEST_CONSOLE")
        .map(|path| open_tty(path.to_str().unwrap()).unwrap());
    let original_mode = console.as_ref().map(console_mode);
    if let Some((keyboard, display, switching)) = original_mode {
        assert_ne!(keyboard, K_OFF);
        assert_ne!(display, KD_GRAPHICS);
        assert_eq!(switching, VT_AUTO);
    }
    for scenario in [
        "normal",
        "unwind",
        "caught_panic",
        "worker_panic",
        "opt_out",
        "signal",
        "worker_signal",
        "blocked_signal",
        "contended_signal",
        "ignored",
        "handled",
        "replacement",
        "after_drop",
        "mask",
    ] {
        for signal in SIGNALS {
            let mut command = child(scenario, signal);
            if scenario == "opt_out" {
                command.env("SLINT_KMS_NO_SIGNAL_HANDLER", "1");
            }
            let mut child = command.spawn().unwrap();
            let deadline = Instant::now() + Duration::from_secs(10);
            while child.try_wait().unwrap().is_none() {
                if Instant::now() >= deadline {
                    child.kill().unwrap();
                    let output = child.wait_with_output().unwrap();
                    if let Some(tty) = &console {
                        set_console_mode(tty, original_mode.unwrap());
                    }
                    panic!("{scenario}/{signal} timed out: {output:?}");
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            let output = child.wait_with_output().unwrap();
            if let Some(tty) = &console {
                let mode = console_mode(tty);
                // Restore even on failure, so the VM's console remains usable for diagnosis.
                set_console_mode(tty, original_mode.unwrap());
                assert_eq!(Some(mode), original_mode, "{scenario}/{signal}");
            }
            let expected_signal = matches!(
                scenario,
                "signal" | "worker_signal" | "blocked_signal" | "contended_signal" | "after_drop"
            );
            assert_eq!(
                output.status.signal(),
                expected_signal.then_some(signal),
                "{scenario}/{signal}: {output:?}"
            );
            if !expected_signal {
                assert!(output.status.success(), "{scenario}/{signal}: {output:?}");
            }
        }
    }
}

#[test]
fn switch_after_kill() {
    let Some(path) = std::env::var_os("SLINT_TEST_CONSOLE") else { return };
    let _console = CONSOLE_IN_USE.lock().unwrap_or_else(|e| e.into_inner());
    let tty = open_tty(path.to_str().unwrap()).unwrap();
    let control = open_tty("/dev/tty0").unwrap();
    let original_mode = console_mode(&tty);
    let console = active_console(&control);

    let (output, timed_out) = run_child("killed");
    let killed_mode = console_mode(&tty);
    let switched = activate(&control, console + 1) && activate(&control, console);
    let mode = console_mode(&tty);
    // Restore even on failure, so the VM's console remains usable for diagnosis.
    set_console_mode(&tty, original_mode);
    activate(&control, console);

    assert!(!timed_out, "{output:?}");
    assert_eq!(output.status.signal(), Some(libc::SIGKILL), "{output:?}");
    assert_eq!(killed_mode, (K_OFF, KD_GRAPHICS, VT_PROCESS));
    assert!(switched);
    // The kernel resets the keyboard to its default mode, which needn't be the original one.
    assert_ne!(mode.0, K_OFF);
    assert_eq!((mode.1, mode.2), (original_mode.1, original_mode.2));
}

#[test]
fn restart_after_kill() {
    let Some(path) = std::env::var_os("SLINT_TEST_CONSOLE") else { return };
    let _console = CONSOLE_IN_USE.lock().unwrap_or_else(|e| e.into_inner());
    let tty = open_tty(path.to_str().unwrap()).unwrap();
    let original_mode = console_mode(&tty);

    let (killed, killed_timed_out) = run_child("killed");
    let (restarted, restarted_timed_out) = run_child("caught_panic");
    let mode = console_mode(&tty);
    // Restore even on failure, so the VM's console remains usable for diagnosis.
    set_console_mode(&tty, original_mode);

    assert!(!killed_timed_out && !restarted_timed_out, "{killed:?} {restarted:?}");
    assert_eq!(killed.status.signal(), Some(libc::SIGKILL), "{killed:?}");
    assert!(restarted.status.success(), "{restarted:?}");
    assert_eq!((mode.1, mode.2), (KD_TEXT, VT_AUTO));
}

/// Returns the output, and whether the child had to be killed after ten seconds.
fn run_child(scenario: &str) -> (std::process::Output, bool) {
    let mut child = child(scenario, libc::SIGTERM).spawn().unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while child.try_wait().unwrap().is_none() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    let timed_out = child.try_wait().unwrap().is_none();
    let _ = child.kill();
    (child.wait_with_output().unwrap(), timed_out)
}
