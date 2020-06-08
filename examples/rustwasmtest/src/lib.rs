#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

// Using a macro for now.  But there could be others ways to do that
sixtyfps::sixtyfps! {
    component TwoRectangle := Rectangle {

        signal clicked;

        Rectangle {
            x: 50;
            y: 50.;
            width: 25;
            height: 25;
            color: red;

            my_area := TouchArea {
                width: 25;
                height: 25;
                clicked => { root.clicked() }
            }

        }
    }


    component ButtonRectangle := Rectangle {
        property<string> button_text;
        signal clicked;
        width: 100;
        height: 75;
        TouchArea {
            width: 100;
            height: 75;
            clicked => { root.clicked() }
        }
        Text {
            x: 50;
            y: 10;
            text: button_text;
            color: black;
        }
    }

    Hello := Rectangle {

        signal foobar;
        property<int32> counter;

        color: white;

        TwoRectangle {
            width: 100;
            height: 100;
            color: blue;
            clicked => { foobar() }
        }
        Rectangle {
            x: 100;
            y: 100;
            width: (100);
            height: {100}
            color: green;
            Rectangle {
                x: 50;
                y: 50.;
                width: 25;
                height: 25;
                color: yellow;
            }
        }

        ButtonRectangle {
            color: 4289374890;
            x: 50;
            y: 225;
            clicked => { counter += 1 }
            button_text: "+";
        }
        counter_label := Text { x: 100; y: 300; text: counter; color: black; }
        ButtonRectangle {
            color: 4289374890;
            x: 50;
            y: 350;
            clicked => { counter -= 1 }
            button_text: "-";
        }

    }

}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub fn wasm_main() {
    // This provides better error messages in debug mode.
    // It's disabled in release mode so it doesn't bloat up the file size.
    #[cfg(debug_assertions)]
    console_error_panic_hook::set_once();

    Hello::default().run();
}
