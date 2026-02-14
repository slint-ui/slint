// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

slint::slint! {
    export component App inherits Window {
        width: 600px;
        height: 600px;
        title: "Pinch to Zoom";
        background: #1e1e2e;

        property <float> current-scale: 1.0;
        property <float> scale-at-start: 1.0;
        property <string> status: "Use trackpad pinch gesture";

        pinch := PinchGestureHandler {
            pinch-started => {
                scale-at-start = current-scale;
                status = "Pinching...";
            }
            pinch-updated => {
                current-scale = clamp(scale-at-start * self.scale, 0.2, 5.0);
                status = "Scale: " + round(current-scale * 100) + "%";
            }
            pinch-ended => {
                status = "Scale: " + round(current-scale * 100) + "% (done)";
            }
            pinch-cancelled => {
                status = "Gesture cancelled";
            }
        }

        VerticalLayout {
            padding: 20px;
            spacing: 20px;

            Text {
                text: root.status;
                color: #cdd6f4;
                font-size: 16px;
                horizontal-alignment: center;
            }

            Rectangle {
                background: transparent;
                clip: true;

                rect := Rectangle {
                    width: 200px * current-scale;
                    height: 200px * current-scale;
                    background: #89b4fa;
                    border-radius: 12px * current-scale;

                    Text {
                        text: round(current-scale * 100) + "%";
                        color: #1e1e2e;
                        font-size: 24px * current-scale;
                        font-weight: 700;
                    }
                }
            }
        }
    }
}

fn main() {
    App::new().unwrap().run().unwrap();
}
