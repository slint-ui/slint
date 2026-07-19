// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_core::api::ComponentHandle;

fn set_global_log_message_handler(
    handler: Option<i_slint_core::debug_log::LogMessageHandler>,
) -> Option<i_slint_core::debug_log::LogMessageHandler> {
    i_slint_backend_selector::with_global_context(|ctx| ctx.set_log_message_handler(handler))
        .unwrap()
}

#[test]
fn reuse_window() {
    i_slint_backend_testing::init_no_event_loop();
    use crate::{Compiler, SharedString, Value};
    let code = r#"
        export component MainWindow inherits Window {
            in-out property<string> text_text: "foo";
            in-out property<string> text_alias: input.text;
            input := TextInput {
                text:  self.enabled ? text_text : text_text;
            }
        }
    "#;
    let handle = {
        let mut compiler = Compiler::default();
        compiler.set_style("fluent".into());
        let result = spin_on::spin_on(compiler.build_from_source(code.into(), Default::default()));
        assert!(!result.has_errors(), "{:?}", result.diagnostics().collect::<Vec<_>>());
        let definition = result.component("MainWindow").unwrap();
        let instance = definition.create().unwrap();
        assert_eq!(
            instance.get_property("text_alias").unwrap(),
            Value::from(SharedString::from("foo"))
        );
        instance
    };

    let _handle2 = {
        let mut compiler = Compiler::default();
        compiler.set_style("fluent".into());
        let result = spin_on::spin_on(compiler.build_from_source(code.into(), Default::default()));
        assert!(!result.has_errors(), "{:?}", result.diagnostics().collect::<Vec<_>>());
        let definition = result.component("MainWindow").unwrap();
        let instance = definition.create_with_existing_window(handle.window()).unwrap();
        drop(handle);
        assert_eq!(
            instance.get_property("text_alias").unwrap(),
            Value::from(SharedString::from("foo"))
        );
        instance
    };
}

#[test]
fn context_debug_handler_overrides_platform() {
    i_slint_backend_testing::init_no_event_loop();
    use crate::Compiler;
    use std::cell::RefCell;
    use std::rc::Rc;

    let captured = Rc::new(RefCell::new(Vec::new()));
    let previous = set_global_log_message_handler(Some(Box::new({
        let captured = captured.clone();
        move |message: i_slint_core::debug_log::LogMessage<'_>| {
            captured.borrow_mut().push(message.message_arguments().to_string());
        }
    })));

    let code = r#"
        export component MainWindow inherits Window {
            init => { debug("from component"); }
        }
    "#;
    let mut compiler = Compiler::default();
    compiler.set_style("fluent".into());
    let result = spin_on::spin_on(compiler.build_from_source(code.into(), Default::default()));
    assert!(!result.has_errors(), "{:?}", result.diagnostics().collect::<Vec<_>>());
    let definition = result.component("MainWindow").unwrap();
    let instance = definition.create().unwrap();

    assert_eq!(captured.borrow().as_slice(), ["from component"]);
    assert!(
        i_slint_backend_testing::access_testing_window(
            instance.window(),
            |w: &i_slint_backend_testing::TestingWindow| w.take_debug_log(),
        )
        .is_empty()
    );

    set_global_log_message_handler(previous);
}

#[test]
fn platform_debug_handler_is_fallback() {
    i_slint_backend_testing::init_no_event_loop();
    use crate::Compiler;

    let code = r#"
        export component MainWindow inherits Window {
            init => { debug("from component"); }
        }
    "#;
    let mut compiler = Compiler::default();
    compiler.set_style("fluent".into());
    let result = spin_on::spin_on(compiler.build_from_source(code.into(), Default::default()));
    assert!(!result.has_errors(), "{:?}", result.diagnostics().collect::<Vec<_>>());
    let definition = result.component("MainWindow").unwrap();
    let instance = definition.create().unwrap();

    assert_eq!(
        i_slint_backend_testing::access_testing_window(
            instance.window(),
            |w: &i_slint_backend_testing::TestingWindow| w.take_debug_log(),
        ),
        ["from component"]
    );
}

