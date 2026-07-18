// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

slint::slint!(export { App } from "navigation.slint";);

pub fn main() {
    App::new().unwrap().run().unwrap();
}
