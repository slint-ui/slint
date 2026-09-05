// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_backend_testing::ElementHandle;
use slint::platform::PointerEventButton;

#[test]
fn test_click_under_transform_scale() {
    i_slint_backend_testing::init_integration_test_with_system_time();

    slint::spawn_local(async move {
        slint::slint! {
            export component App inherits Window {
                width: 400px;
                height: 300px;
                out property <int> click-count: 0;
                Rectangle {
                    transform-scale: 0.25;
                    Rectangle {
                        x: 0px;
                        y: 0px;
                        width: 200px;
                        height: 100px;
                        ta := TouchArea {
                            clicked => { root.click-count += 1; }
                        }
                    }
                }
            }
        }

        let app = App::new().unwrap();

        let elem = ElementHandle::find_by_element_id(&app, "App::ta").next().unwrap();
        elem.single_click(PointerEventButton::Left).await;
        assert_eq!(app.get_click_count(), 1, "the click aimed outside the scaled element");

        slint::quit_event_loop().unwrap();
    })
    .unwrap();
    slint::run_event_loop().unwrap();
}