#[test]
fn global_debug_messages_use_context_handler() {
    i_slint_backend_testing::init_no_event_loop();
    use crate::Compiler;
    use std::cell::RefCell;
    use std::rc::Rc;

    let captured = Rc::new(RefCell::new(Vec::new()));
    let previous = set_global_log_message_handler(Some(Box::new({
        let captured = captured.clone();
        move |message: i_slint_core::debug_log::LogMessage<'_>| {
            captured.borrow_mut().push(message.message_arguments().to_string());
        }
    })));

    let code = r#"
        export global Logic {
            in-out property<string> greeting: {
                debug("from global");
                "hello"
            };
        }

        export component MainWindow inherits Window { }
    "#;
    let mut compiler = Compiler::default();
    compiler.set_style("fluent".into());
    let result = spin_on::spin_on(compiler.build_from_source(code.into(), Default::default()));
    assert!(!result.has_errors(), "{:?}", result.diagnostics().collect::<Vec<_>>());
    let definition = result.component("MainWindow").unwrap();
    let instance = definition.create().unwrap();

    assert_eq!(
        instance.get_global_property("Logic", "greeting").unwrap(),
        crate::Value::from(crate::SharedString::from("hello"))
    );
    assert_eq!(captured.borrow().as_slice(), ["from global"]);
    assert!(
        i_slint_backend_testing::access_testing_window(
            instance.window(),
            |w: &i_slint_backend_testing::TestingWindow| w.take_debug_log(),
        )
        .is_empty()
    );

    set_global_log_message_handler(previous);
}

#[test]
fn set_wrong_struct() {
    i_slint_backend_testing::init_no_event_loop();
    use crate::{Compiler, Struct, Value};
    let code = r#"
struct TimeStruct {
    Clock:              string,
    Enabled:            bool,
}

export global Device {
    in-out property <TimeStruct> Time: { Clock: "11:37", Enabled: true };
}

export component Clock {
    ta := TouchArea {
        enabled: Device.Time.Enabled;
    }
    out property <bool> ta_enabled: ta.enabled;
    out property <string> time: Device.Time.Clock;
}
    "#;
    let compiler = Compiler::default();
    let result = spin_on::spin_on(compiler.build_from_source(code.into(), Default::default()));
    assert!(!result.has_errors(), "{:?}", result.diagnostics().collect::<Vec<_>>());
    let definition = result.component("Clock").unwrap();
    let instance = definition.create().unwrap();
    assert_eq!(instance.get_property("ta_enabled").unwrap(), Value::from(true));
    assert_eq!(instance.get_property("time").unwrap(), Value::String("11:37".into()));
    // This only has "Clock" but no "Enabled"
    instance
        .set_global_property(
            "Device",
            "Time",
            Struct::from_iter([("Clock".into(), Value::String("10:37".into()))]).into(),
        )
        .unwrap();
    assert_eq!(instance.get_property("ta_enabled").unwrap(), Value::from(false));
    assert_eq!(instance.get_property("time").unwrap(), Value::String("10:37".into()));

    // Setting a struct with wrong fields leads to an error
    assert_eq!(
        instance.set_global_property(
            "Device",
            "Time",
            Struct::from_iter([("Broken".into(), Value::Number(12.1))]).into(),
        ),
        Err(crate::SetPropertyError::WrongType)
    );
    assert_eq!(instance.get_property("ta_enabled").unwrap(), Value::from(false));
    assert_eq!(instance.get_property("time").unwrap(), Value::String("10:37".into()));
}

#[test]
fn popup_is_open() {
    i_slint_backend_testing::init_no_event_loop();
    use crate::{Compiler, ComponentHandle, Value};
    // `Dropdown` is used twice, so it stays a real sub-component and gets fully inlined, which moves
    // the synthesized `is-open` property to the parent's root with a mangled name -- exercising the
    // `move_declarations` fixup path. `popup.is-open` is read from `open` inside the sub-component.
    let code = r#"
component Dropdown inherits Rectangle {
    width: 40px;
    height: 20px;
    out property <bool> open: popup.is-open;
    callback do-show;
    callback do-close;
    do-show => { popup.show(); }
    do-close => { popup.close(); }
    popup := PopupWindow {
        close-policy: no-auto-close;
        x: 0; y: 20px; width: 40px; height: 40px;
        Rectangle { background: blue; }
    }
}
export component TestCase {
    width: 300px;
    height: 300px;
    out property <bool> a-open: a.open;
    out property <bool> a2-open: a2.open;
    callback show-a;
    callback close-a;
    callback show-a2;
    show-a => { a.do-show(); }
    close-a => { a.do-close(); }
    show-a2 => { a2.do-show(); }
    a := Dropdown { x: 0; y: 0; }
    a2 := Dropdown { x: 50px; y: 0; }
}
    "#;
    let compiler = Compiler::default();
    let result = spin_on::spin_on(compiler.build_from_source(code.into(), Default::default()));
    assert!(!result.has_errors(), "{:?}", result.diagnostics().collect::<Vec<_>>());
    let definition = result.component("TestCase").unwrap();
    let instance = definition.create().unwrap();
    let _ = instance.window(); // ensure window

    assert_eq!(instance.get_property("a-open").unwrap(), Value::from(false), "a before show");
    assert_eq!(instance.get_property("a2-open").unwrap(), Value::from(false), "a2 before show");

    // Showing `a` flips only `a`'s is-open to true.
    instance.invoke("show-a", &[]).unwrap();
    assert_eq!(instance.get_property("a-open").unwrap(), Value::from(true), "a after show");
    assert_eq!(instance.get_property("a2-open").unwrap(), Value::from(false), "a2 unaffected");

    // A second independent instance opens on its own.
    instance.invoke("show-a2", &[]).unwrap();
    assert_eq!(instance.get_property("a2-open").unwrap(), Value::from(true), "a2 after show");

    // Programmatic close() drops the `PopupWindow`, whose `Drop` impl resets is-open to false.
    instance.invoke("close-a", &[]).unwrap();
    assert_eq!(instance.get_property("a-open").unwrap(), Value::from(false), "a after close");
}

