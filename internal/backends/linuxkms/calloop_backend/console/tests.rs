// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::*;
use std::os::unix::process::ExitStatusExt;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

// Set SLINT_TEST_CONSOLE=/dev/tty1 only in a disposable VM with tty1 active.
static HANDLED: AtomicBool = AtomicBool::new(false);

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

fn keyboard() -> ConsoleKeyboard {
    if std::env::var_os("SLINT_TEST_CONSOLE").is_some() {
        return ConsoleKeyboard::acquire().unwrap().expect("a virtual console is required");
    }

    // Host tests exercise process-wide signal ownership without touching a virtual console.
    let mut keyboard = ConsoleKeyboard {
        tty: open_tty("/dev/null").unwrap(),
        original_mode: 0,
        handlers: [None; 3],
        owns_console: true,
    };
    let handle_signals = std::env::var_os("SLINT_KMS_NO_SIGNAL_HANDLER").is_none();
    let _lock = ConsoleLock::new().unwrap();
    assert_eq!(TTY_FD.load(Ordering::Relaxed), -1);
    TTY_FD.store(keyboard.tty.as_raw_fd(), Ordering::Relaxed);
    ORIGINAL_MODE.store(0, Ordering::Relaxed);
    if handle_signals {
        keyboard.install_handlers().unwrap();
    }
    keyboard
}

fn assert_disabled(keyboard: &ConsoleKeyboard) {
    if std::env::var_os("SLINT_TEST_CONSOLE").is_some() {
        let mut mode = 0;
        unsafe { kdgkbmode(keyboard.tty.as_raw_fd(), &mut mode).unwrap() };
        assert_eq!(mode, K_OFF);
    }
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
    let keyboard = keyboard();
    assert_disabled(&keyboard);
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
            assert_disabled(&keyboard);
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
            assert_disabled(&keyboard);
            drop(keyboard);
            assert_eq!(action(signal).sa_sigaction, handler);
        }
        "opt_out" => {
            for signal in SIGNALS {
                assert_eq!(action(signal).sa_sigaction, libc::SIG_DFL);
            }
            drop(keyboard);
        }
        "replacement" => {
            set_handler(signal, application_handler as *const () as usize);
            drop(keyboard);
            unsafe { libc::raise(signal) };
            assert!(HANDLED.load(Ordering::Relaxed));
        }
        "unwind" => {
            assert!(
                std::panic::catch_unwind(move || {
                    let _keyboard = keyboard;
                    panic!("unwind out of event loop");
                })
                .is_err()
            );
        }
        "caught_panic" => {
            assert!(std::panic::catch_unwind(|| panic!("caught in callback")).is_err());
            assert_disabled(&keyboard);
            drop(keyboard);
        }
        "worker_panic" => {
            assert!(std::thread::spawn(|| panic!("worker panic")).join().is_err());
            assert_disabled(&keyboard);
            drop(keyboard);
        }
        "after_drop" => {
            drop(keyboard);
            assert_eq!(action(signal).sa_sigaction, libc::SIG_DFL);
            unsafe { libc::raise(signal) };
            panic!("default disposition wasn't restored");
        }
        "normal" => {
            drop(keyboard);
            for signal in SIGNALS {
                assert_eq!(action(signal).sa_sigaction, libc::SIG_DFL);
            }
            let restored = action(signal);
            assert_ne!(restored.sa_flags & libc::SA_RESTART, 0);
            assert_eq!(unsafe { libc::sigismember(&restored.sa_mask, libc::SIGUSR1) }, 1);
            drop(self::keyboard());
        }
        "mask" => {
            drop(keyboard);
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
    let console = std::env::var_os("SLINT_TEST_CONSOLE")
        .map(|path| open_tty(path.to_str().unwrap()).unwrap());
    let original_mode = console.as_ref().map(|tty| {
        let mut mode = 0;
        unsafe { kdgkbmode(tty.as_raw_fd(), &mut mode).unwrap() };
        assert_ne!(mode, K_OFF);
        mode
    });
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
            let mut command = Command::new(std::env::current_exe().unwrap());
            command
                .args(["--exact", "calloop_backend::console::tests::signal_child", "--nocapture"])
                .env("SLINT_CONSOLE_TEST_CHILD", scenario)
                .env("SLINT_CONSOLE_TEST_SIGNAL", signal.to_string())
                .env_remove("SLINT_KMS_NO_SIGNAL_HANDLER");
            if scenario == "opt_out" {
                command.env("SLINT_KMS_NO_SIGNAL_HANDLER", "1");
            }
            let mut child = command.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
            let deadline = Instant::now() + Duration::from_secs(10);
            while child.try_wait().unwrap().is_none() {
                if Instant::now() >= deadline {
                    child.kill().unwrap();
                    let output = child.wait_with_output().unwrap();
                    if let Some(tty) = &console {
                        unsafe { kdskbmode(tty.as_raw_fd(), original_mode.unwrap()).unwrap() };
                    }
                    panic!("{scenario}/{signal} timed out: {output:?}");
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            let output = child.wait_with_output().unwrap();
            if let Some(tty) = &console {
                let mut mode = 0;
                unsafe { kdgkbmode(tty.as_raw_fd(), &mut mode).unwrap() };
                // Restore even on failure, so the VM's console remains usable for diagnosis.
                unsafe { kdskbmode(tty.as_raw_fd(), original_mode.unwrap()).unwrap() };
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
