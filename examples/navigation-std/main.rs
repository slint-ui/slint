// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

slint::slint!(export { App } from "navigation.slint";);

pub fn main() {
    let app = App::new().unwrap();

    // Bridge the std-widgets tab bar to the navigator's int-index adapter. The
    // adapter lives on the root's public API (current-route-index /
    // navigate-index) but is synthesized too late to reference from .slint, so
    // the wiring happens here instead.
    let weak = app.as_weak();
    app.on_nav_select(move |index| {
        let app = weak.unwrap();
        // A tab click navigates by ordinal ...
        app.invoke_navigate_index(index);
        // ... and the highlight follows the resulting current route.
        app.set_nav_index(app.get_current_route_index());
    });

    // Seed the highlight from the initial route.
    app.set_nav_index(app.get_current_route_index());

    app.run().unwrap();
}