// The milestone test: a `navigator` renders the destination component for the
// current route, and re-renders when the route changes. Each destination
// records its name in a global on `init`, so the observed value tells us which
// route's screen is currently instantiated. Requires `enable_experimental`,
// which is only reachable through the `internal` compiler configuration API.
#[cfg(feature = "internal")]
#[test]
fn navigator_shows_current_route() {
    i_slint_backend_testing::init_no_event_loop();
    use crate::{Compiler, SharedString, Value};
    let code = r#"
enum Route { Home, Settings }
global NavProbe { in-out property <string> active; }
component HomeScreen inherits Rectangle { init => { NavProbe.active = "home"; } }
component SettingsScreen inherits Rectangle { init => { NavProbe.active = "settings"; } }
export component TestCase inherits Window {
    width: 100px;
    height: 100px;
    in-out property <Route> current-route: Route.Home;
    out property <string> active: NavProbe.active;
    navigator (current-route) {
        Route.Home: HomeScreen { }
        Route.Settings: SettingsScreen { }
    }
}
"#;
    let mut compiler = Compiler::default();
    compiler.set_style("fluent".into());
    compiler.compiler_configuration(i_slint_core::InternalToken).enable_experimental = true;
    let result = spin_on::spin_on(compiler.build_from_source(code.into(), Default::default()));
    assert!(!result.has_errors(), "{:?}", result.diagnostics().collect::<Vec<_>>());
    let definition = result.component("TestCase").unwrap();
    let instance = definition.create().unwrap();
    let _ = instance.window();

    let route = |v: &str| Value::EnumerationValue("Route".into(), v.into());

    // The initial route renders its screen.
    i_slint_backend_testing::mock_elapsed_time(100);
    assert_eq!(
        instance.get_property("active").unwrap(),
        Value::from(SharedString::from("home")),
        "Route.Home renders HomeScreen"
    );

    // Switching the current route renders the other screen.
    instance.set_property("current-route", route("Settings")).unwrap();
    i_slint_backend_testing::mock_elapsed_time(100);
    assert_eq!(
        instance.get_property("active").unwrap(),
        Value::from(SharedString::from("settings")),
        "Route.Settings renders SettingsScreen"
    );

    // And back to the first route.
    instance.set_property("current-route", route("Home")).unwrap();
    i_slint_backend_testing::mock_elapsed_time(100);
    assert_eq!(
        instance.get_property("active").unwrap(),
        Value::from(SharedString::from("home")),
        "Route.Home renders HomeScreen again"
    );
}

// A federated `mount Impl via Contract` route destination renders the mounted
// module directly (no ComponentContainer indirection): switching the shell's
// route to the mounted case instantiates `ModuleA`, whose screen records itself
// in a global. Same observation approach as `navigator_shows_current_route`.
#[cfg(feature = "internal")]
#[test]
fn navigator_mount_renders_module() {
    i_slint_backend_testing::init_no_event_loop();
    use crate::{Compiler, SharedString, Value};
    let code = r#"
interface AppNavV1 {
    route Home;
}
global NavProbe { in-out property <string> active; }
enum ModuleRoute { Home }
component ModHome inherits Rectangle { init => { NavProbe.active = "module-home"; } }
component ModuleA implements AppNavV1 inherits Rectangle {
    in-out property <ModuleRoute> current-route: ModuleRoute.Home;
    navigator (current-route) {
        ModuleRoute.Home: ModHome { }
    }
}
component HomeScreen inherits Rectangle { init => { NavProbe.active = "shell-home"; } }
enum ShellRoute { Home, ModuleA }
export component TestCase inherits Window {
    width: 100px;
    height: 100px;
    in-out property <ShellRoute> current: ShellRoute.Home;
    out property <string> active: NavProbe.active;
    navigator (current) {
        ShellRoute.Home: HomeScreen { }
        ShellRoute.ModuleA: mount ModuleA via AppNavV1 { }
    }
}
"#;
    let mut compiler = Compiler::default();
    compiler.set_style("fluent".into());
    compiler.compiler_configuration(i_slint_core::InternalToken).enable_experimental = true;
    let result = spin_on::spin_on(compiler.build_from_source(code.into(), Default::default()));
    assert!(!result.has_errors(), "{:?}", result.diagnostics().collect::<Vec<_>>());
    let definition = result.component("TestCase").unwrap();
    let instance = definition.create().unwrap();
    let _ = instance.window();

    let route = |v: &str| Value::EnumerationValue("ShellRoute".into(), v.into());

    // The initial route renders the shell's own screen.
    i_slint_backend_testing::mock_elapsed_time(100);
    assert_eq!(
        instance.get_property("active").unwrap(),
        Value::from(SharedString::from("shell-home")),
        "ShellRoute.Home renders HomeScreen"
    );

    // Switching to the mounted route renders the module's own screen, proving the
    // mount instantiated ModuleA directly as the route destination.
    instance.set_property("current", route("ModuleA")).unwrap();
    i_slint_backend_testing::mock_elapsed_time(100);
    assert_eq!(
        instance.get_property("active").unwrap(),
        Value::from(SharedString::from("module-home")),
        "ShellRoute.ModuleA mounts and renders ModuleA"
    );
}

