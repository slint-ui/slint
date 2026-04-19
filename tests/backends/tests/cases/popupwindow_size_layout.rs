// Check that the layout sizes are correct
// For the TextInput the textsize must be determining using the scale factor so it is handled differently
// than a normal rectangle

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
            in-out property <length> text-width: -1px;
            in-out property <length> text-height: -1px;

            callback cb-popup-initialized();
            callback cb-buttons-visible();
            callback cb-button-top-visible();
        }
        export component MainWindow inherits Window {
            width: 600px;
            height: 400px;

            in-out property <bool> show-buttons;

            Timer {
                running: true;
                interval: 100ms;
                triggered => {
                    self.running = false;
                    popup.show();
                }
            }

            show-buttons-timer:= Timer {
                running: show-buttons;
                interval: 100ms;
                triggered => {
                    Properties.cb-buttons-visible();
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

                    Properties.text-height = ti.preferred-height;
                    Properties.text-width = ti.width;

                    Properties.button-height = max(btn.preferred-height, btn.min-height);
                    Properties.button-width = btn.width;

                    // debug("Popup initialized");
                    // debug("Text input size: ", ti.width, ", ", ti.height);
                    Properties.cb-popup-initialized();
                }

                Timer {
                    running: true;
                    interval: 200ms;
                    triggered => {
                        self.running = false;
                        root.show-buttons = true;
                    }
                }


                VerticalBox {
                    padding: 9px;
                    spacing: 6px;
                    ti:= TextInput {
                        width: 100px;
                        text: "Hello";

                        init => {
                            // debug("Text initialized");
                            Properties.text-height = self.preferred-height;
                            Properties.text-width = self.width;
                        }
                    }
                    if root.show-buttons : HorizontalBox {
                        padding: 9px;
                        spacing: 6px;
                        Button {
                            text: "Button top";
                            init => {
                                // debug("Button. Preferred width: ", self.preferred-width);
                                // debug("Button. Preferred height: ", self.preferred-height);
                                // debug("Button. width: ", self.width);
                                Properties.button_top_visible = true;
                                Properties.cb-button-top-visible();
                            }
                        }

                        Button {
                            text: "Button top";
                        }

                        Button {
                            text: "Button bottom";
                        }
                    }

                    btn:= Button {
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

    let app = MainWindow::new().unwrap();
    const PADDING: f32 = 9.;
    const SPACING: f32 = 6.;
    const BUTTON_HEIGHT: f32 = 32.;
    const TEXT_HEIGHT: f32 = 17.;
    const HORIZONTAL_BOX_HEIGHT: f32 = PADDING + BUTTON_HEIGHT + PADDING;

    // app.invoke_show_popup(); // Opens the popup, but does not execute Winit backend update_window_properties()
    assert_eq!(app.global::<Properties>().get_popup_initialized(), false);
    app.global::<Properties>().on_cb_popup_initialized({
        let app = app.as_weak();
        move || {
            let app = app.upgrade().unwrap();
            // assert_eq!(app.global::<Properties>().get_popup_width(), 126.);
            assert_eq!(app.global::<Properties>().get_text_height(), TEXT_HEIGHT);
            assert_eq!(app.global::<Properties>().get_button_height(), BUTTON_HEIGHT);
            assert_eq!(
                app.global::<Properties>().get_popup_height(),
                PADDING + TEXT_HEIGHT + SPACING + BUTTON_HEIGHT + PADDING
            );
        }
    });

    app.global::<Properties>().on_cb_buttons_visible({
        let app = app.as_weak();
        move || {
            let app = app.upgrade().unwrap();
            assert_eq!(app.global::<Properties>().get_text_height(), TEXT_HEIGHT);
            assert_eq!(app.global::<Properties>().get_button_height(), BUTTON_HEIGHT);
            assert_eq!(
                app.global::<Properties>().get_popup_height(),
                PADDING
                    + TEXT_HEIGHT
                    + SPACING
                    + HORIZONTAL_BOX_HEIGHT
                    + SPACING
                    + BUTTON_HEIGHT
                    + PADDING
            );
            slint::quit_event_loop().unwrap();
        }
    });

    app.run().unwrap();
}
