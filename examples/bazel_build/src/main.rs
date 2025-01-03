// Copyright © 2024 OTIV B.V.
// SPDX-License-Identifier: MIT

include!(concat!(env!("OUT_DIR"), "/", "index.rs"));

use slint::ComponentHandle;

fn main() {
    let main_window = BazelBuildExampleWindow::new().expect("could not create main window");
    main_window.show().expect("could not show main window");
    main_window.run();
}