// Without `enable_experimental`, `navigator` is rejected at object-tree
// lowering with the experimental-feature diagnostic. `Compiler::default()`
// does not enable experimental features.
#[test]
fn navigator_requires_experimental() {
    i_slint_backend_testing::init_no_event_loop();
    use crate::Compiler;
    let code = r#"
enum Route { Home }
component HomeScreen inherits Rectangle { }
export component TestCase inherits Window {
    in-out property <Route> current-route: Route.Home;
    navigator (current-route) {
        Route.Home: HomeScreen { }
    }
}
"#;
    let compiler = Compiler::default();
    let result = spin_on::spin_on(compiler.build_from_source(code.into(), Default::default()));
    assert!(result.has_errors(), "navigator must be rejected without experimental");
    assert!(
        result.diagnostics().any(|d| d.message().contains("navigator is an experimental feature")),
        "expected the experimental-feature diagnostic, got: {:?}",
        result.diagnostics().map(|d| d.message().to_owned()).collect::<Vec<_>>()
    );
}

// The back-stack: `navigate(route)` pushes the current route before switching,
// `back()` restores the previous route (no-op at the root), and `can-go-back`
// reflects whether the stack is non-empty. The `active` global still tells us
// which screen is instantiated, so we assert on the rendered route.
#[cfg(feature = "internal")]
#[test]
fn navigator_back_stack() {
    i_slint_backend_testing::init_no_event_loop();
    use crate::{Compiler, SharedString, Value};
    let code = r#"
enum Route { Home, Details, Settings }
global NavProbe { in-out property <string> active; }
component HomeScreen inherits Rectangle { init => { NavProbe.active = "home"; } }
component DetailsScreen inherits Rectangle { init => { NavProbe.active = "details"; } }
component SettingsScreen inherits Rectangle { init => { NavProbe.active = "settings"; } }
export component TestCase inherits Window {
    width: 100px;
    height: 100px;
    in-out property <Route> current-route: Route.Home;
    out property <string> active: NavProbe.active;
    navigator (current-route) {
        Route.Home: HomeScreen { }
        Route.Details: DetailsScreen { }
        Route.Settings: SettingsScreen { }
    }
}
"#;
    let mut compiler = Compiler::default();
    compiler.set_style("fluent".into());
    compiler.compiler_configuration(i_slint_core::InternalToken).enable_experimental = true;
    let result = spin_on::spin_on(compiler.build_from_source(code.into(), Default::default()));
    assert!(!result.has_errors(), "{:?}", result.diagnostics().collect::<Vec<_>>());
    let definition = result.component("TestCase").unwrap();
    let instance = definition.create().unwrap();
    let _ = instance.window();

    let route = |v: &str| Value::EnumerationValue("Route".into(), v.into());
    let rendered = |instance: &crate::ComponentInstance| {
        i_slint_backend_testing::mock_elapsed_time(100);
        instance.get_property("active").unwrap()
    };

    // Root: nothing to go back to.
    assert_eq!(rendered(&instance), Value::from(SharedString::from("home")));
    assert_eq!(instance.get_property("can-go-back").unwrap(), Value::from(false), "root");

    // navigate(Home -> Details -> Settings): the stack fills up.
    instance.invoke("navigate", &[route("Details")]).unwrap();
    assert_eq!(rendered(&instance), Value::from(SharedString::from("details")));
    assert_eq!(instance.get_property("can-go-back").unwrap(), Value::from(true), "after Details");

    instance.invoke("navigate", &[route("Settings")]).unwrap();
    assert_eq!(rendered(&instance), Value::from(SharedString::from("settings")));
    assert_eq!(instance.get_property("can-go-back").unwrap(), Value::from(true), "after Settings");

    // back() twice unwinds Settings -> Details -> Home.
    instance.invoke("back", &[]).unwrap();
    assert_eq!(rendered(&instance), Value::from(SharedString::from("details")), "back to Details");
    assert_eq!(instance.get_property("can-go-back").unwrap(), Value::from(true), "one left");

    instance.invoke("back", &[]).unwrap();
    assert_eq!(rendered(&instance), Value::from(SharedString::from("home")), "back to Home");
    assert_eq!(
        instance.get_property("can-go-back").unwrap(),
        Value::from(false),
        "back at the root"
    );

    // back() at the root is a no-op.
    instance.invoke("back", &[]).unwrap();
    assert_eq!(rendered(&instance), Value::from(SharedString::from("home")), "no-op at root");
    assert_eq!(instance.get_property("can-go-back").unwrap(), Value::from(false), "still root");
}

