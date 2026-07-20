// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

slint::slint!(export { App } from "ui/app.slint";);

pub fn main() {
    App::new().unwrap().run().unwrap();
}
