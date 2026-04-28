// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_backend_testing::ElementHandle;
use slint::platform::PointerEventButton;

#[test]
fn test_click() {
    i_slint_backend_testing::init_integration_test_with_system_time();

    slint::spawn_local(async move {
        slint::slint! {
            export component App inherits Window {
                out property <int> click-count: 0;
                out property <int> double-click-count: 0;
                ta := TouchArea {
                    clicked => { root.click-count += 1; }
                    double-clicked => { root.double-click-count += 1; }
                }
            }
        }

        let app = App::new().unwrap();

        let mut it = ElementHandle::find_by_element_id(&app, "App::ta");
        let elem = it.next().unwrap();
        assert!(it.next().is_none());

        assert_eq!(app.get_click_count(), 0);
        assert_eq!(app.get_double_click_count(), 0);
        elem.single_click(PointerEventButton::Left).await;
        assert_eq!(app.get_click_count(), 1);
        assert_eq!(app.get_double_click_count(), 0);

        elem.double_click(PointerEventButton::Left).await;
        assert_eq!(app.get_click_count(), 3);
        assert_eq!(app.get_double_click_count(), 1);

        slint::quit_event_loop().unwrap();
    })
    .unwrap();
    slint::run_event_loop().unwrap();
}