// The int-index adapter: `current-route-index` reports the ordinal of the
// current route in declaration order, and `navigate-index(i)` navigates to the
// route at that ordinal. This is what int-index chrome (current_index /
// index_changed) binds to, so it must agree with the route enum both ways.
#[cfg(feature = "internal")]
#[test]
fn navigator_index_adapter() {
    i_slint_backend_testing::init_no_event_loop();
    use crate::{Compiler, Value};
    let code = r#"
enum Route { Home, Details, Settings }
global NavProbe { in-out property <string> active; }
component HomeScreen inherits Rectangle { init => { NavProbe.active = "home"; } }
component DetailsScreen inherits Rectangle { init => { NavProbe.active = "details"; } }
component SettingsScreen inherits Rectangle { init => { NavProbe.active = "settings"; } }
export component TestCase inherits Window {
    width: 100px;
    height: 100px;
    in-out property <Route> current-route: Route.Home;
    out property <string> active: NavProbe.active;
    navigator (current-route) {
        Route.Home: HomeScreen { }
        Route.Details: DetailsScreen { }
        Route.Settings: SettingsScreen { }
    }
}
"#;
    let mut compiler = Compiler::default();
    compiler.set_style("fluent".into());
    compiler.compiler_configuration(i_slint_core::InternalToken).enable_experimental = true;
    let result = spin_on::spin_on(compiler.build_from_source(code.into(), Default::default()));
    assert!(!result.has_errors(), "{:?}", result.diagnostics().collect::<Vec<_>>());
    let definition = result.component("TestCase").unwrap();
    let instance = definition.create().unwrap();
    let _ = instance.window();

    let route = |v: &str| Value::EnumerationValue("Route".into(), v.into());
    let index = |instance: &crate::ComponentInstance| {
        i_slint_backend_testing::mock_elapsed_time(100);
        instance.get_property("current-route-index").unwrap()
    };

    // Setting the route enum drives current-route-index to the declared ordinal.
    assert_eq!(index(&instance), Value::from(0.), "Home is ordinal 0");
    instance.set_property("current-route", route("Details")).unwrap();
    assert_eq!(index(&instance), Value::from(1.), "Details is ordinal 1");
    instance.set_property("current-route", route("Settings")).unwrap();
    assert_eq!(index(&instance), Value::from(2.), "Settings is ordinal 2");

    // navigate-index(i) drives the route enum the other way (and the index with it).
    instance.invoke("navigate-index", &[Value::from(2.)]).unwrap();
    assert_eq!(
        instance.get_property("current-route").unwrap(),
        route("Settings"),
        "navigate-index(2) selects Settings"
    );
    assert_eq!(index(&instance), Value::from(2.));

    instance.invoke("navigate-index", &[Value::from(0.)]).unwrap();
    assert_eq!(
        instance.get_property("current-route").unwrap(),
        route("Home"),
        "navigate-index(0) selects Home"
    );
    assert_eq!(index(&instance), Value::from(0.));

    // Out-of-range is a no-op: the route (and index) stay put.
    instance.invoke("navigate-index", &[Value::from(9.)]).unwrap();
    assert_eq!(
        instance.get_property("current-route").unwrap(),
        route("Home"),
        "out-of-range navigate-index is a no-op"
    );
    assert_eq!(index(&instance), Value::from(0.));
}

