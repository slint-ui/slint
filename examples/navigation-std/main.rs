// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

slint::slint!(export { App } from "navigation.slint";);

pub fn main() {
    let app = App::new().unwrap();

    let weak = app.as_weak();
    app.on_nav_select(move |index| {
        let app = weak.unwrap();
        app.invoke_navigate_index(index);
        app.set_nav_index(app.get_current_route_index());
    });

    app.set_nav_index(app.get_current_route_index());

    app.run().unwrap();
}
