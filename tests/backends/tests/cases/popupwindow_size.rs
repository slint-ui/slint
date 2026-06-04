/// If the size of the popup window is fixed, do not use the layout constraint values
#[satchel::test]
fn popupwindow_size() {
    slint::slint! {

        export global Properties {
            in-out property <bool> popup-initialized: false;
            in-out property <length> popup-width: -1px;
            in-out property <length> popup-height: -1px;

            callback popup-timer-triggered(int);

        }
        export component MainWindow inherits Window {
            width: 600px;
            height: 400px;

            Timer {
                running: true;
                interval: 100ms;
                triggered => {
                    self.running = false;
                    popup.show();
                }
            }

            // Layout constraints are 0, 0 because the Rectangle prefers 0,0
            popup := PopupWindow {
                width: 99px;
                height: 102px;

                init => {
                    Properties.popup-initialized = true;
                    Properties.popup-width = self.width;
                    Properties.popup-height = self.height;
                }

                changed width => {
                    Properties.popup-width = self.width;
                    Properties.popup-height = self.height;
                }

                rect:= Rectangle {
                    background: red;
                }

                Timer {
                    running: true;
                    interval: 100ms;

                    property <int> count:0;

                    triggered => {
                        rect.max-width = 100px;
                        self.count += 1;
                        popup.width += 10px;
                        popup.height += 1px;
                        Properties.popup-timer-triggered(count);
                    }
                }
            }
        }
    }

    let app = MainWindow::new().unwrap();
    const TIMER_TRIGGER_COUNTS: i32 = 3;
    // app.invoke_show_popup(); // Opens the popup, but does not execute Winit backend update_window_properties()
    assert_eq!(app.global::<Properties>().get_popup_initialized(), false);
    app.global::<Properties>().on_popup_timer_triggered(|count| {
        if count >= TIMER_TRIGGER_COUNTS {
            slint::quit_event_loop().unwrap();
        }
    });

    app.run().unwrap();

    assert_eq!(app.global::<Properties>().get_popup_initialized(), true);
    assert_eq!(
        app.global::<Properties>().get_popup_width(),
        99. + TIMER_TRIGGER_COUNTS as f32 * 10.
    );
    assert_eq!(
        app.global::<Properties>().get_popup_height(),
        102. + TIMER_TRIGGER_COUNTS as f32 * 1.
    );
}