// PR A8: a std-widgets navigation presentation (a Button-row tab bar) driving
// the navigator through its int-index adapter. The tab bar is a plain std
// `Button` row that exposes `current-index` / `selected(int)`; the host bridges
// those to the navigator's `current-route-index` / `navigate-index` (the adapter
// is synthesized after resolve, so it is reachable from the host rather than
// from .slint). This guards that the std presentation compiles with the
// navigator and that both directions of the binding hold: the highlight source
// follows the current route, and activating a tab moves the current route.
#[cfg(feature = "internal")]
#[test]
fn navigator_std_chrome_index_binding() {
    i_slint_backend_testing::init_no_event_loop();
    use crate::{Compiler, SharedString, Value};
    let code = r#"
import { Button, HorizontalBox, VerticalBox } from "std-widgets.slint";

enum Route { Home, Details, Settings }
global NavProbe { in-out property <string> active; }
component HomeScreen inherits Rectangle { init => { NavProbe.active = "home"; } }
component DetailsScreen inherits Rectangle { init => { NavProbe.active = "details"; } }
component SettingsScreen inherits Rectangle { init => { NavProbe.active = "settings"; } }

// The std presentation: a Button row whose active segment follows current-index.
component NavBar inherits HorizontalBox {
    in property <[string]> titles;
    in property <int> current-index;
    callback selected(index: int);
    for title[index] in titles: Button {
        text: title;
        primary: index == root.current-index;
        clicked => { root.selected(index); }
    }
}

export component TestCase inherits Window {
    width: 300px;
    height: 200px;
    in-out property <Route> current-route: Route.Home;
    out property <string> active: NavProbe.active;
    // The chrome inputs the host bridges to the navigator adapter.
    in property <int> nav-index;
    callback nav-select(index: int);

    NavBar {
        titles: ["Home", "Details", "Settings"];
        current-index: root.nav-index;
        selected(index) => { root.nav-select(index); }
    }
    navigator (current-route) {
        Route.Home: HomeScreen { }
        Route.Details: DetailsScreen { }
        Route.Settings: SettingsScreen { }
    }
}
"#;
    let mut compiler = Compiler::default();
    compiler.set_style("fluent".into());
    compiler.compiler_configuration(i_slint_core::InternalToken).enable_experimental = true;
    let result = spin_on::spin_on(compiler.build_from_source(code.into(), Default::default()));
    assert!(!result.has_errors(), "{:?}", result.diagnostics().collect::<Vec<_>>());
    let definition = result.component("TestCase").unwrap();
    let instance = definition.create().unwrap();
    let _ = instance.window();

    let route = |v: &str| Value::EnumerationValue("Route".into(), v.into());
    let settle = || i_slint_backend_testing::mock_elapsed_time(100);

    // The host feeds the tab bar's highlight from the adapter. Setting the route
    // (as an in-screen action would) moves current-route-index, which is what
    // the chrome's current-index binds to.
    settle();
    instance.set_property("current-route", route("Details")).unwrap();
    settle();
    let highlight = instance.get_property("current-route-index").unwrap();
    assert_eq!(highlight, Value::from(1.), "highlight source follows the current route");
    instance.set_property("nav-index", highlight).unwrap();
    settle();
    assert_eq!(
        instance.get_property("nav-index").unwrap(),
        Value::from(1.),
        "the tab bar's current-index reflects the current route"
    );
    assert_eq!(
        instance.get_property("active").unwrap(),
        Value::from(SharedString::from("details")),
        "the navigator renders the Details screen"
    );

    // Activating a tab: the chrome's `selected(int)` is routed to navigate-index,
    // which moves the current route (and its rendered screen).
    instance.invoke("navigate-index", &[Value::from(2.)]).unwrap();
    settle();
    assert_eq!(
        instance.get_property("current-route").unwrap(),
        route("Settings"),
        "activating the Settings tab navigates by ordinal"
    );
    assert_eq!(
        instance.get_property("current-route-index").unwrap(),
        Value::from(2.),
        "the highlight source moves with the activated tab"
    );
    assert_eq!(
        instance.get_property("active").unwrap(),
        Value::from(SharedString::from("settings")),
        "the navigator renders the Settings screen"
    );
}

