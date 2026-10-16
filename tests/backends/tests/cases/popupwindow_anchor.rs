// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// Check that the anchor location, the anchor rectangle's size, the gravity and the offset
// place the popup where expected, whether the backend shows it in its own window or inside
// the parent window.
//
// The position is compared against a second popup without anchor at the parent window's origin,
// because a window's position includes its frame while popups are placed relative to its content.

use i_slint_core::window::{PopupWindowLocation, WindowInner};

#[satchel::test]
fn popupwindow_anchor() {
    slint::slint! {
        export component MainWindow inherits Window {
            width: 400px;
            height: 300px;

            callback popup-shown();

            Timer {
                running: true;
                interval: 100ms;
                triggered => {
                    self.running = false;
                    reference.show();
                    popup.show();
                }
            }

            // Each popup has its own parent element: showing a popup closes its siblings.
            Rectangle {
                reference := PopupWindow {
                    x: 0px;
                    y: 0px;
                    width: 10px;
                    height: 10px;
                    close-policy: PopupClosePolicy.no-auto-close;
                    Rectangle {
                        background: red;
                    }
                }
            }

            popup := PopupWindow {
                x: 50px;
                y: 60px;
                width: 80px;
                height: 40px;
                close-policy: PopupClosePolicy.no-auto-close;
                anchor: {
                    location: PopupAnchorLocation.bottom-right,
                    width: 100px,
                    height: 20px,
                    gravity: PopupGravity.bottom-right,
                    x: 5px,
                    y: 7px,
                };

                Rectangle {
                    background: green;
                }

                Timer {
                    running: true;
                    interval: 500ms;
                    triggered => {
                        self.running = false;
                        root.popup-shown();
                    }
                }
            }
        }
    }

    let app = MainWindow::new().unwrap();
    app.on_popup_shown({
        let app = app.as_weak();
        move || {
            let app = app.upgrade().unwrap();
            let popups = WindowInner::from_pub(app.window()).active_popups.borrow();
            assert_eq!(popups.len(), 2, "Both popups are open");
            let position = |location: &PopupWindowLocation| match location {
                PopupWindowLocation::ChildWindow(location) => (location.x, location.y),
                PopupWindowLocation::TopLevel(adapter) => {
                    let position = adapter
                        .position()
                        .expect("the popup window has a position")
                        .to_logical(adapter.window().scale_factor());
                    (position.x, position.y)
                }
            };
            let reference = position(&popups[0].location);
            let popup = position(&popups[1].location);
            let (x, y) = (popup.0 - reference.0, popup.1 - reference.1);
            // The anchor point is the bottom-right corner of the anchor rectangle:
            // (50 + 100, 60 + 20). With the bottom-right gravity the popup's top-left corner
            // sits on it, moved by the offset (5, 7).
            assert_eq!((x.round(), y.round()), (155., 87.));
            slint::quit_event_loop().unwrap();
        }
    });

    app.run().unwrap();
}
