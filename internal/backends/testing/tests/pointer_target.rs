// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_backend_testing::{ElementHandle, ElementQuery};
use slint::{
    ComponentHandle,
    platform::{PointerEventButton, WindowEvent},
};

slint::slint! {
    export component App inherits Window {
        width: 400px; height: 400px;
        in-out property <bool> covered: true;
        out property <int> clicks: 0;
        out property <bool> hovered: target.has-hover;
        target := TouchArea {
            x: 10px; y: 10px; width: 80px; height: 40px;
            accessible-role: button; accessible-label: "Target";
            clicked => { root.clicks += 1; }
        }
        if root.covered: TouchArea {
            x: 10px; y: 10px; width: 80px; height: 40px;
            accessible-role: button; accessible-label: "Cover";
            clicked => { root.clicks += 100; }
        }
        flick := Flickable {
            x: 10px; y: 100px; width: 120px; height: 70px;
            content-height: 400px; content-width: 120px;
            far := TouchArea {
                x: 10px; y: 300px; width: 80px; height: 30px;
                accessible-role: button; accessible-label: "Far";
                clicked => { root.clicks += 10; }
            }
        }
        Rectangle {
            x: 200px; y: 100px; width: 80px; height: 70px; clip: true;
            clipped := TouchArea { y: 100px; width: 60px; height: 30px; }
        }
        disabled := TouchArea { x: 200px; y: 10px; width: 60px; height: 30px; enabled: false; }
        popup := PopupWindow {
            width: 100px; height: 80px;
            popup-target := TouchArea { accessible-role: button; accessible-label: "Popup target"; }
        }
        callback show-popup();
        show-popup => { popup.show(); }
    }
}

fn find(app: &App, name: &str) -> ElementHandle {
    ElementQuery::from_root(app)
        .include_clipped()
        .match_id(format!("App::{name}"))
        .find_first()
        .unwrap()
}

#[test]
fn read_only_routing_and_explicit_scrolling() {
    i_slint_backend_testing::init_integration_test_with_system_time();
    slint::spawn_local(async {
        let app = App::new().unwrap();
        app.show().unwrap();
        let target = find(&app, "target");
        let result = target.pointer_target().unwrap();
        assert_eq!(result.status, "covered", "{result:?}");
        assert!(result.detail.contains("Cover"));
        assert!(!app.get_hovered());
        assert_eq!(app.get_clicks(), 0);
        app.set_covered(false);
        assert_eq!(target.pointer_target().unwrap().status, "ready");
        assert!(!app.get_hovered());
        target.single_click(PointerEventButton::Left).await;
        assert_eq!(app.get_clicks(), 1);
        let far = find(&app, "far");
        assert!(
            ElementQuery::from_root(&app)
                .include_clipped()
                .match_id("App::flick")
                .match_descendants()
                .match_id("App::far")
                .find_first()
                .is_some()
        );
        assert_eq!(far.pointer_target().unwrap().status, "clipped");
        assert!(ElementHandle::find_by_element_id(&app, "App::far").next().is_none());
        assert_eq!(far.scroll_into_view().unwrap().status, "ready");
        far.single_click(PointerEventButton::Left).await;
        assert_eq!(app.get_clicks(), 11);
        assert_eq!(find(&app, "clipped").scroll_into_view().unwrap().status, "clipped");
        assert_ne!(find(&app, "disabled").pointer_target().unwrap().status, "ready");
        app.window().dispatch_event(WindowEvent::PointerPressed {
            position: target.pointer_target().unwrap().position,
            button: PointerEventButton::Left,
        });
        assert_eq!(target.pointer_target().unwrap().status, "busy");
        app.window().dispatch_event(WindowEvent::PointerReleased {
            position: target.pointer_target().unwrap().position,
            button: PointerEventButton::Left,
        });
        app.invoke_show_popup();
        assert_ne!(target.pointer_target().unwrap().status, "ready");
        assert_eq!(find(&app, "popup-target").pointer_target().unwrap().status, "ready");
        slint::quit_event_loop().unwrap();
    })
    .unwrap();
    slint::run_event_loop().unwrap();
}

#[cfg(feature = "system-testing")]
#[test]
fn python_protocol_matches_native_schema() {
    assert_eq!(
        include_bytes!(concat!(env!("OUT_DIR"), "/slint_systest.descriptor")).as_slice(),
        include_bytes!("../../../../tools/slint-test/slint_test/native.descriptor").as_slice(),
        "Copy OUT_DIR/slint_systest.descriptor to tools/slint-test/slint_test/native.descriptor"
    );
}
