// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
// cSpell: ignore slintsave

// This example shows how a Slint application can handle the files that the operating
// system asks it to open, for example because the user double-clicked a file of an
// associated type or chose "Open With" in Finder.
//
// The example models a "backup archive inspect" tool: opening a `.slintsave` file
// lists the entries it contains. On macOS the file-open request arrives through
// `slint::set_open_file_handler`; on Windows and Linux the same app can be handed the
// path on the command line. The example demonstrates both entry points.

slint::include_modules!();

use slint::{SharedString, VecModel};
use std::io::Read;
use std::path::Path;

/// Inspect a backup archive and return a summary of its contents. This example treats
/// the opened file as a small text archive that lists file names, one per line, and
/// shows them in the window. A real application would parse its own format here.
fn inspect_backup(path: &str) -> Vec<SharedString> {
    let mut contents = String::new();
    match std::fs::File::open(path).and_then(|mut f| f.read_to_string(&mut contents)) {
        Ok(_) => {
            let total = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
            let mut entries =
                Vec::from_iter(contents.lines().filter(|l| !l.is_empty()).map(SharedString::from));
            if entries.is_empty() {
                entries.push(SharedString::from(format!("(archive is empty, {total} bytes)")));
            }
            entries
        }
        Err(err) => {
            let name = Path::new(path).file_name().and_then(|n| n.to_str()).unwrap_or(path);
            vec![SharedString::from(format!("{name}: could not be read ({err})"))]
        }
    }
}

fn display_backup(window: &MainWindow, path: &str) {
    window.set_current_file_name(path.into());
    let entries = inspect_backup(path);
    window.set_archive_entries(slint::ModelRc::new(VecModel::from(entries)));
    window.set_opened_at(SharedString::from("Opened just now"));
}

/// Register the handler that receives file-open requests. On macOS this is how the
/// double-click / "Open With" path arrives; on other platforms it is never called.
fn install_open_file_handler(window: &MainWindow) -> Result<(), slint::PlatformError> {
    let weak = window.as_weak();
    slint::set_open_file_handler(move |paths| {
        if let Some(first) = paths.first()
            && let Some(window) = weak.upgrade()
        {
            display_backup(&window, first.as_str());
        }
    })?;
    Ok(())
}

fn main() -> Result<(), slint::PlatformError> {
    let window = MainWindow::new()?;
    install_open_file_handler(&window)?;

    // On Windows/Linux the application is started with the file as an argument; on
    // macOS the same file arrives via the handler above, so avoid double-handling.
    #[cfg(not(target_os = "macos"))]
    if let Some(path) = std::env::args().nth(1) {
        display_backup(&window, &path);
    }

    window.run().or_else(|_| Result::<(), slint::PlatformError>::Ok(()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inspect_backup_lists_entries() {
        let dir = std::env::temp_dir();
        let path = dir.join("slint-file-open-test.archive");
        std::fs::write(&path, "save-game-1\nsave-game-2\n\nsettings.json\n").unwrap();

        let entries = inspect_backup(path.to_str().unwrap());
        assert_eq!(
            entries,
            vec![
                SharedString::from("save-game-1"),
                SharedString::from("save-game-2"),
                SharedString::from("settings.json"),
            ]
        );

        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn inspect_backup_handles_missing_file() {
        let entries = inspect_backup("/nonexistent/slintsave.archive");
        assert_eq!(entries.len(), 1);
        assert!(entries[0].as_str().contains("could not be read"));
    }
}
