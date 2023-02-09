// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

#[allow(dead_code)]
fn main() {
    MainWindow::new().unwrap().run().unwrap();
}
slint::slint! {
// ANCHOR: tile
component MemoryTile inherits Rectangle {
    width: 64px;
    height: 64px;
    background: #3960D5;

    Image {
        source: @image-url("icons/bus.png");
        width: parent.width;
        height: parent.height;
    }
}

export component MainWindow inherits Window {
    MemoryTile {}
}
// ANCHOR_END: tile
}
