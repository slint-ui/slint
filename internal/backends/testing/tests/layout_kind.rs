// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use core::ops::ControlFlow;
use i_slint_backend_testing::{ElementHandle, LayoutKind};

#[test]
fn test_layout_kind() {
    i_slint_backend_testing::init_integration_test_with_system_time();

    slint::slint! {
        export component App inherits Window {
            hl := HorizontalLayout {
                Rectangle {}
            }
            vl := VerticalLayout {
                Rectangle {}
            }
            gl := GridLayout {
                Rectangle {}
            }
            rect := Rectangle {}
        }
    }

    let app = App::new().unwrap();

    let elems: Vec<_> = ElementHandle::find_by_element_id(&app, "App::hl").collect();
    assert_eq!(elems.len(), 1);
    assert_eq!(elems[0].layout_kind(), Some(LayoutKind::HorizontalLayout));

    let elems: Vec<_> = ElementHandle::find_by_element_id(&app, "App::vl").collect();
    assert_eq!(elems.len(), 1);
    assert_eq!(elems[0].layout_kind(), Some(LayoutKind::VerticalLayout));

    let elems: Vec<_> = ElementHandle::find_by_element_id(&app, "App::gl").collect();
    assert_eq!(elems.len(), 1);
    assert_eq!(elems[0].layout_kind(), Some(LayoutKind::GridLayout));

    let elems: Vec<_> = ElementHandle::find_by_element_id(&app, "App::rect").collect();
    assert_eq!(elems.len(), 1);
    assert_eq!(elems[0].layout_kind(), None);

    // Nested layouts: inner layout reports its own kind
    slint::slint! {
        export component Nested inherits Window {
            outer := VerticalLayout {
                inner := HorizontalLayout {
                    Rectangle {}
                }
            }
        }
    }

    let nested = Nested::new().unwrap();

    let outer: Vec<_> = ElementHandle::find_by_element_id(&nested, "Nested::outer").collect();
    assert_eq!(outer.len(), 1);
    assert_eq!(outer[0].layout_kind(), Some(LayoutKind::VerticalLayout));

    let inner: Vec<_> = ElementHandle::find_by_element_id(&nested, "Nested::inner").collect();
    assert_eq!(inner.len(), 1);
    assert_eq!(inner[0].layout_kind(), Some(LayoutKind::HorizontalLayout));

    // Verify hierarchy: inner HorizontalLayout is a descendant of outer VerticalLayout
    let found_inner = outer[0].visit_descendants(|descendant| {
        if descendant.id() == Some(slint::SharedString::from("Nested::inner")) {
            ControlFlow::Break(descendant)
        } else {
            ControlFlow::Continue(())
        }
    });
    assert!(found_inner.is_some());
    assert_eq!(found_inner.unwrap().layout_kind(), Some(LayoutKind::HorizontalLayout));

    // type_name() is consistent with layout_kind()
    assert_eq!(inner[0].type_name(), Some(slint::SharedString::from("HorizontalLayout")));

    // HorizontalBox / VerticalBox (styled widget variants)
    slint::slint! {
        import { HorizontalBox, VerticalBox } from "std-widgets.slint";
        export component Boxes inherits Window {
            hb := HorizontalBox {
                Rectangle {}
            }
            vb := VerticalBox {
                Rectangle {}
            }
        }
    }

    let boxes = Boxes::new().unwrap();

    let elems: Vec<_> = ElementHandle::find_by_element_id(&boxes, "Boxes::hb").collect();
    assert_eq!(elems.len(), 1);
    assert_eq!(elems[0].layout_kind(), Some(LayoutKind::HorizontalLayout));
    assert_eq!(elems[0].type_name(), Some(slint::SharedString::from("HorizontalBox")));

    let elems: Vec<_> = ElementHandle::find_by_element_id(&boxes, "Boxes::vb").collect();
    assert_eq!(elems.len(), 1);
    assert_eq!(elems[0].layout_kind(), Some(LayoutKind::VerticalLayout));
    assert_eq!(elems[0].type_name(), Some(slint::SharedString::from("VerticalBox")));
}
