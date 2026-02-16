// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

slint::slint! {
    export component Demo inherits Window {
        width: 600px;
        height: 500px;
        background: #1e1e2e;
        title: "Rounded Clip Demo";

        VerticalLayout {
            padding: 20px;
            spacing: 20px;

            Text {
                text: "Rounded Clip Demo - Software Renderer";
                color: white;
                font-size: 24px;
                horizontal-alignment: center;
            }

            HorizontalLayout {
                spacing: 30px;
                alignment: center;

                // Example 1: Basic rounded clip
                VerticalLayout {
                    spacing: 10px;
                    Text {
                        text: "Basic Rounded Clip";
                        color: #cdd6f4;
                        horizontal-alignment: center;
                    }
                    Rectangle {
                        width: 150px;
                        height: 150px;
                        border-radius: 30px;
                        background: #45475a;
                        clip: true;

                        // Inner rectangle that gets clipped
                        Rectangle {
                            x: -20px;
                            y: -20px;
                            width: 100px;
                            height: 100px;
                            background: #f38ba8;
                        }

                        // Another rectangle in the corner
                        Rectangle {
                            x: 90px;
                            y: 90px;
                            width: 80px;
                            height: 80px;
                            background: #89b4fa;
                        }
                    }
                }

                // Example 2: Circular clip
                VerticalLayout {
                    spacing: 10px;
                    Text {
                        text: "Circular Clip";
                        color: #cdd6f4;
                        horizontal-alignment: center;
                    }
                    Rectangle {
                        width: 150px;
                        height: 150px;
                        border-radius: 75px;  // Makes it a circle
                        background: #45475a;
                        clip: true;

                        // Large square that gets clipped to circle
                        Rectangle {
                            width: 150px;
                            height: 150px;
                            background: #fab387;
                        }

                        // Inner square
                        Rectangle {
                            x: 50px;
                            y: 50px;
                            width: 50px;
                            height: 50px;
                            background: #313244;
                        }
                    }
                }

                // Example 3: Asymmetric radii
                VerticalLayout {
                    spacing: 10px;
                    Text {
                        text: "Different Corner Radii";
                        color: #cdd6f4;
                        horizontal-alignment: center;
                    }
                    Rectangle {
                        width: 150px;
                        height: 150px;
                        border-top-left-radius: 50px;
                        border-top-right-radius: 10px;
                        border-bottom-left-radius: 10px;
                        border-bottom-right-radius: 50px;
                        background: #45475a;
                        clip: true;

                        // Rectangle spanning full area
                        Rectangle {
                            width: 150px;
                            height: 150px;
                            background: #cba6f7;
                        }

                        // Corner rectangles to show clipping
                        Rectangle {
                            x: -10px;
                            y: -10px;
                            width: 40px;
                            height: 40px;
                            background: #f38ba8;
                        }
                        Rectangle {
                            x: 120px;
                            y: 120px;
                            width: 40px;
                            height: 40px;
                            background: #a6e3a1;
                        }
                    }
                }
            }

            // Example 4: Content that moves showing clip
            VerticalLayout {
                spacing: 10px;
                Text {
                    text: "Content Offset (showing clip at edges)";
                    color: #cdd6f4;
                    horizontal-alignment: center;
                }
                Rectangle {
                    width: 300px;
                    height: 100px;
                    border-radius: 20px;
                    background: #313244;
                    clip: true;

                    // Rectangle positioned to show clipping at left edge
                    Rectangle {
                        x: -30px;
                        y: 10px;
                        width: 80px;
                        height: 80px;
                        background: #f5c2e7;
                        border-radius: 10px;
                    }

                    // Rectangle in the middle
                    Rectangle {
                        x: 110px;
                        y: 20px;
                        width: 80px;
                        height: 60px;
                        background: #94e2d5;
                        border-radius: 5px;
                    }

                    // Rectangle positioned to show clipping at right edge
                    Rectangle {
                        x: 250px;
                        y: 10px;
                        width: 80px;
                        height: 80px;
                        background: #89b4fa;
                        border-radius: 10px;
                    }
                }
            }

            // Example 5: Gradient clipped by rounded corners
            VerticalLayout {
                spacing: 10px;
                Text {
                    text: "Gradient Clipped";
                    color: #cdd6f4;
                    horizontal-alignment: center;
                }
                Rectangle {
                    width: 200px;
                    height: 100px;
                    border-radius: 30px;
                    background: #45475a;
                    clip: true;

                    // Gradient rectangle that gets clipped
                    Rectangle {
                        width: 200px;
                        height: 100px;
                        background: @linear-gradient(135deg, #f38ba8, #fab387, #f9e2af, #a6e3a1, #89b4fa, #cba6f7);
                    }
                }
            }

            // Example 6: Nested clips
            VerticalLayout {
                spacing: 10px;
                Text {
                    text: "Nested Rounded Clips";
                    color: #cdd6f4;
                    horizontal-alignment: center;
                }
                Rectangle {
                    width: 200px;
                    height: 100px;
                    border-radius: 30px;
                    background: #45475a;
                    clip: true;

                    Rectangle {
                        x: 25px;
                        y: 10px;
                        width: 150px;
                        height: 80px;
                        border-radius: 20px;
                        background: #585b70;
                        clip: true;

                        Rectangle {
                            x: -20px;
                            y: -20px;
                            width: 60px;
                            height: 60px;
                            background: #f38ba8;
                        }

                        Rectangle {
                            x: 110px;
                            y: 40px;
                            width: 60px;
                            height: 60px;
                            background: #a6e3a1;
                        }
                    }
                }
            }
        }
    }
}

fn main() {
    // Use software renderer with line-by-line rendering
    std::env::set_var("SLINT_BACKEND", "winit-software");
    std::env::set_var("SLINT_LINE_BY_LINE", "1");

    Demo::new().unwrap().run().unwrap();
}

