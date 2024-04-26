<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.2 OR LicenseRef-Slint-commercial -->

**NOTE**: This library is an **internal** crate of the [Slint project](https://slint.dev).
This crate should **not be used directly** by applications using Slint.
You should use the `slint` crate instead.

**WARNING**: This crate does not follow the semver convention for versioning and can
only be used with `version = "=x.y.z"` in Cargo.toml.

# Preliminary Slint Testing API

This crate provides the preliminary API that we're developing to enable different
user interface (UI) testing scenarios for Slint applications.

To use this functionality, you need to be cautious when importing dependencies since
this crate does not adhere to semver and may introduce breaking changes in any patch release.
Additionally, the version of this crate must match the version of Slint.
To indicate that you specifically want this version, include the `=` symbol in the version string.

```toml
[dependencies]
slint = { version = "x.y.z", ... }
i-slint-backend-testing = "=x.y.z"
```

## Testing Backend

By default, Slint applications will select a backend and renderer suitable for application display
on the screen, by means of utilizing a windowing system - if present - or directly rendering to
the framebuffer.

For automated testing in CI environments without a windowing system / display, it might still be
desirable to run tests. The Slint Testing Backend simulates a windowing system without requiring one:
No pixels are rendered and text is measured by a fixed font size.

Use [`init_integration_test()`] for [integration tests](https://doc.rust-lang.org/rust-by-example/testing/integration_testing.html)
where your test code requires Slint to provide an event loop, for example when spawning threads
and calling `slint::invoke_from_event_loop()`.

Use [`init_no_event_loop()`] for [unit tests](https://doc.rust-lang.org/rust-by-example/testing/unit_testing.html) when your test
code does not require an event loop.

## Preliminary User Interface Testing API

We're developing APIs to faciliate the creation of automated tests for Slint based UIs. A building block
is the ability to introspect and modify the state from within what would be a regular application process.

A regular application might have a `main()` entry-point like so:

```rust,no_run
# slint::slint!{ export component App {} }
fn main() -> Result<(), slint::PlatformError>
{
    let app = App::new()?;
    // ... set up state, callbacks, models, ...

    app.run()
}
```

In addition, it may be desirable to create an integration test that verifies how the application behaves when simulating user input.
The objective of the user interface testing API provided in this crate is to faciliate locating, mutation, and verifying state of
elements in your UI. [`ElementHandle`] provides a view for these elements.

The example below assumes that somehwere in the UI you have declared a `Button` with the text "Submit" and you may want to verify
how the application behaves when simulation the activation. This is done by locating and triggering it via its accessibility interface,
that every `Button` implements.

```slint,no-preview
import { Button } from "std-widgets.slint";
component Form {
    callback submit();
    VerticalLayout {
        // ...
        Button {
            text: "Submit";
            clicked => { root.submit(); }
        }
    }
}

export component App {
    callback submit <=> form.submit;
    // ...
    form := Form {
        // ...
    }
}
```

```rust
# use i_slint_core_macros::identity as test;
# slint::slint!{
#     import { Button } from "std-widgets.slint";
#     component Form {
#     callback submit();
#     VerticalLayout {
#         // ...
#         Button {
#             text: "Submit";
#             clicked => { root.submit(); }
#         }
#     }
# }
#
# export component App {
#     callback submit <=> form.submit;
#     // ...
#     form := Form {
#         // ...
#     }
# }
# }
#[test]
fn test_basic_user_interface()
{
    i_slint_backend_testing::init_no_event_loop();
    let app = App::new().unwrap();
    // ... set up state, callbacks, models, ...
    let submitted = std::rc::Rc::new(std::cell::RefCell::new(false));

    app.on_submit({
        let submitted = submitted.clone();
        move || { *submitted.borrow_mut() = true; }
    });

    let buttons: Vec<_> = i_slint_backend_testing::ElementHandle::find_by_accessible_label(&app, "Submit").collect();
    assert_eq!(buttons.len(), 1);
    let button = &buttons[0];

    button.invoke_accessible_default_action();

    assert!(*submitted.borrow());
}
```
