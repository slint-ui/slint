// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// PR A8b acceptance: ONE route model (the `navigator { Route... }` block in
// app.slint) drives Material's int-index chrome through the adapter. Material's
// `BaseNavigation` int API is untouched; the binding lives here, in the two
// lines that connect the bar to `navigate-index` / `current-route-index`.

slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let app = App::new()?;

    // Bind Material's chrome to the navigator adapter:
    //   index_changed(i) => navigate-index(i)   (the tap navigates)
    //   current_index    <- current-route-index (the bar reflects the route)
    // Both bar taps and in-screen buttons funnel through `request-index`, so
    // every navigation goes through the adapter and the bar stays in sync.
    let weak = app.as_weak();
    app.on_request_index(move |index| {
        let app = weak.unwrap();
        app.invoke_navigate_index(index);
        app.set_bar_index(app.get_current_route_index());
    });

    // Initial sync: the bar starts on whatever route the navigator starts on.
    app.set_bar_index(app.get_current_route_index());

    app.run()
}
