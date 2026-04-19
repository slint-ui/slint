// Check that the layout sizes are fixed even when the layout changes

#[satchel::test]
fn popupwindow_size_layout() {
    slint::slint! {

        import { Button, VerticalBox, HorizontalBox } from "std-widgets.slint";

        export global Properties {
            in-out property <bool> popup-initialized: false;
            in-out property <length> popup-width: -1px;
            in-out property <length> popup-height: -1px;

            in-out property <bool> button-in-popup-pressed: false;
            in-out property <bool> button-top-visible: false;

            in-out property <length> button-width: -1px;
            in-out property <length> button-height: -1px;
            in-out property <length> btn-top-width: -1px;
            in-out property <length> btn-top-height: -1px;
            in-out property <length> btn-middle-width: -1px;
            in-out property <length> btn-middle-height: -1px;
            in-out property <length> btn-bottom-width: -1px;
            in-out property <length> btn-bottom-height: -1px;

            callback cb-popup-initialized();
            callback cb-buttons-visible();
            callback cb-button-top-visible();
        }
        export component MainWindow inherits Window {
            width: 600px;
            height: 400px;
            title: "popupwindow_size2.rs";

            in-out property <bool> show-buttons;

            Timer {
                running: true;
                interval: 100ms;
                triggered => {
                    self.running = false;
                    popup.show();
                }
            }

            VerticalBox {
                Button {
                    text: "Show popup";

                    clicked => {
                        debug("Show popup");
                        popup.show();
                    }
                }
            }

            popup := PopupWindow {
                width: 102px;
                height: 180px;

                Rectangle {
                    background: red;
                }

                changed width => {
                    Properties.popup-width = self.width;
                }

                changed height => {
                    Properties.popup-height = self.height;
                }

                close-policy: PopupClosePolicy.no-auto-close;

                init => {
                    Properties.popup-initialized = true;
                    Properties.popup-width = self.width;
                    Properties.popup-height = self.height;
                }

                // Popup initialized and trigger show the middle button
                Timer {
                    running: true;
                    interval: 100ms;
                    triggered => {
                        // We are sure that the previous state was correctly initialized
                        Properties.btn-top-height = btn-top.height;
                        Properties.btn-top-width = btn-top.width;
                        Properties.btn-bottom-height = btn-bottom.height;
                        Properties.btn-bottom-width = btn-bottom.width;
                        Properties.cb-popup-initialized();
                        self.running = false;
                        root.show-buttons = true;
                    }
                }

                // Middle button initialized properly
                show-buttons-timer:= Timer {
                    running: show-buttons;
                    interval: 100ms;
                    triggered => {
                        Properties.btn-top-height = btn-top.height;
                        Properties.btn-top-width = btn-top.width;
                        // Properties.btn-middle-height = btn-middle.height;
                        // Properties.btn-middle-width = btn-middle.width;
                        Properties.btn-bottom-height = btn-bottom.height;
                        Properties.btn-bottom-width = btn-bottom.width;

                        Properties.cb-buttons-visible();
                    }
                }


                VerticalBox {
                    padding: 4px;
                    spacing: 8px;
                    btn-top:= Button {
                        text: "Button Top";
                    }
                    if root.show-buttons : Button {
                        text: "Button Middle";
                    }

                    btn-bottom:= Button {
                        text: root.show-buttons ? "Hide Buttons" : "Show buttons";

                        clicked => {
                            // debug(self.text);
                            Properties.button-in-popup-pressed = true;
                            root.show-buttons = true;
                        }
                    }
                }
            }
        }
    }

    const PADDING: f32 = 4.;
    const SPACING: f32 = 8.;

    const POPUP_FIXED_WIDTH: f32 = 102.;
    const POPUP_FIXED_HEIGHT: f32 = 180.;

    let app = MainWindow::new().unwrap();

    // app.invoke_show_popup(); // Opens the popup, but does not execute Winit backend update_window_properties()
    assert_eq!(app.global::<Properties>().get_popup_initialized(), false);
    app.global::<Properties>().on_cb_popup_initialized({
        let app = app.as_weak();
        move || {
            let app = app.upgrade().unwrap();
            assert_eq!(app.global::<Properties>().get_popup_initialized(), true);
            assert_eq!(app.global::<Properties>().get_popup_width(), POPUP_FIXED_WIDTH);
            assert_eq!(app.global::<Properties>().get_popup_height(), POPUP_FIXED_HEIGHT);

            const BUTTON_HEIGHT: f32 = (POPUP_FIXED_HEIGHT - 2. * PADDING - SPACING) / 2.;
            assert_eq!(app.global::<Properties>().get_btn_top_height(), BUTTON_HEIGHT);
            assert_eq!(app.global::<Properties>().get_btn_bottom_height(), BUTTON_HEIGHT);
        }
    });

    app.global::<Properties>().on_cb_buttons_visible({
        let app = app.as_weak();
        move || {
            let app = app.upgrade().unwrap();
            assert_eq!(app.global::<Properties>().get_popup_width(), POPUP_FIXED_WIDTH);
            assert_eq!(app.global::<Properties>().get_popup_height(), POPUP_FIXED_HEIGHT);

            const BUTTON_HEIGHT: f32 = (POPUP_FIXED_HEIGHT - 2. * PADDING - 2. * SPACING) / 3.;
            assert_eq!(app.global::<Properties>().get_btn_top_height(), BUTTON_HEIGHT);
            // assert_eq!(app.global::<Properties>().get_btn_middle_height(), BUTTON_HEIGHT);
            assert_eq!(app.global::<Properties>().get_btn_bottom_height(), BUTTON_HEIGHT);

            slint::quit_event_loop().unwrap();
        }
    });

    app.run().unwrap();
}