// The presentation-agnostic proof: an int-index chrome that mimics Material's
// `BaseNavigation` (`items` / `current_index` / `index_changed` / `select`)
// drives the enum-typed navigator through the int-index adapter, both ways.
// `IntChrome` copies that contract verbatim (only `select` is public so the test
// can trigger a "tap"; Material's is protected and fired by item clicks). The
// two adapter members are consumed exactly as native Material wiring would:
//   current_index  <- current-route-index   (the bar reflects the route)
//   index_changed(i) => navigate-index(i)    (a tap navigates)
// Nothing about the `navigator { Route... }` block is chrome-specific, so std
// chrome (PR A8) binds the same two members with zero change to the routes.
#[cfg(feature = "internal")]
#[test]
fn navigator_int_chrome_binding() {
    i_slint_backend_testing::init_no_event_loop();
    use crate::{Compiler, SharedString, Value};
    let code = r#"
enum Route { Home, Details, Settings }
global NavProbe { in-out property <string> active; }
component HomeScreen inherits Rectangle { init => { NavProbe.active = "home"; } }
component DetailsScreen inherits Rectangle { init => { NavProbe.active = "details"; } }
component SettingsScreen inherits Rectangle { init => { NavProbe.active = "settings"; } }

// Mirrors Material's BaseNavigation int API (items/current_index/index_changed/select).
component IntChrome {
    in property <[string]> items;
    in-out property <int> current_index;
    callback index_changed(index: int);
    public function select(index: int) {
        if (index < 0 || index >= root.items.length) {
            return;
        }
        root.current_index = index;
        root.index_changed(index);
    }
}

export component TestCase inherits Window {
    width: 100px;
    height: 100px;
    in-out property <Route> current-route: Route.Home;
    out property <string> active: NavProbe.active;

    // Adapter members are synthesized after expression resolution, so they are a
    // native/runtime surface, not source-referenceable. The test copies
    // current-route-index into `chrome-index` and forwards index_changed out, then
    // binds both to the adapter through the runtime API, as native code does.
    in property <int> chrome-index;
    out property <int> chrome-index-out: chrome.current_index;
    callback chrome-index-changed(index: int);

    chrome := IntChrome {
        items: ["Home", "Details", "Settings"];
        current_index: root.chrome-index;              // display <- current-route-index
        index_changed(i) => { root.chrome-index-changed(i); }  // tap forwarded out
    }
    navigator (current-route) {
        Route.Home: HomeScreen { }
        Route.Details: DetailsScreen { }
        Route.Settings: SettingsScreen { }
    }
    // Simulate an item tap (Material fires select() from a NavigationItem click).
    public function tap(index: int) { chrome.select(index); }
}
"#;
    let mut compiler = Compiler::default();
    compiler.set_style("fluent".into());
    compiler.compiler_configuration(i_slint_core::InternalToken).enable_experimental = true;
    let result = spin_on::spin_on(compiler.build_from_source(code.into(), Default::default()));
    assert!(!result.has_errors(), "{:?}", result.diagnostics().collect::<Vec<_>>());
    let definition = result.component("TestCase").unwrap();
    let instance = definition.create().unwrap();
    let _ = instance.window();

    let route = |v: &str| Value::EnumerationValue("Route".into(), v.into());
    let settle = || i_slint_backend_testing::mock_elapsed_time(100);

    // The tap binding: index_changed(i) => navigate-index(i). Wired once, exactly
    // as native Material code binds NavigationBar.index_changed to the adapter.
    let weak = instance.as_weak();
    instance
        .set_callback("chrome-index-changed", move |args| {
            let i: i32 = args[0].clone().try_into().unwrap();
            weak.unwrap().invoke("navigate-index", &[Value::from(i as f64)]).unwrap();
            Value::Void
        })
        .unwrap();

    // The display binding: current_index <- current-route-index. Pushed after each
    // route change (native code does the same via set_bar_index).
    let mirror = |instance: &crate::ComponentInstance| {
        let idx = instance.get_property("current-route-index").unwrap();
        instance.set_property("chrome-index", idx).unwrap();
        settle();
    };

    // Display side: moving the route moves the chrome's current_index.
    mirror(&instance);
    assert_eq!(instance.get_property("chrome-index-out").unwrap(), Value::from(0.), "Home -> 0");
    instance.set_property("current-route", route("Details")).unwrap();
    mirror(&instance);
    assert_eq!(instance.get_property("chrome-index-out").unwrap(), Value::from(1.), "Details -> 1");
    instance.set_property("current-route", route("Settings")).unwrap();
    mirror(&instance);
    assert_eq!(
        instance.get_property("chrome-index-out").unwrap(),
        Value::from(2.),
        "Settings -> 2"
    );

    // Tap side: chrome.select(i) fires index_changed -> navigate-index(i) -> route.
    instance.invoke("tap", &[Value::from(0.)]).unwrap();
    mirror(&instance);
    assert_eq!(instance.get_property("current-route").unwrap(), route("Home"), "tap 0 -> Home");
    assert_eq!(instance.get_property("chrome-index-out").unwrap(), Value::from(0.), "bar shows 0");

    instance.invoke("tap", &[Value::from(2.)]).unwrap();
    mirror(&instance);
    assert_eq!(
        instance.get_property("current-route").unwrap(),
        route("Settings"),
        "tap 2 -> Settings"
    );
    assert_eq!(instance.get_property("chrome-index-out").unwrap(), Value::from(2.), "bar shows 2");
    assert_eq!(
        instance.get_property("active").unwrap(),
        Value::from(SharedString::from("settings"))
    );

    // Out-of-range tap is a no-op in the mimic's guard (same as BaseNavigation.select).
    instance.invoke("tap", &[Value::from(9.)]).unwrap();
    mirror(&instance);
    assert_eq!(
        instance.get_property("current-route").unwrap(),
        route("Settings"),
        "out-of-range tap is a no-op"
    );
}

