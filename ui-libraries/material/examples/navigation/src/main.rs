// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let app = App::new()?;

    let weak = app.as_weak();
    app.on_request_index(move |index| {
        let app = weak.unwrap();
        app.invoke_navigate_index(index);
        app.set_bar_index(app.get_current_route_index());
    });

    app.set_bar_index(app.get_current_route_index());

    app.run()
}
