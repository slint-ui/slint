<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0 -->

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

Use [`init_integration_test_with_system_time()`] for [integration tests](https://doc.rust-lang.org/rust-by-example/testing/integration_testing.html)
where your test code requires Slint to provide an event loop, for example when spawning threads
and calling `slint::invoke_from_event_loop()`. If you want to not only simulate the windowing system but
also the system time, use [`init_integration_test_with_mock_time()`] to initialize the backend and then
call [`mock_elapsed_time()`] to advance animations and move timers closer to their next timeout.

Use [`init_no_event_loop()`] for [unit tests](https://doc.rust-lang.org/rust-by-example/testing/unit_testing.html) when your test
code does not require an event loop. Note that system time is also mocked in this scenario, so use
[`mock_elapsed_time()`] to advance animations and timers.

## Preliminary User Interface Testing API

We're developing APIs to facilitate the creation of automated tests for Slint based UIs. A building block
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
The objective of the user interface testing API provided in this crate is to facilitate locating, mutation, and verifying state of
elements in your UI. [`ElementHandle`] provides a view for these elements.

The example below assumes that somewhere in the UI you have declared a `Button` with the text "Submit" and you may want to verify
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

## Simulating events / Asynchronous testing

When testing user interfaces it may be desirable to not only invoke accessible actions on elements, but it may also be
useful to simulate touch or mouse input. For example a mouse click on a button is a sequence:

1. An initial mouse move event to a location over the button
2. A mouse press event.
3. In real life, a certain amount of time would elapse now.
4. Finally, the user lifts the finger again from the mouse and a mouse release event is triggered.

To simulate this behaviour, [`ElementHandle`] provides functions such as [`ElementHandle::single_click()`] and [`ElementHandle::double_click()`].
Since these functions simulate a sequence of events with a period of idle time between the events, these functions are [async](https://doc.rust-lang.org/std/keyword.async.html)
and return a [`std::future::Future`], which resolves when the last event in the sequence was sent.

Calling these functions requires running the test function itself as a future and running an event loop in the background.
This can be accomplished using `slint::spawn_local()`, `slint::run_event_loop()`, and `slint::quit_event_loop()`. The following
example wraps the core functions for testing in an async closure:

```rust

use slint::platform::PointerEventButton;

#[test]
fn test_click() {
    i_slint_backend_testing::init_integration_test_with_system_time();

    slint::spawn_local(async move {
        slint::slint! {
            export component App inherits Window {
                out property <int> click-count: 0;
                ta := TouchArea {
                    clicked => { root.click-count += 1; }
                }
            }
        }

        let app = App::new().unwrap();

        let mut it = ElementHandle::find_by_element_id(&app, "App::ta");
        let elem = it.next().unwrap();
        assert!(it.next().is_none());

        assert_eq!(app.get_click_count(), 0);
        elem.single_click(PointerEventButton::Left).await;
        assert_eq!(app.get_click_count(), 1);

        slint::quit_event_loop().unwrap();
    })
    .unwrap();
    slint::run_event_loop().unwrap();
}
```

After initializing the testing backend with support for using the system time, an async
closure is spawned, which does the actual testing. In the subsequent `run_event_loop()` call,
the event loop is started, and that will start polling the async closure passed to `spawn_local()`.

In this closure we can now call `.await` on the future [`ElementHandle::single_click()`] returns, which
will keep running the event loop until the click is complete, and then continue with the test function.