// PR #13: the navigator's public members are declared before expression
// resolution, so widget chrome binds to them INLINE in .slint (not from the
// host language). This is the same IntChrome contract as
// navigator_int_chrome_binding, but the two adapter members are wired in .slint:
//   current-index: root.current-route-index       (the bar reflects the route)
//   index-changed(i) => root.navigate-index(i)     (a tap navigates)
// and can-go-back is read in a .slint binding too. Before #13 these members did
// not exist at resolve time, so this binding failed with "does not have a
// property 'current-route-index'" and had to be done from Rust.
#[cfg(feature = "internal")]
#[test]
fn navigator_inline_chrome_binding() {
    i_slint_backend_testing::init_no_event_loop();
    use crate::{Compiler, SharedString, Value};
    let code = r#"
enum Route { Home, Details, Settings }
global NavProbe { in-out property <string> active; }
component HomeScreen inherits Rectangle { init => { NavProbe.active = "home"; } }
component DetailsScreen inherits Rectangle { init => { NavProbe.active = "details"; } }
component SettingsScreen inherits Rectangle { init => { NavProbe.active = "settings"; } }

// Int-index chrome, same contract as Material's BaseNavigation: the host feeds
// the highlighted index in, a tap fires index-changed out.
component IntChrome {
    in property <[string]> items;
    in property <int> current-index;
    callback index-changed(index: int);
    public function select(index: int) {
        if (index < 0 || index >= root.items.length) {
            return;
        }
        root.index-changed(index);
    }
}

export component TestCase inherits Window {
    width: 100px;
    height: 100px;
    in-out property <Route> current-route: Route.Home;
    out property <string> active: NavProbe.active;

    // The #13 proof: the chrome binds the navigator's synthesized members
    // directly in .slint. These mirrors let the test observe the bound values.
    out property <int> chrome-index: chrome.current-index;
    out property <bool> chrome-can-back: root.can-go-back;

    chrome := IntChrome {
        items: ["Home", "Details", "Settings"];
        current-index: root.current-route-index;            // display <- current-route-index
        index-changed(i) => { root.navigate-index(i); }     // tap -> navigate-index
    }
    navigator (current-route) {
        Route.Home: HomeScreen { }
        Route.Details: DetailsScreen { }
        Route.Settings: SettingsScreen { }
    }
    // Simulate an item tap (Material fires select() from a NavigationItem click).
    public function tap(index: int) { chrome.select(index); }
}
"#;
    let mut compiler = Compiler::default();
    compiler.set_style("fluent".into());
    compiler.compiler_configuration(i_slint_core::InternalToken).enable_experimental = true;
    let result = spin_on::spin_on(compiler.build_from_source(code.into(), Default::default()));
    assert!(!result.has_errors(), "{:?}", result.diagnostics().collect::<Vec<_>>());
    let definition = result.component("TestCase").unwrap();
    let instance = definition.create().unwrap();
    let _ = instance.window();

    let route = |v: &str| Value::EnumerationValue("Route".into(), v.into());
    let settle = || i_slint_backend_testing::mock_elapsed_time(100);

    // Read side, all in .slint: the bar's current-index follows current-route-index.
    settle();
    assert_eq!(instance.get_property("chrome-index").unwrap(), Value::from(0.), "Home -> 0");
    assert_eq!(instance.get_property("chrome-can-back").unwrap(), Value::from(false), "root");

    instance.set_property("current-route", route("Details")).unwrap();
    settle();
    assert_eq!(
        instance.get_property("chrome-index").unwrap(),
        Value::from(1.),
        "the bar's current-index reflects the route, bound in .slint"
    );
    assert_eq!(
        instance.get_property("active").unwrap(),
        Value::from(SharedString::from("details"))
    );

    // Write side, all in .slint: tap -> select -> index-changed -> navigate-index.
    instance.invoke("tap", &[Value::from(2.)]).unwrap();
    settle();
    assert_eq!(
        instance.get_property("current-route").unwrap(),
        route("Settings"),
        "a .slint-wired tap navigates by ordinal"
    );
    assert_eq!(instance.get_property("chrome-index").unwrap(), Value::from(2.), "bar shows 2");
    assert_eq!(
        instance.get_property("active").unwrap(),
        Value::from(SharedString::from("settings"))
    );
    // navigate-index pushed the previous route, so can-go-back (bound in .slint) is true.
    assert_eq!(
        instance.get_property("chrome-can-back").unwrap(),
        Value::from(true),
        "can-go-back reflects the push, read in a .slint binding"
    );

    // Another tap moves it back to the first route.
    instance.invoke("tap", &[Value::from(0.)]).unwrap();
    settle();
    assert_eq!(instance.get_property("current-route").unwrap(), route("Home"), "tap 0 -> Home");
    assert_eq!(instance.get_property("chrome-index").unwrap(), Value::from(0.), "bar shows 0");

    // Out-of-range tap is a no-op (the chrome's own guard).
    instance.invoke("tap", &[Value::from(9.)]).unwrap();
    settle();
    assert_eq!(
        instance.get_property("current-route").unwrap(),
        route("Home"),
        "out-of-range tap is a no-op"
    );
}
