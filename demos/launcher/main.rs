// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

slint::include_modules!();

mod input_device_monitor;

use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use std::path::PathBuf;

struct CatalogEntry {
    binary: &'static str,
    title: &'static str,
    description: &'static str,
    args: &'static [&'static str],
}

/// The demos and examples this launcher knows about, by installed binary name.
/// Only the ones found in PATH (or the extra search directories) are shown.
const CATALOG: &[CatalogEntry] = &[
    CatalogEntry {
        binary: "slint-viewer",
        title: "Remote Live-Preview",
        description: "Turn this device into a live-preview target that the Slint IDE integrations connect to over the network.",
        args: &["--remote"],
    },
    CatalogEntry {
        binary: "printerdemo",
        title: "Printer Demo",
        description: "A fictional user interface for the touch screen of a printer.",
        args: &[],
    },
    CatalogEntry {
        binary: "energy-monitor",
        title: "Energy Monitor",
        description: "A fictional user interface of a device that monitors energy consumption in a building.",
        args: &[],
    },
    CatalogEntry {
        binary: "home-automation",
        title: "Home Automation",
        description: "A fictional user interface of a device that automates the control of a home.",
        args: &[],
    },
    CatalogEntry {
        binary: "usecases",
        title: "Usecases",
        description: "Different example use cases in one app.",
        args: &[],
    },
    CatalogEntry {
        binary: "weather-demo",
        title: "Weather Demo",
        description: "A weather application using real weather data from the OpenWeather API.",
        args: &[],
    },
    CatalogEntry {
        binary: "gallery",
        title: "Widgets Gallery",
        description: "A gallery of the standard widgets that come with Slint.",
        args: &[],
    },
    CatalogEntry {
        binary: "todo",
        title: "Todo",
        description: "A simple todo list application.",
        args: &[],
    },
    CatalogEntry {
        binary: "memory",
        title: "Memory Game",
        description: "The memory game from the Slint tutorial.",
        args: &[],
    },
    CatalogEntry {
        binary: "slide_puzzle",
        title: "Slide Puzzle",
        description: "A sliding tile puzzle game.",
        args: &[],
    },
    CatalogEntry {
        binary: "carousel",
        title: "Carousel",
        description: "A custom carousel widget that can be controlled by touch, mouse, and keyboard.",
        args: &[],
    },
    CatalogEntry {
        binary: "speedometer",
        title: "Speedometer",
        description: "A dashboard with an animated speedometer.",
        args: &[],
    },
];

/// Search for `binary` in `search_dirs` and return the first match.
fn find_binary(binary: &str, search_dirs: &[PathBuf]) -> Option<PathBuf> {
    let file_name = format!("{binary}{}", std::env::consts::EXE_SUFFIX);
    search_dirs.iter().map(|dir| dir.join(&file_name)).find(|candidate| {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            candidate.metadata().is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        }
        #[cfg(not(unix))]
        {
            candidate.is_file()
        }
    })
}

/// On LinuxKMS the launcher owns the screen exclusively; there is no windowing
/// system to hand the display back and forth with a demo running side by side.
/// The launched demo replaces the launcher instead.
fn backend_is_linuxkms() -> bool {
    match std::env::var("SLINT_BACKEND") {
        Ok(backend) => backend.to_ascii_lowercase().starts_with("linuxkms"),
        Err(_) => {
            cfg!(any(feature = "backend-linuxkms", feature = "backend-linuxkms-noseat"))
                && !cfg!(feature = "default")
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut search_dirs = Vec::new();
    // In a development build, the demos and examples built from the workspace
    // land in the same cargo target directory as the launcher itself, so
    // `cargo run -p launcher` finds them without any configuration.
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        search_dirs.push(dir.to_path_buf());
    }
    if let Some(path) = std::env::var_os("PATH") {
        search_dirs.extend(std::env::split_paths(&path));
    }
    let installed: Vec<(&'static CatalogEntry, PathBuf)> = CATALOG
        .iter()
        .filter_map(|entry| find_binary(entry.binary, &search_dirs).map(|path| (entry, path)))
        .collect();

    // On LinuxKMS, only show the focus ring while a keyboard is attached, and
    // show a hint while no input device is attached at all. The monitor's
    // libinput hook must be installed before the window is created.
    let monitor = input_device_monitor::install()?;
    let ui = LauncherWindow::new()?;
    monitor.attach(&ui);

    if backend_is_linuxkms() {
        ui.invoke_default_to_dark_color_scheme();
    }

    let launcher = ui.global::<Launcher>();
    launcher.set_entries(ModelRc::new(
        installed
            .iter()
            .map(|(entry, _)| LauncherEntry {
                title: entry.title.into(),
                description: entry.description.into(),
            })
            .collect::<VecModel<_>>(),
    ));

    launcher.on_launch({
        let ui = ui.as_weak();
        move |index| {
            let Some((entry, path)) = installed.get(index as usize) else { return };
            let binary = entry.binary;

            let mut command = std::process::Command::new(path);
            command.args(entry.args);

            // On LinuxKMS, replace this process with the demo: exec() closes
            // the DRM device and hands the whole screen over. There is no way
            // back other than restarting the launcher.
            #[cfg(unix)]
            if backend_is_linuxkms() {
                use std::os::unix::process::CommandExt;
                let err = command.exec(); // only returns on failure
                ui.unwrap()
                    .global::<Launcher>()
                    .set_status(format!("Failed to start {binary}: {err}").into());
                return;
            }

            let ui_handle = ui.clone();
            let report = move |message: SharedString| {
                // Reporting fails when the launcher was closed while the demo
                // was still running; there is nobody left to tell then.
                let _ = ui_handle.upgrade_in_event_loop(move |ui| {
                    ui.global::<Launcher>().set_status(message);
                });
            };

            std::thread::spawn(move || match command.status() {
                Ok(status) if status.success() => report(SharedString::default()),
                Ok(status) => report(format!("{binary} exited with {status}").into()),
                Err(err) => report(format!("Failed to start {binary}: {err}").into()),
            });
        }
    });

    ui.run()?;
    Ok(())
}
